"""Generate golden fixtures (tests/golden/*.json) from the Python implementation in core.py.

The fixtures are the behaviour contract for the Rust port (docs/CONTRACT.md). Windows only: file attributes and
creation times are set with the Win32 API. Run:  python tools/gen_golden.py [out_dir]
"""
from __future__ import annotations

import ctypes
import json
import os
import shutil
import sys
import tempfile
import time
import zipfile
from contextlib import ExitStack
from ctypes import wintypes
from pathlib import Path
from unittest import mock

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "src"))

from declutter import core  # noqa: E402

DAY, MB = 86400, 1024 * 1024
OLD = 10 * DAY  # default age: well clear of every threshold (5 min, 3 d, 7 d)
SCHEMA = 1

_k32 = ctypes.WinDLL("kernel32", use_last_error=True)
_k32.CreateFileW.restype = wintypes.HANDLE
_k32.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, wintypes.LPVOID,
                             wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
_k32.SetFileTime.argtypes = [wintypes.HANDLE, wintypes.LPVOID, wintypes.LPVOID, wintypes.LPVOID]
_k32.CloseHandle.argtypes = [wintypes.HANDLE]
_k32.GetFileAttributesW.argtypes = [wintypes.LPCWSTR]
_k32.SetFileAttributesW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD]


# ---------- tree materialisation ----------

def _filetime(t: float) -> wintypes.FILETIME:
    v = int((t + 11644473600) * 10_000_000)
    return wintypes.FILETIME(v & 0xFFFFFFFF, v >> 32)


def set_times(path: Path, mtime: float, created: float) -> None:
    h = _k32.CreateFileW(str(path), 0x100, 7, None, 3, 0x02000000, None)
    if h in (None, wintypes.HANDLE(-1).value):
        raise OSError(f"CreateFileW failed for {path}")
    try:
        c, m = _filetime(created), _filetime(mtime)
        if not _k32.SetFileTime(h, ctypes.byref(c), ctypes.byref(m), ctypes.byref(m)):
            raise OSError(f"SetFileTime failed for {path}")
    finally:
        _k32.CloseHandle(h)


def add_attrs(path: Path, attrs: list[str]) -> None:
    bits = {"hidden": 0x2, "offline": 0x1000}
    cur = _k32.GetFileAttributesW(str(path))
    for a in attrs:
        cur |= bits[a]
    if not _k32.SetFileAttributesW(str(path), cur):
        raise OSError(f"SetFileAttributesW failed for {path}")


def content_of(f: dict) -> bytes:
    if "segments" in f:
        return b"".join(c.encode() * n for c, n in f["segments"])
    if "text" in f:
        return f["text"].encode()
    return f.get("fill", "a").encode() * f.get("size", 10)


def materialize(tree: dict, root: Path, now: float) -> None:
    root.mkdir(parents=True, exist_ok=True)
    for d in tree.get("dirs", []):
        (root / d).mkdir(parents=True, exist_ok=True)
    for f in tree.get("files", []):
        p = root / f["path"]
        p.parent.mkdir(parents=True, exist_ok=True)
        if "zip" in f:
            with zipfile.ZipFile(p, "w") as z:
                for name, text in f["zip"].items():
                    z.writestr(name, text)
        elif f.get("corrupt"):
            p.write_bytes(b"this is not a zip archive")
        else:
            p.write_bytes(content_of(f))
    for f in tree.get("files", []):
        p = root / f["path"]
        age = f.get("age_s", OLD)
        set_times(p, now - age, now - f.get("created_age_s", age))
    for f in tree.get("files", []):
        if f.get("attrs"):
            add_attrs(root / f["path"], f["attrs"])
    for d, attrs in tree.get("dir_attrs", {}).items():
        add_attrs(root / d, attrs)


def snapshot(root: Path) -> dict:
    files, dirs = [], []
    for here, ds, fs in os.walk(root):
        for d in ds:
            dirs.append(os.path.relpath(Path(here, d), root).replace("\\", "/"))
        for f in fs:
            files.append(os.path.relpath(Path(here, f), root).replace("\\", "/"))
    return {"files": sorted(files), "dirs": sorted(dirs)}


# ---------- scenario runner ----------

