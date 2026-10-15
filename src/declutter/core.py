"""Declutter core logic. No GUI imports here so it can be tested on its own.

Safety rules:
- Nothing is ever permanently deleted: removals go to the Recycle Bin.
- Sorting moves are logged so the last batch can be undone.
- Drive roots, the home folder (and its parents) and system folders are refused.
- Symlinks, junctions, hidden items and cloud-only placeholders are never followed or hashed.
- Every action re-checks the disk right before touching it, because the scan may be stale.
"""
from __future__ import annotations

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
CATEGORY_NAMES = set(CATEGORIES) | {"Other"}
_CATEGORY_LOWER = {c.lower() for c in CATEGORY_NAMES}

PROTECTED_DIRS = {
    "windows", "program files", "program files (x86)", "programdata",
    "$recycle.bin", "system volume information", ".git", "node_modules", "appdata",
}

LOG_DIR = Path(os.environ.get("APPDATA", Path.home() / ".config")) / "Declutter"
UNDO_LOG = LOG_DIR / "undo.json"

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


def walk_files(root: Path, skip_dirs: set[str] | None = None,
               cancel: CancelFn = None) -> Iterator[Entry]:
    """Yield (path, stat) for regular files under root, in one pass.

    Skips symlinks, junctions, hidden and protected folders. Stats come from the directory
    listing, so no extra syscalls are needed by callers.
    """
    skip = {d.lower() for d in (skip_dirs or ())}
    stack = [os.fspath(root)]
    while stack:
        if cancel and cancel():
            return
        try:
            it = os.scandir(stack.pop())
        except OSError:
            continue
        with it:
            for e in it:
                try:
                    if e.is_symlink():
                        continue
                    st = e.stat(follow_symlinks=False)
                except OSError:
                    continue
                if e.name.startswith(".") or _attrs(st) & _HIDDEN:
                    continue
                if stat.S_ISDIR(st.st_mode):
                    low = e.name.lower()
                    if low in PROTECTED_DIRS or low in skip or _is_link_dir(st):
                        continue
                    stack.append(e.path)
                elif stat.S_ISREG(st.st_mode):
                    yield Path(e.path), st


def scan_entries(root: str | Path, cancel: CancelFn = None) -> list[Entry]:
    """One walk of the folder; pass the result to plan_sort / find_duplicates / find_large_old."""
    return list(walk_files(require_root(root), cancel=cancel))


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
            elif p.exists():
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


def plan_sort(root: str | Path, recursive: bool = False, cancel: CancelFn = None,
              entries: list[Entry] | None = None) -> list[Move]:
    root = require_root(root)
    if entries is None:
        entries = list(walk_files(root, cancel=cancel))
    moves: list[Move] = []
    taken: set[str] = set()  # destinations already planned in this batch
    for src, st in sorted(entries, key=lambda e: str(e[0])):
        if recursive:
            try:
                parents = src.relative_to(root).parts[:-1]
            except ValueError:
                continue
            if any(part.lower() in _CATEGORY_LOWER for part in parents):
                continue  # already inside a type folder
        elif src.parent != root:
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
            m.dst.parent.mkdir(parents=True, exist_ok=True)
            final = unique_path(m.dst)
            shutil.move(str(m.src), str(final))
            done.append(Move(m.src, final))
        except Exception as e:
            errors.append(f"{m.src.name}: {e}")
    if done:
        _append_log(done)
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


def _read_log() -> list[dict]:
    try:
        data = json.loads(UNDO_LOG.read_text(encoding="utf-8"))
        return data if isinstance(data, list) else []
    except (OSError, ValueError):
        return []


def can_undo() -> bool:
    return bool(_read_log())


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
        gone = set(remove)
        keepers = [f for f in group.files if f not in gone and f.is_file()
                   and not os.path.islink(f) and f.stat().st_size == group.size]
        if not keepers:
            errors.append(f"{group.files[0].name}: no intact copy would remain, skipped")
            continue
        n, errs = to_recycle_bin(remove, root)
        ok += n
        errors += errs
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
                   cancel: CancelFn = None, entries: list[Entry] | None = None) -> list[FileInfo]:
    """Files with size >= min_mb AND age >= older_days. A value of 0 disables that filter."""
    root = require_root(root)
    if entries is None:
        entries = list(walk_files(root, cancel=cancel))
    cutoff = time.time() - older_days * 86400
    out = [FileInfo(p, st.st_size, st.st_mtime) for p, st in entries
           if not (min_mb and st.st_size < min_mb * 1024 * 1024)
           and not (older_days and st.st_mtime > cutoff)]
    out.sort(key=lambda x: x.size, reverse=True)
    return out


# ---------- 4. empty folders ----------

def find_empty_dirs(root: str | Path, cancel: CancelFn = None) -> list[Path]:
    """Folders containing no files, directly or in any subfolder. Root itself excluded.

    Hidden, protected and linked folders are never reported and count as content,
    so their parents are never reported either.
    """
    root = require_root(root)
    root_s = os.fspath(root)
    empty: list[Path] = []

    def visit(d: str) -> bool:
        if cancel and cancel():
            return False
        try:
            it = os.scandir(d)
        except OSError:
            return False
        is_empty = True
        with it:
            for e in it:
                try:
                    st = e.stat(follow_symlinks=False)
                    plain_dir = (stat.S_ISDIR(st.st_mode) and not e.is_symlink() and not _is_link_dir(st)
                                 and not e.name.startswith(".") and not _attrs(st) & _HIDDEN
                                 and e.name.lower() not in PROTECTED_DIRS)
                except OSError:
                    plain_dir = False
                if not plain_dir or not visit(e.path):
                    is_empty = False
        if is_empty and d != root_s:
            empty.append(Path(d))
        return is_empty

    visit(root_s)
    return sorted(empty, key=lambda p: len(str(p)), reverse=True)  # deepest first


def _has_files(d: Path) -> bool:
    return any(names for _, _, names in os.walk(d))


def trash_empty_dirs(paths: list[Path], root: str | Path) -> tuple[int, list[str]]:
    """Recycle folders, re-checking each is still empty (a file may have arrived since the scan)."""
    ok, errors = 0, []
    for p in paths:
        if not p.exists():
            continue  # a parent was already removed
        if _has_files(p):
            errors.append(f"{p.name}: now contains files, skipped")
            continue
        n, errs = to_recycle_bin([p], root)
        ok += n
        errors += errs
    return ok, errors
