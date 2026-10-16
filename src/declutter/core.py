"""Declutter core logic. No GUI imports here so it can be tested on its own.

Safety rules:
- Nothing is ever permanently deleted: removals go to the Recycle Bin.
- Sorting moves are logged so the last batch can be undone.
- Drive roots, the home folder (and its parents), system folders and project folders are refused.
- Symlinks, junctions, hidden items and cloud-only placeholders are never followed or hashed.
- Project folders (.git, node_modules, .venv, package.json, *.sln) are never entered.
- Active downloads and files changed in the last 5 minutes are left alone.
- Removals only happen on local fixed drives, where the Recycle Bin exists.
- Every action re-checks the disk right before touching it, because the scan may be stale.
- One failing item never aborts a batch: failures are collected and returned.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
import shutil
import stat
import time
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterator, Optional

from send2trash import send2trash

CATEGORIES = {
    "Images": {".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".svg", ".heic", ".tiff", ".ico"},
    "Documents": {".pdf", ".doc", ".docx", ".txt", ".rtf", ".odt", ".md", ".ppt", ".pptx", ".xls", ".xlsx", ".csv"},
    "Videos": {".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm"},
    "Audio": {".mp3", ".wav", ".flac", ".aac", ".ogg", ".m4a"},
    "Archives": {".zip", ".rar", ".7z", ".tar", ".gz", ".bz2"},
    "Installers": {".exe", ".msi", ".msix", ".apk", ".dmg", ".iso"},
    "Code": {".py", ".js", ".ts", ".html", ".css", ".json", ".java", ".c", ".cpp", ".cs", ".ipynb", ".sql"},
}
EXT_TO_CAT = {ext: cat for cat, exts in CATEGORIES.items() for ext in exts}

PROTECTED_DIRS = {
    "windows", "program files", "program files (x86)", "programdata",
    "$recycle.bin", "system volume information", ".git", "node_modules", "appdata",
}

PROJECT_MARKERS = {".git", "node_modules", ".venv", "package.json"}  # plus any *.sln
PARTIAL_EXTS = {".crdownload", ".part", ".tmp", ".download", ".opdownload", ".!qb"}
RECENT_SECS = 5 * 60
FOLDER_JUNK = {"desktop.ini", "thumbs.db"}  # Explorer's own files; a folder holding only these is empty

LOG_DIR = Path(os.environ.get("LOCALAPPDATA", Path.home() / ".config")) / "Declutter"
UNDO_LOG = LOG_DIR / "undo.json"
OLD_LOG_DIR = Path(os.environ.get("APPDATA", Path.home() / ".config")) / "Declutter"  # before 0.2
DRIVE_FIXED = 3  # GetDriveTypeW

_HIDDEN = 0x2
_CLOUD_ONLY = 0x1000 | 0x40000 | 0x400000  # OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS
_LINK_TAGS = {0xA000000C, 0xA0000003}      # symlink, junction (mount point)
_PARTIAL = 64 * 1024                       # bytes hashed in the cheap first pass
_CHUNK = 1024 * 1024

ProgressFn = Optional[Callable[[str], None]]
CancelFn = Optional[Callable[[], bool]]
Entry = tuple[Path, os.stat_result]


# ---------- helpers ----------

def human_size(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024 or unit == "TB":
            return f"{n:.0f} {unit}" if unit == "B" else f"{n:.1f} {unit}"
        n /= 1024
    return f"{n} B"


def check_root(path: str | Path) -> tuple[bool, str]:
    """Return (ok, reason). Refuses locations where bulk operations are risky."""
    try:
        p = Path(path).resolve()
    except OSError as e:
        return False, str(e)
    if not p.is_dir():
        return False, "Not a folder."
    if p.parent == p:
        return False, "Drive roots are not allowed. Pick a specific folder."
    home = Path.home().resolve()
    if p == home:
        return False, "Pick a subfolder (e.g. Downloads), not your whole home folder."
    if p in home.parents:
        return False, "That folder contains your whole profile. Pick a subfolder."
    if any(part.lower() in PROTECTED_DIRS for part in p.parts):
        return False, "System or protected folder."
    try:
        if is_project({n.lower() for n in os.listdir(p)}):
            return False, "This is a project folder (it has .git, node_modules, .venv, package.json or a .sln file)."
    except OSError as e:
        return False, str(e)
    return True, ""


def require_root(path: str | Path) -> Path:
    """Return the resolved root or raise ValueError with a plain-language reason."""
    ok, reason = check_root(path)
    if not ok:
        raise ValueError(reason)
    return Path(path).resolve()


def _attrs(st: os.stat_result) -> int:
    return getattr(st, "st_file_attributes", 0)


def _is_link_dir(st: os.stat_result) -> bool:
    return getattr(st, "st_reparse_tag", 0) in _LINK_TAGS


def is_project(lower_names: set[str]) -> bool:
    """True if a folder holding these (lowercased) names is a code project."""
    return any(n in PROJECT_MARKERS or n.endswith(".sln") for n in lower_names)


def _leave_alone(name: str, st: os.stat_result, now: float) -> bool:
    """Active download, changed in the last few minutes, or an online-only cloud placeholder."""
    return (os.path.splitext(name)[1].lower() in PARTIAL_EXTS or now - st.st_mtime < RECENT_SECS
            or bool(_attrs(st) & _CLOUD_ONLY))


def _drive_root(p: str | Path) -> str:
    return os.path.splitdrive(os.path.abspath(p))[0].rstrip("\\") + "\\"


def _drive_type(p: str | Path) -> int:
    """GetDriveTypeW; anything but a local fixed disk (USB, network, CD, RAM disk) may have no Recycle Bin."""
    windll = getattr(ctypes, "windll", None)
    if windll is None:
        return DRIVE_FIXED  # shortcut: non-Windows dev runs only; the app ships for Windows
    return windll.kernel32.GetDriveTypeW(ctypes.c_wchar_p(_drive_root(p)))


def is_hidden(p: Path | str) -> bool:
    """Dot-name or Windows hidden attribute."""
    p = Path(p)
    if p.name.startswith("."):
        return True
    try:
        return bool(_attrs(os.lstat(p)) & _HIDDEN)
    except OSError:
        return False


def _inside(p: Path, root: Path) -> bool:
    try:
        Path(os.path.abspath(p)).relative_to(root)
        return True
    except ValueError:
        return False


def _pin_keys(pins) -> frozenset[str]:
    return frozenset(os.path.normcase(os.path.abspath(p)) for p in pins)


def is_pinned(p: str | Path, pins) -> bool:
    """True if p is a pinned ("never touch") file or folder, or lies inside one."""
    keys = _pin_keys(pins)
    p = os.path.normcase(os.path.abspath(p))
    return any(p == k or p.startswith(k.rstrip("\\/") + os.sep) for k in keys)


def walk_files(root: Path, skip_dirs: set[str] | None = None,
               cancel: CancelFn = None, ignore=()) -> Iterator[Entry]:
    """Yield (path, stat) for regular files under root, in one pass.

    Skips symlinks, junctions, hidden, protected and project folders, and files to leave alone
    (downloads in progress, just-changed files, cloud placeholders). Stats come from the directory
    listing, so no extra syscalls are needed by callers.
    """
    skip = {d.lower() for d in (skip_dirs or ())}
    pins = _pin_keys(ignore)
    stack = [os.fspath(root)]
    now = time.time()
    while stack:
        if cancel and cancel():
            return
        try:
            with os.scandir(stack.pop()) as it:
                entries = list(it)
        except OSError:
            continue
        if is_project({e.name.lower() for e in entries}):
            continue
        for e in entries:
            try:
                if e.is_symlink():
                    continue
                st = e.stat(follow_symlinks=False)
            except OSError:
                continue
            if e.name.startswith(".") or _attrs(st) & _HIDDEN or os.path.normcase(e.path) in pins:
                continue
            if stat.S_ISDIR(st.st_mode):
                low = e.name.lower()
                if low in PROTECTED_DIRS or low in skip or _is_link_dir(st):
                    continue
                stack.append(e.path)
            elif stat.S_ISREG(st.st_mode) and not _leave_alone(e.name, st, now):
                yield Path(e.path), st


def scan_entries(root: str | Path, cancel: CancelFn = None, ignore=()) -> list[Entry]:
    """One walk of the folder; pass the result to plan_sort / find_duplicates / find_large_old.
    `ignore` holds pinned files and folders, which are skipped."""
    return list(walk_files(require_root(root), cancel=cancel, ignore=ignore))


def iter_files(root: Path, skip_dirs: set[str] | None = None,
               cancel: CancelFn = None) -> Iterator[Path]:
    for p, _ in walk_files(root, skip_dirs, cancel):
        yield p


def unique_path(dst: Path, taken: set[str] | None = None) -> Path:
    """First free name: 'a.txt', 'a (1).txt', ... `taken` holds lowercased paths
    already planned in this batch (Windows names are case-insensitive)."""
    def free(p: Path) -> bool:
        return not p.exists() and not (taken and str(p).lower() in taken)

    cand, i = dst, 0
    while not free(cand):
        i += 1
        cand = dst.with_name(f"{dst.stem} ({i}){dst.suffix}")
    return cand


def to_recycle_bin(paths: list[Path], root: str | Path | None = None) -> tuple[int, list[str]]:
    """Send paths to the Recycle Bin. With `root`, anything outside it is refused."""
    base = require_root(root) if root is not None else None
    ok, errors = 0, []
    for p in paths:
        try:
            if base is not None and (not _inside(p, base) or Path(os.path.abspath(p)) == base):
                errors.append(f"{p.name}: outside the chosen folder, skipped")
            elif os.path.islink(p):
                errors.append(f"{p.name}: is a link, skipped")
            elif not p.exists():
                continue
            elif _drive_type(p) != DRIVE_FIXED:
                errors.append(f"{p.name}: on a removable or network drive, which may have no Recycle Bin, skipped")
            elif p.is_file() and _leave_alone(p.name, os.lstat(p), time.time()):
                errors.append(f"{p.name}: still downloading, just changed or online-only, skipped")
            else:
                send2trash(os.path.abspath(p))
                ok += 1
        except Exception as e:  # send2trash raises several OS-specific errors
            errors.append(f"{p.name}: {e}")
    return ok, errors


# ---------- 1. sort by type ----------

@dataclass
class Move:
    src: Path
    dst: Path
    size: int = 0

    @property
    def category(self) -> str:
        return self.dst.parent.name


def plan_sort(root: str | Path, cancel: CancelFn = None,
              entries: list[Entry] | None = None) -> list[Move]:
    """Loose files directly in root only; subfolders are never touched."""
    root = require_root(root)
    if entries is None:
        entries = list(walk_files(root, cancel=cancel))
    moves: list[Move] = []
    taken: set[str] = set()  # destinations already planned in this batch
    for src, st in sorted(entries, key=lambda e: str(e[0])):
        if src.parent != root:
            continue
        cat = EXT_TO_CAT.get(src.suffix.lower(), "Other")
        dst = unique_path(root / cat / src.name, taken)
        taken.add(str(dst).lower())
        moves.append(Move(src, dst, st.st_size))
    return moves


def apply_moves(moves: list[Move], root: str | Path | None = None) -> tuple[int, list[str]]:
    base = require_root(root) if root is not None else None
    done: list[Move] = []
    errors: list[str] = []
    for m in moves:
        try:
            if base is not None and not (_inside(m.src, base) and _inside(m.dst, base)):
                errors.append(f"{m.src.name}: outside the chosen folder, skipped")
                continue
            st = os.lstat(m.src)  # the scan may be stale: re-check it is still a plain file
            if not stat.S_ISREG(st.st_mode):
                errors.append(f"{m.src.name}: is no longer a regular file, skipped")
                continue
            if _leave_alone(m.src.name, st, time.time()):
                errors.append(f"{m.src.name}: still downloading, just changed or online-only, skipped")
                continue
            m.dst.parent.mkdir(parents=True, exist_ok=True)
            final = unique_path(m.dst)
            shutil.move(str(m.src), str(final))
            done.append(Move(m.src, final))
        except Exception as e:
            errors.append(f"{m.src.name}: {e}")
    if done:
        try:
            _append_log(done)
        except Exception as e:
            errors.append(f"The undo log could not be saved ({e}), so this sort cannot be undone from the app.")
    return len(done), errors


def _write_log(batches: list[dict]) -> None:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    tmp = UNDO_LOG.with_suffix(".tmp")
    tmp.write_text(json.dumps(batches, indent=1), encoding="utf-8")
    os.replace(tmp, UNDO_LOG)  # atomic: a crash never leaves a half-written log


def _append_log(done: list[Move]) -> None:
    batches = _read_log()
    batches.append({
        "time": time.strftime("%Y-%m-%d %H:%M:%S"),
        "moves": [{"src": str(m.src), "dst": str(m.dst)} for m in done],
    })
    _write_log(batches[-20:])


def migrate(old: Path, new: Path) -> None:
    """Move a data file from its pre-0.2 %APPDATA% home once; if that fails it just stays there."""
    if old != new and old.is_file() and not new.exists():
        try:
            new.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(old), str(new))
        except OSError:
            pass


def _read_log() -> list[dict]:
    migrate(OLD_LOG_DIR / UNDO_LOG.name, UNDO_LOG)
    try:
        data = json.loads(UNDO_LOG.read_text(encoding="utf-8"))
        return data if isinstance(data, list) else []
    except (OSError, ValueError):
        return []


def can_undo() -> bool:
    return bool(_read_log())


def undo_count() -> int:
    """Number of moves in the batch "Undo last sort" would reverse."""
    batches = _read_log()
    return len(batches[-1].get("moves", [])) if batches and isinstance(batches[-1], dict) else 0


def _log_path_ok(p: Path) -> bool:
    """The log is a plain file a user (or malware) could edit: never trust it blindly."""
    return p.is_absolute() and not any(part.lower() in PROTECTED_DIRS for part in p.parts[1:])


def undo_last() -> tuple[int, list[str]]:
    """Reverse the most recent sort batch. Skips files that changed since."""
    batches = _read_log()
    if not batches:
        return 0, ["Nothing to undo."]
    batch = batches.pop()
    restored, errors = 0, []
    for mv in reversed(batch.get("moves", [])):
        try:
            src, dst = Path(mv["src"]), Path(mv["dst"])
        except (KeyError, TypeError):
            errors.append("Skipped a damaged log entry.")
            continue
        try:
            if not (_log_path_ok(src) and _log_path_ok(dst)):
                errors.append(f"{dst.name}: location not allowed, skipped")
            elif os.path.islink(dst) or not dst.is_file():
                errors.append(f"{dst.name}: no longer at {dst.parent}")
            else:
                src.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(dst), str(unique_path(src)))
                restored += 1
                try:
                    dst.parent.rmdir()  # remove category folder if now empty
                except OSError:
                    pass
        except Exception as e:
            errors.append(f"{dst.name}: {e}")
    _write_log(batches)
    return restored, errors


# ---------- 2. duplicates ----------

@dataclass
class DupGroup:
    size: int
    files: list[Path]  # oldest-modified first; index 0 is the suggested keeper


def _hash(path: Path, limit: int | None, cancel: CancelFn) -> str | None:
    """SHA-256 of the first `limit` bytes (all if None). None on error or cancel."""
    h = hashlib.sha256()
    left = limit
    try:
        with open(path, "rb") as f:
            while left is None or left > 0:
                if cancel and cancel():
                    return None
                block = f.read(_CHUNK if left is None else min(_CHUNK, left))
                if not block:
                    break
                h.update(block)
                if left is not None:
                    left -= len(block)
    except OSError:
        return None
    return h.hexdigest()


def _hash_many(paths: list[Path], limit: int | None, cancel: CancelFn) -> dict[Path, str | None]:
    if not paths:
        return {}
    with ThreadPoolExecutor(max_workers=4) as ex:  # I/O bound; hashlib releases the GIL
        return dict(zip(paths, ex.map(lambda p: _hash(p, limit, cancel), paths)))


def find_duplicates(root: str | Path, min_size: int = 1, progress: ProgressFn = None,
                    cancel: CancelFn = None, entries: list[Entry] | None = None) -> list[DupGroup]:
    """Group by size, then first 64 KB, then full SHA-256. Cloud-only files are never read."""
    root = require_root(root)
    if entries is None:
        entries = list(walk_files(root, cancel=cancel))
    mtime: dict[Path, float] = {}
    by_size: dict[int, list[Path]] = defaultdict(list)
    for p, st in entries:
        if st.st_size >= min_size and not _attrs(st) & _CLOUD_ONLY:
            by_size[st.st_size].append(p)
            mtime[p] = st.st_mtime
    sized = {s: fs for s, fs in by_size.items() if len(fs) > 1}

    if progress:
        progress(f"Comparing {sum(len(f) for f in sized.values())} files of equal size…")
    first = _hash_many([p for fs in sized.values() for p in fs], _PARTIAL, cancel)
    by_first: dict[tuple[int, str], list[Path]] = defaultdict(list)
    for size, fs in sized.items():
        for p in fs:
            if first.get(p):
                by_first[(size, first[p])].append(p)
    by_first = {k: v for k, v in by_first.items() if len(v) > 1}

    big = [p for (size, _), fs in by_first.items() if size > _PARTIAL for p in fs]
    if progress and big:
        progress(f"Checking {len(big)} possible duplicates in full…")
    full = _hash_many(big, None, cancel)

    groups: list[DupGroup] = []
    for (size, fh), fs in by_first.items():
        if size <= _PARTIAL:  # the first pass already covered the whole file
            buckets = {fh: fs}
        else:
            buckets = defaultdict(list)
            for p in fs:
                if full.get(p):
                    buckets[full[p]].append(p)
        for same in buckets.values():
            if len(same) > 1:
                same.sort(key=lambda p: (mtime[p], str(p)))
                groups.append(DupGroup(size, same))
    if cancel and cancel():
        return []
    groups.sort(key=lambda g: g.size * (len(g.files) - 1), reverse=True)
    return groups


def check_dup_selection(groups: list[DupGroup], selected: set[Path]) -> None:
    """Raise ValueError if any group would lose every copy."""
    for g in groups:
        if all(f in selected for f in g.files):
            raise ValueError(f"All {len(g.files)} copies of {g.files[0].name} are selected. "
                             "Keep at least one.")


def trash_duplicates(removals: list[tuple[DupGroup, list[Path]]],
                     root: str | Path) -> tuple[int, list[str]]:
    """Recycle the chosen copies, but only where a kept copy still exists on disk right now."""
    ok, errors = 0, []
    for group, remove in removals:
        try:
            gone = set(remove)
            keepers = [f for f in group.files if f not in gone and f.is_file()
                       and not os.path.islink(f) and f.stat().st_size == group.size]
            if not keepers:
                errors.append(f"{group.files[0].name}: no intact copy would remain, skipped")
                continue
            n, errs = to_recycle_bin(remove, root)
            ok += n
            errors += errs
        except Exception as e:
            errors.append(f"{group.files[0].name}: {e}")
    return ok, errors


# ---------- 3. large and old files ----------

@dataclass
class FileInfo:
    path: Path
    size: int
    mtime: float

    @property
    def age_days(self) -> int:
        return int((time.time() - self.mtime) / 86400)


def find_large_old(root: str | Path, min_mb: float = 100, older_days: int = 0,
                   cancel: CancelFn = None, entries: list[Entry] | None = None,
                   exts: set[str] | None = None) -> list[FileInfo]:
    """Files with size >= min_mb AND age >= older_days. A value of 0 disables that filter.
    With `exts` (lowercase, e.g. CATEGORIES["Installers"]) only those file types are listed."""
    root = require_root(root)
    if entries is None:
        entries = list(walk_files(root, cancel=cancel))
    cutoff = time.time() - older_days * 86400
    out = [FileInfo(p, st.st_size, st.st_mtime) for p, st in entries
           if not (min_mb and st.st_size < min_mb * 1024 * 1024)
           and not (older_days and st.st_mtime > cutoff)
           and (exts is None or p.suffix.lower() in exts)]
    out.sort(key=lambda x: x.size, reverse=True)
    return out


# ---------- 4. empty folders ----------

def find_empty_dirs(root: str | Path, cancel: CancelFn = None, ignore=()) -> list[Path]:
    """Folders containing no files, directly or in any subfolder. Root itself excluded.
    desktop.ini and Thumbs.db do not count as files.

    Hidden, protected, linked, pinned and project folders are never reported and count as content,
    so their parents are never reported either.
    """
    root = require_root(root)
    root_s = os.fspath(root)
    pins = _pin_keys(ignore)
    empty: list[Path] = []

    def visit(d: str) -> bool:
        if cancel and cancel():
            return False
        try:
            with os.scandir(d) as it:
                entries = list(it)
        except OSError:
            return False
        if is_project({e.name.lower() for e in entries}):
            return False
        is_empty = True
        for e in entries:
            try:
                st = e.stat(follow_symlinks=False)
                if stat.S_ISREG(st.st_mode) and e.name.lower() in FOLDER_JUNK:
                    continue
                plain_dir = (stat.S_ISDIR(st.st_mode) and not e.is_symlink() and not _is_link_dir(st)
                             and not e.name.startswith(".") and not _attrs(st) & _HIDDEN
                             and e.name.lower() not in PROTECTED_DIRS and os.path.normcase(e.path) not in pins)
            except OSError:
                plain_dir = False
            if not plain_dir or not visit(e.path):
                is_empty = False
        if is_empty and d != root_s:
            empty.append(Path(d))
        return is_empty

    visit(root_s)
    return sorted(empty, key=lambda p: len(str(p)), reverse=True)  # deepest first


def _still_empty(d: Path) -> bool:
    for _, dirs, files in os.walk(d):
        if is_project({n.lower() for n in dirs + files}) or any(f.lower() not in FOLDER_JUNK for f in files):
            return False
    return True


def trash_empty_dirs(paths: list[Path], root: str | Path) -> tuple[int, list[str]]:
    """Recycle folders, re-checking each is still empty (a file may have arrived since the scan).
    A folder holding only desktop.ini / Thumbs.db goes to the Bin whole, those files included."""
    ok, errors = 0, []
    for p in paths:
        try:
            if not p.exists():
                continue  # a parent was already removed
            if not _still_empty(p):
                errors.append(f"{p.name}: now contains files or is a project folder, skipped")
                continue
            n, errs = to_recycle_bin([p], root)
            ok += n
            errors += errs
        except Exception as e:
            errors.append(f"{p.name}: {e}")
    return ok, errors