class Ctx:
    def __init__(self, tmp: Path, spec: dict):
        self.tmp, self.spec = tmp, spec
        self.root = (tmp / "work").resolve()

    def rel(self, p) -> str:
        return os.path.relpath(p, self.root).replace("\\", "/")

    def path(self, r: str) -> Path:
        return Path(os.path.normpath(self.root / r))

    def norm(self, s: str) -> str:
        return s.replace(str(self.root), "<root>")

    def result(self, res: tuple[int, list[str]]) -> dict:
        return {"count": res[0], "errors": [self.norm(e) for e in res[1]]}


def _patches(ctx: Ctx, stack: ExitStack) -> None:
    p = ctx.spec.get("params", {})
    logs = ctx.tmp / "logs"
    stack.enter_context(mock.patch.object(core, "LOG_DIR", logs))
    stack.enter_context(mock.patch.object(core, "UNDO_LOG", logs / "undo.json"))
    stack.enter_context(mock.patch.object(core, "OLD_LOG_DIR", ctx.tmp / "oldlogs"))
    stack.enter_context(mock.patch.object(core, "_drive_type", lambda _p: p.get("drive_type", core.DRIVE_FIXED)))
    dl = p.get("downloads")
    ids = core.KNOWN_FOLDERS["Downloads"]
    stack.enter_context(mock.patch.object(
        core, "_known_folder_path", lambda fid: (ctx.root / dl) if (dl and fid == ids) else None))
    bin_dir = ctx.tmp / "bin"
    bin_dir.mkdir(exist_ok=True)
    stack.enter_context(mock.patch.object(
        core, "send2trash", lambda path: shutil.move(path, str(core.unique_path(bin_dir / Path(path).name)))))


def op_scan(ctx):
    p = ctx.spec.get("params", {})
    skipped: list[Path] = []
    ents = core.scan_entries(ctx.root, ignore=[ctx.path(i) for i in p.get("ignore", [])], skipped=skipped)
    return {"files": sorted(ctx.rel(e[0]) for e in ents), "skipped_dirs": sorted(ctx.rel(s) for s in skipped)}


def op_plan_sort(ctx):
    plan = core.plan_sort(ctx.root, recent_hours=ctx.spec.get("params", {}).get("recent_hours", 0))
    return {"moves": [{"src": ctx.rel(m.src), "dst": ctx.rel(m.dst), "size": m.size, "category": m.category}
                      for m in plan], "recent_left": plan.recent_left}


def op_duplicates(ctx):
    gs = core.find_duplicates(ctx.root, min_size=ctx.spec.get("params", {}).get("min_size", 1))
    return {"groups": [{"size": g.size, "files": [ctx.rel(f) for f in g.files]} for g in gs]}


def op_large_old(ctx):
    p = ctx.spec["params"]
    out = core.find_large_old(ctx.root, p["min_mb"], p["older_days"])
    return {"files": [{"path": ctx.rel(f.path), "size": f.size} for f in out]}


def op_leftovers(ctx):
    out = core.find_leftovers(ctx.root)
    return {"items": [{"path": ctx.rel(x.path), "size": x.size, "reason": x.reason} for x in out]}


def op_redownloads(ctx):
    excl = [ctx.path(e) for e in ctx.spec.get("params", {}).get("exclude", [])]
    out = core.find_redownloads(ctx.root, exclude=excl)
    return {"pairs": [{"original": ctx.rel(r.original), "copy": ctx.rel(r.copy),
                       "orig_size": r.orig_size, "copy_size": r.copy_size} for r in out]}


def op_empty_dirs(ctx):
    ignore = [ctx.path(i) for i in ctx.spec.get("params", {}).get("ignore", [])]
    return {"dirs": [ctx.rel(d) for d in core.find_empty_dirs(ctx.root, ignore=ignore)]}


def op_check_root(ctx):
    home = Path.home().resolve()
    tokens = {"$DRIVE_ROOT": Path(home.anchor), "$HOME": home, "$HOME_PARENT": home.parent}
    results = []
    for case in ctx.spec["params"]["cases"]:
        path = tokens[case] if case in tokens else ctx.path(case)
        ok, reason = core.check_root(path)
        results.append({"case": case, "ok": ok, "reason": reason})
    return {"results": results}


def _undo_log(ctx) -> list:
    try:
        raw = json.loads(core.UNDO_LOG.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    if not isinstance(raw, list):
        return []
    # the log may be hand-edited (see undo-tampered-log): keep whatever keys each entry has
    return [{"moves": [{k: (ctx.rel(v) if os.path.isabs(v) and not v.startswith("C:\\Windows") else v)
                        for k, v in m.items()} for m in b["moves"]],
             "time_is_timestamp": len(b["time"]) == 19} for b in raw]


def _act(ctx, action: dict) -> None:
    kind, path = action["do"], ctx.path(action["path"])
    if kind == "delete":
        os.remove(path)
    elif kind == "write":
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content_of(action))
    elif kind == "truncate":
        path.write_bytes(path.read_bytes()[:action["size"]])
    elif kind == "mkdir":
        path.mkdir(parents=True, exist_ok=True)
    else:
        raise ValueError(kind)


def op_undo(ctx):
    p = ctx.spec.get("params", {})
    out: dict = {}
    if "raw_log" in p:  # a hand-written or damaged log; <root> is replaced by the work root
        core.LOG_DIR.mkdir(parents=True, exist_ok=True)
        core.UNDO_LOG.write_text(json.dumps(p["raw_log"]).replace("<root>", str(ctx.root).replace("\\", "\\\\")),
                                 encoding="utf-8")
    for _ in range(p.get("rounds", 1 if "raw_log" not in p else 0)):
        plan = core.plan_sort(ctx.root, recent_hours=p.get("recent_hours", 0))
        moves = plan[:p["move_first"]] if "move_first" in p else list(plan)
        out.setdefault("applies", []).append(ctx.result(core.apply_moves(moves, ctx.root)))
        for a in p.get("between", []):
            _act(ctx, a)
    if p.get("apply_rounds"):  # one file per round, to exercise the 20-batch cap
        for i in range(p["apply_rounds"]):
            (ctx.root / f"f{i:02d}.txt").write_text(str(i))
            set_times(ctx.root / f"f{i:02d}.txt", time.time() - OLD, time.time() - OLD)
            out.setdefault("applies", []).append(ctx.result(core.apply_moves(core.plan_sort(ctx.root), ctx.root)))
    out["log_after_apply"] = _undo_log(ctx)
    out["tree_after_apply"] = snapshot(ctx.root)
    for a in p.get("after_apply", []):
        _act(ctx, a)
    out["can_undo"] = core.can_undo()
    out["undo_count"] = core.undo_count()
    out["undo"] = ctx.result(core.undo_last())
    out["tree_after_undo"] = snapshot(ctx.root)
    out["log_len_after_undo"] = len(_undo_log(ctx))
    return out


def op_recycle(ctx):
    p = ctx.spec["params"]
    paths = [ctx.path(x) for x in p["paths"]]
    res = core.to_recycle_bin(paths, ctx.root if p.get("with_root", True) else None)
    return {"result": ctx.result(res), "tree": snapshot(ctx.root)}


def op_trash_duplicates(ctx):
    p = ctx.spec["params"]
    groups = core.find_duplicates(ctx.root)
    for a in p.get("after_scan", []):
        _act(ctx, a)
    removals = [(groups[gi], [groups[gi].files[fi] for fi in fis]) for gi, fis in p["remove"]]
    return {"groups": [[ctx.rel(f) for f in g.files] for g in groups],
            "result": ctx.result(core.trash_duplicates(removals, ctx.root)), "tree": snapshot(ctx.root)}


def op_check_dup_selection(ctx):
    groups = core.find_duplicates(ctx.root)
    out = []
    for sel in ctx.spec["params"]["selections"]:
        chosen = {ctx.path(x) for x in sel}
        try:
            core.check_dup_selection(groups, chosen)
            out.append({"selection": sel, "raises": False, "message": ""})
        except ValueError as e:
            out.append({"selection": sel, "raises": True, "message": str(e)})
    return {"results": out}


def op_trash_empty(ctx):
    p = ctx.spec["params"]
    dirs = core.find_empty_dirs(ctx.root)
    for a in p.get("after_scan", []):
        _act(ctx, a)
    res = core.trash_empty_dirs(dirs, ctx.root)
    return {"scanned": [ctx.rel(d) for d in dirs], "result": ctx.result(res), "tree": snapshot(ctx.root)}


def op_trash_redownloads(ctx):
    p = ctx.spec["params"]
    pairs = core.find_redownloads(ctx.root)
    for a in p.get("after_scan", []):
        _act(ctx, a)
    removing = [ctx.path(x) for x in p.get("removing", [])]
    res = core.trash_redownloads(pairs, ctx.root, removing=removing)
    return {"pairs": [{"original": ctx.rel(r.original), "copy": ctx.rel(r.copy)} for r in pairs],
            "result": ctx.result(res), "tree": snapshot(ctx.root)}


OPS = {k[3:]: v for k, v in globals().items() if k.startswith("op_")}


def run(spec: dict) -> dict:
    scratch = ROOT_DIR / ".golden_tmp"  # not %TEMP%: that lives under AppData, which check_root refuses
    scratch.mkdir(exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix="run-", dir=scratch)).resolve()
    try:
        ctx = Ctx(tmp, spec)
        now = time.time()
        materialize(spec["tree"], ctx.root, now)
        for extra in spec.get("outside", []):  # files beside the root, e.g. "../outside.txt"
            materialize({"files": [extra]}, ctx.root, now)
        with ExitStack() as stack:
            _patches(ctx, stack)
            expected = OPS[spec["op"]](ctx)
        return {"schema": SCHEMA, "name": spec["name"], "description": spec["description"], "op": spec["op"],
                "params": spec.get("params", {}), "tree": spec["tree"], "expected": expected}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


# ---------- scenarios ----------

def F(path, **kw):
    return {"path": path, **kw}


def S(name, op, description, files=(), dirs=(), params=None, dir_attrs=None, outside=()):
    tree = {"files": list(files), "dirs": list(dirs)}
    if dir_attrs:
        tree["dir_attrs"] = dir_attrs
    spec = {"name": name, "op": op, "description": description, "tree": tree}
    if params is not None:
        spec["params"] = params
    if outside:
        spec["outside"] = list(outside)
    return spec


def big(tag: str, mid: str, n_end: int = 70000) -> list:
    """Content with the same first and last 64 KiB for any `mid`, so only a full hash tells copies apart."""
    return [[tag, n_end], [mid, 50], [tag, n_end]]


SCENARIOS = [
    # --- scan ---
    S("scan-skips-hidden-dot-protected", "scan",
      "Hidden (dot-name and attribute), protected-name folders and hidden folders are never entered.",
      files=[F("a.txt"), F(".dot.txt"), F("hid.txt", attrs=["hidden"]), F("AppData/x.txt"), F("Windows/y.txt"),
             F("$Recycle.Bin/z.txt"), F("hiddendir/f.txt"), F(".dotdir/g.txt"), F("ok/b.txt")],
      dir_attrs={"hiddendir": ["hidden"]}),
    S("scan-project-and-app-folders", "scan",
      "Project folders (.git, package.json, *.sln) and portable-app folders (.exe + .dll) below the root are skipped and reported.",
      files=[F("loose.txt"), F("proj/code.py"), F("proj2/package.json"), F("proj2/a.js"), F("slnproj/x.sln"),
             F("slnproj/c.cs"), F("app/tool.exe"), F("app/lib.dll"), F("app/readme.txt"), F("exeonly/tool.exe"),
             F("exeonly/readme.txt"), F("sub/ok.txt")],
      dirs=["proj/.git", "mixed/.venv"]),
    S("scan-leave-alone", "scan",
      "Active downloads, files changed in the last 5 minutes and online-only placeholders are left alone; old dead downloads are not.",
      files=[F("fresh.txt", age_s=60), F("recent4.txt", age_s=240), F("older6.txt", age_s=400),
             F("dead.crdownload", age_s=OLD), F("young.part", age_s=2 * DAY), F("any.tmp", age_s=OLD),
             F("x.opdownload", age_s=OLD), F("y.download", age_s=OLD), F("z.!qB", age_s=OLD),
             F("cloud.docx", attrs=["offline"]), F("old.txt")]),
    S("scan-pinned-never-touch", "scan",
      "Pinned files and folders (Never Touch) are skipped, with everything inside a pinned folder.",
      files=[F("pinned.txt"), F("Keep/inside.txt"), F("free.txt"), F("sub/Keep2/deep.txt"), F("sub/also.txt")],
      params={"ignore": ["pinned.txt", "Keep", "sub/Keep2"]}),
    S("scan-nested", "scan", "Files at any depth are found in one walk.",
      files=[F("a/b/c/d.txt"), F("a/e.txt"), F("f.txt")]),

    # --- sort ---
    S("sort-categories", "plan_sort", "Each extension maps to its category, case-insensitively; unknown and extensionless go to Other.",
      files=[F("a.JPG", size=11), F("b.pdf", size=12), F("c.Mp4", size=13), F("d.flac", size=14), F("e.zip", size=15),
             F("f.msi", size=16), F("g.py", size=17), F("h.xyz", size=18), F("noext", size=19), F("i.heic", size=20),
             F("j.csv", size=21), F("k.iso", size=22), F("l.ipynb", size=23)]),
    S("sort-image-subbuckets", "plan_sort",
      "Screenshots and chat pictures go one level deeper, matched at the start of the name only and only for images.",
      files=[F("Screenshot 2024-01-01.png"), F("SNIP.png"), F("Capture1.jpg"), F("my screenshot.png"),
             F("IMG-20240101-WA0001.jpg"), F("WhatsApp Image 2024-01-01.jpeg"), F("Screenshot notes.pdf"),
             F("photo.png")]),
    S("sort-collisions", "plan_sort",
      "A clash with an existing file gets ' (1)', then ' (2)'; case-insensitive.",
      files=[F("a.png"), F("Images/a.png"), F("Images/a (1).png"), F("b.pdf"), F("Documents/B.PDF")]),
    S("sort-skip-rules", "plan_sort",
      ".lnk/.url/.ini and dead downloads stay; files in subfolders and project subfolders are never sorted.",
      files=[F("go.lnk"), F("site.url"), F("conf.ini"), F("dead.crdownload"), F("dead2.part"), F("sub/in.txt"),
             F("proj/x.txt"), F("keep.txt"), F("hid.txt", attrs=["hidden"])],
      dirs=["proj/.git"]),
    S("sort-recent-grace", "plan_sort",
      "With recent_hours the later of modified and created time decides; skipped files are counted.",
      files=[F("fresh2h.txt", age_s=2 * 3600), F("old30h.txt", age_s=30 * 3600), F("copy.txt", age_s=OLD, created_age_s=3600),
             F("ancient.txt")],
      params={"recent_hours": 24}),
    S("sort-recent-off", "plan_sort", "recent_hours 0 moves everything that is not in the 5-minute or active-download rules.",
      files=[F("fresh2h.txt", age_s=2 * 3600), F("copy.txt", age_s=OLD, created_age_s=3600), F("fresh1m.txt", age_s=60)],
      params={"recent_hours": 0}),

    # --- duplicates ---
    S("dup-basic", "duplicates", "Identical content groups together; oldest copy is first (keeper); same size, different content is not a duplicate.",
      files=[F("a/x.bin", text="same content here", age_s=9 * DAY), F("b/y.bin", text="same content here", age_s=20 * DAY),
             F("c/z.bin", text="same content here", age_s=15 * DAY), F("d/other.bin", text="diff content here!"[:17]),
             F("e/solo.bin", text="unique and alone")]),
    S("dup-partial-vs-full", "duplicates",
      "Same size and identical first/last 64 KiB but different middle is NOT a duplicate; a true big duplicate pair is.",
      files=[F("p/a.bin", segments=big("A", "x")), F("p/b.bin", segments=big("A", "y")),
             F("q/c.bin", segments=big("B", "m"), age_s=20 * DAY), F("q/d.bin", segments=big("B", "m"), age_s=12 * DAY)]),
    S("dup-small-files", "duplicates", "Files up to 128 KiB are fully covered by the ends pass; a one-byte difference splits the group.",
      files=[F("s/a.txt", segments=[["A", 100000]], age_s=11 * DAY), F("s/b.txt", segments=[["A", 100000]], age_s=12 * DAY),
             F("s/c.txt", segments=[["A", 99999], ["B", 1]]), F("s/d.txt", segments=[["A", 99999], ["B", 1]], age_s=13 * DAY)]),
    S("dup-skip-rules", "duplicates",
      ".lnk/.ini, dead downloads, online-only files and empty files are never listed as duplicates.",
      files=[F("a.lnk", text="link!"), F("b.lnk", text="link!"), F("a.ini", text="conf"), F("b.ini", text="conf"),
             F("a.crdownload", text="partial"), F("b.crdownload", text="partial"), F("c1.dat", text="cloud!", attrs=["offline"]),
             F("c2.dat", text="cloud!", attrs=["offline"]), F("e1.txt", text=""), F("e2.txt", text=""),
             F("n1.txt", text="normal dup"), F("n2.txt", text="normal dup")]),
    S("dup-min-size", "duplicates", "min_size drops groups of smaller files.",
      files=[F("a.txt", text="tiny"), F("b.txt", text="tiny"), F("c.txt", segments=[["Z", 5000]]),
             F("d.txt", segments=[["Z", 5000]])],
      params={"min_size": 1000}),
    S("dup-ordering", "duplicates",
      "Groups sort by reclaimable size (size x copies-1) descending; equal mtimes break by path.",
      files=[F("small/s1.bin", segments=[["S", 100]], age_s=12 * DAY), F("small/s2.bin", segments=[["S", 100]], age_s=11 * DAY),
             F("mid/m1.bin", segments=[["M", 5000]], age_s=12 * DAY), F("mid/m2.bin", segments=[["M", 5000]], age_s=11 * DAY),
             F("mid/m3.bin", segments=[["M", 5000]], age_s=10 * DAY),
             F("tie/b.bin", segments=[["T", 700]], age_s=14 * DAY), F("tie/a.bin", segments=[["T", 700]], age_s=14 * DAY)]),

    # --- large and old ---
    *[S(f"large-old-{tag}", "large_old", f"Filter {desc}; sorted by size descending; dead downloads excluded.",
        files=[F("big_new.bin", size=2 * MB + 1000, age_s=DAY), F("big_old.bin", size=3 * MB, age_s=40 * DAY),
               F("small_old.bin", size=2000, age_s=60 * DAY), F("small_new.bin", size=1000, age_s=DAY),
               F("mid_old.bin", size=MB + 5, age_s=35 * DAY), F("dead.crdownload", size=4 * MB, age_s=20 * DAY)],
        params={"min_mb": mb, "older_days": days})
      for tag, desc, mb, days in (("both", "size >= 1 MB AND older than 30 days", 1, 30),
                                  ("size-only", "size >= 2 MB, age off", 2, 0),
                                  ("age-only", "older than 30 days, size off", 0, 30),
                                  ("none", "both filters off", 0, 0))],

    # --- leftovers ---
    S("leftover-zip-extracted", "leftovers",
      "A zip counts as unpacked only when the same-named folder beside it holds every file at its stored size.",
      files=[F("good.zip", zip={"a.txt": "hello", "sub/b.txt": "world!"}), F("good/a.txt", text="hello"), F("good/sub/b.txt", text="world!"),
             F("missing.zip", zip={"a.txt": "hello", "b.txt": "x"}), F("missing/a.txt", text="hello"),
             F("wrongsize.zip", zip={"a.txt": "hello"}), F("wrongsize/a.txt", text="hello!!"),
             F("slip.zip", zip={"../evil.txt": "bad"}), F("slip/ok.txt", text="x"), F("evil.txt", text="bad"),
             F("empty.zip", zip={}), F("empty/x.txt", text="x"), F("corrupt.zip", corrupt=True), F("corrupt/x.txt", text="x"),
             F("nofolder.zip", zip={"a.txt": "hello"}), F("elsewhere/nofolder/a.txt", text="hello")]),
    S("leftover-installers-and-downloads", "leftovers",
      "Installers over 7 days old inside the Downloads folder, and dead .crdownload/.part over 3 days.",
      files=[F("Downloads/setup.exe", age_s=10 * DAY, size=105), F("Downloads/tool.msi", age_s=3 * DAY, size=106),
             F("Downloads/disc.iso", age_s=8 * DAY, size=107), F("Downloads/app.msix", age_s=20 * DAY, size=108),
             F("Downloads/dead.part", age_s=4 * DAY, size=109), F("Downloads/young.crdownload", age_s=2 * DAY, size=110),
             F("Downloads/old.crdownload", age_s=5 * DAY, size=111), F("Other/setup.exe", age_s=10 * DAY, size=112),
             F("Downloads/sub/deep.exe", age_s=10 * DAY, size=113)],
      params={"downloads": "Downloads"}),

    # --- re-downloads ---
    S("redownload-pairs", "redownloads",
      "'name (N).ext' and 'name - Copy[ (N)].ext' beside the original, same type, size within 2 %.",
      files=[F("report.pdf", size=1000), F("report (1).pdf", size=1010), F("photo.jpg", size=5000),
             F("photo - Copy.jpg", size=5000), F("photo - Copy (2).jpg", size=4950), F("far.zip", size=1000),
             F("far (1).zip", size=1100), F("zero.txt", size=10), F("zero (1).txt", text=""),
             F("Doc.PDF", size=2000), F("doc (1).pdf", size=2000), F("a.lnk", size=50), F("a (1).lnk", size=50),
             F("x/note.txt", size=300), F("y/note (1).txt", size=300), F("lonely (1).txt", size=300),
             F("typed.png", size=400), F("typed (1).jpg", size=400)]),
    S("redownload-exclude", "redownloads", "Copies that are identical-content duplicates are passed as exclude and skipped.",
      files=[F("a.bin", size=1000), F("a (1).bin", size=1000), F("b.bin", size=2000), F("b (1).bin", size=2000)],
      params={"exclude": ["a (1).bin"]}),

    # --- empty folders ---
    S("empty-dirs-basic", "empty_dirs",
      "Folders with no files at any depth; desktop.ini/Thumbs.db do not count; hidden, protected, pinned and project folders count as content.",
      files=[F("junk/desktop.ini", text="x"), F("junk/Thumbs.db", text="x"), F("full/f.txt"), F("parent/full/g.txt"),
             F("hid/.secret", text="s"), F("holdhidden/inner/.x", text="s"), F("pinned/keep.txt"),
             F("holdprotected/Windows/w.txt"), F("withproj/proj/package.json", text="{}")],
      dirs=["a/b/c", "d", "holdhidden2/hiddendir", "holdpin/Keep", "holdproj/proj/.git", "holdprot/Windows", "nested/empty1/empty2"],
      dir_attrs={"holdhidden2/hiddendir": ["hidden"]},
      params={"ignore": ["holdpin/Keep"]}),

    # --- check_root ---
    S("check-root-refusals", "check_root",
      "Refused roots and their user-visible reasons.",
      files=[F("file.txt"), F("projgit/x.txt"), F("projpkg/package.json", text="{}"), F("projsln/a.sln", text="x"),
             F("projnm/f.txt"), F("Windows/sub/f.txt"), F("Program Files/f.txt"), F("ok/f.txt"), F("data/AppData/f.txt")],
      dirs=["projgit/.git", "projnm/node_modules", "empty"],
      params={"cases": ["$DRIVE_ROOT", "$HOME", "$HOME_PARENT", "file.txt", "nonexistent", "projgit", "projpkg",
                        "projsln", "projnm", "Windows/sub", "Program Files", "data/AppData", "ok", "empty", "."]}),

    # --- undo ---
    S("undo-roundtrip", "undo", "Apply a sort, check the log, undo: files return and empty category folders disappear.",
      files=[F("a.png", size=5), F("b.pdf", size=6), F("c.mp3", size=7), F("Screenshot 1.png", size=8), F("sub/keep.txt")],
      params={"rounds": 1}),
    S("undo-name-taken", "undo", "If the original name is taken again, undo restores to ' (1)'.",
      files=[F("a.png", size=5), F("b.pdf", size=6)],
      params={"rounds": 1, "after_apply": [{"do": "write", "path": "a.png", "text": "new file"}]}),
    S("undo-destination-missing", "undo", "A moved file that is gone is reported, the rest are restored.",
      files=[F("a.png", size=5), F("b.pdf", size=6), F("c.txt", size=7)],
      params={"rounds": 1, "after_apply": [{"do": "delete", "path": "Images/a.png"}]}),
    S("undo-nothing", "undo", "No log at all.", files=[F("a.txt")], params={"rounds": 0}),
    S("undo-batch-cap", "undo", "The log keeps only the newest 20 batches; undo reverses just the last one.",
      files=[], params={"rounds": 0, "apply_rounds": 22}),
    S("undo-subbucket-cleanup", "undo", "Images/Screenshots is removed, and Images too if empty; Images stays if it still has files.",
      files=[F("Screenshot a.png", size=5), F("Screenshot b.png", size=6), F("pic.png", size=7)],
      params={"rounds": 1, "after_apply": [{"do": "write", "path": "Images/extra.png", "text": "kept"}]}),
    S("undo-tampered-log", "undo", "A hand-edited log: protected location refused, damaged entry reported, good entry restored.",
      files=[F("Documents/good.pdf", text="good")],
      params={"raw_log": [{"time": "2026-01-01 10:00:00", "moves": [
          {"src": "<root>\\good.pdf", "dst": "<root>\\Documents\\good.pdf"},
          {"src": "<root>\\x.txt"},
          {"src": "C:\\Windows\\sys.dll", "dst": "C:\\Windows\\other.dll"}]}]}),
    S("undo-nonlist-log", "undo", "A log that is not a list is treated as empty.",
      files=[F("a.txt")], params={"raw_log": {"oops": True}}),

    # --- removals ---
    S("recycle-guards", "recycle",
      "Outside-root, the root itself, missing paths, left-alone files and normal files and folders.",
      files=[F("ok.txt"), F("fresh.txt", age_s=60), F("cloud.dat", attrs=["offline"]), F("dir/inner.txt")],
      outside=[F("../outside.txt")],
      params={"paths": ["ok.txt", "../outside.txt", ".", "gone.txt", "fresh.txt", "cloud.dat", "dir"]}),
    S("recycle-removable-drive", "recycle", "Anything not on a local fixed drive is refused with a plain message.",
      files=[F("a.txt"), F("b.txt")], params={"paths": ["a.txt", "b.txt"], "drive_type": 2}),
    S("trash-duplicates-guards", "trash_duplicates",
      "Removes copies only where an intact kept copy still exists; refuses a group with no keeper left.",
      files=[F("a1.bin", text="AAAA11", age_s=12 * DAY), F("a2.bin", text="AAAA11", age_s=11 * DAY),
             F("a3.bin", text="AAAA11", age_s=10 * DAY),
             F("b1.bin", text="BBBBBBBBBB", age_s=12 * DAY), F("b2.bin", text="BBBBBBBBBB", age_s=11 * DAY),
             F("c1.bin", text="CCCCCCCCCCCCCCCC", age_s=12 * DAY), F("c2.bin", text="CCCCCCCCCCCCCCCC", age_s=11 * DAY)],
      # groups sort by reclaimable size: 0 = c (16 B x 1), 1 = a (6 B x 2), 2 = b (10 B x 1)
      params={"remove": [[0, [0, 1]], [1, [0]], [2, [1]]]}),
    S("trash-duplicates-stale-keeper", "trash_duplicates", "If the keeper disappeared or changed size since the scan, nothing is removed.",
      files=[F("k1.bin", text="KEEPKEEPKEEP", age_s=12 * DAY), F("k2.bin", text="KEEPKEEPKEEP", age_s=11 * DAY),
             F("t1.bin", text="TRUNCATED-FILE-X", age_s=12 * DAY), F("t2.bin", text="TRUNCATED-FILE-X", age_s=11 * DAY)],
      params={"remove": [[0, [1]], [1, [1]]],
              "after_scan": [{"do": "delete", "path": "k1.bin"}, {"do": "truncate", "path": "t1.bin", "size": 3}]}),
    S("check-dup-selection", "check_dup_selection", "Selecting every copy of a group is refused; leaving one is fine.",
      files=[F("g1a.bin", text="GROUPONE"), F("g1b.bin", text="GROUPONE"), F("g2a.bin", text="GROUPTWO!"),
             F("g2b.bin", text="GROUPTWO!")],
      params={"selections": [["g1a.bin"], ["g1a.bin", "g1b.bin"], ["g1a.bin", "g2a.bin"],
                             ["g1a.bin", "g1b.bin", "g2a.bin", "g2b.bin"], []]}),
    S("trash-empty-dirs-recheck", "trash_empty",
      "Deepest first; a folder that received a file since the scan or holds a project is refused; junk-only goes whole.",
      files=[F("junkdir/desktop.ini", text="x"), F("keep/f.txt")],
      dirs=["a/b/c", "late/inner", "nested/e1/e2"],
      params={"after_scan": [{"do": "write", "path": "late/inner/new.txt", "text": "arrived"},
                             {"do": "mkdir", "path": "a/b/c/.git"}]}),
    S("trash-redownloads-guards", "trash_redownloads",
      "A copy is removed only if its original still exists and is not itself being removed.",
      files=[F("a.pdf", size=1000), F("a (1).pdf", size=1000), F("b.pdf", size=2000), F("b (1).pdf", size=2000),
             F("c.pdf", size=3000), F("c (1).pdf", size=3000)],
      params={"after_scan": [{"do": "delete", "path": "a.pdf"}], "removing": ["b.pdf"]}),
]


def main(out: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.json"):
        old.unlink()
    names = []
    for spec in SCENARIOS:
        fixture = run(spec)
        (out / f"{spec['name']}.json").write_text(json.dumps(fixture, indent=1, sort_keys=False) + "\n", encoding="utf-8")
        names.append(spec["name"])
    (out / "index.json").write_text(json.dumps({"schema": SCHEMA, "fixtures": names}, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {len(names)} fixtures to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT_DIR / "tests" / "golden"))
