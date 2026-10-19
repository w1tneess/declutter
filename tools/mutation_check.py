"""Switch each rule off in turn; the suite must fail every time. Restores core.py at the end.

Run from anywhere:  python tools/mutation_check.py
Exit code 1 if any mutant survives (a rule no test notices) or a pattern no longer matches core.py.
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
core = ROOT / "src" / "declutter" / "core.py"
raw = core.read_bytes()  # restored byte for byte at the end, so line endings never change
orig = core.read_text(encoding="utf-8")
M = {
    "A grace skip": ("        if recent_hours and last_touched(st) > cutoff:", "        if False:"),
    "A count": ("            plan.recent_left += 1\n", "            pass\n"),
    "A creation time": ("    return max(st.st_mtime, born)", "    return st.st_mtime"),
    "A ctime fallback": ("        born = st.st_ctime", "        born = 0"),
    "A apply recheck": ("            if recent_hours and last_touched(st) > (time.time()", "            if False and last_touched(st) > (time.time()"),
    "A clamp": ("max(0, min(RECENT_HOURS_MAX, int(float(v))))", "int(float(v))"),
    "B shortcuts": ("ext in SKIP_SORT_EXTS or ", ""),
    "C known folder": ("return _known_folder_path(KNOWN_FOLDERS[name]) or Path.home() / name", "return Path.home() / name"),
    "C downloads id": ('"Downloads": "{374DE290-123F-4565-9164-39C4925E467B}"', '"Downloads": "{374DE290-123F-4565-9164-39C4925E467C}"'),
    "C in_downloads": ("    return is_pinned(p, [known_folder(\"Downloads\")])", "    return \"downloads\" in str(p).lower()"),
    "D dead age-aware": ("    active = ext in PARTIAL_EXTS and not (ext in DEAD_EXTS and now - st.st_mtime > DEAD_SECS)", "    active = ext in PARTIAL_EXTS"),
    "D dead age": ("        if ext in DEAD_EXTS and now - st.st_mtime > DEAD_SECS:", "        if ext in DEAD_EXTS:"),
    "D installer age": ("now - st.st_mtime > INSTALLER_SECS and in_downloads(p)", "in_downloads(p)"),
    "D installer in Downloads": ("now - st.st_mtime > INSTALLER_SECS and in_downloads(p)", "now - st.st_mtime > INSTALLER_SECS"),
    "D zip same-named folder": ("    folder = zip_path.parent / zip_path.stem", "    folder = zip_path.parent"),
    "D zip sizes": ("or os.path.getsize(target) != i.file_size:", ":"),
    "D zip entry exists": ("or not os.path.isfile(target)", ""),
    "D zip escape": ("            if not os.path.normcase(target).startswith(base + os.sep):", "            if False:"),
    "D zip empty": ("        if not files:\n            return False\n", ""),
    "D zip only": ('        elif ext == ".zip" and _is_extracted(p):', '        elif ext in (".zip", ".7z", ".rar") and _is_extracted(p):'),
    "D sort skips dead": (" or ext in DEAD_EXTS:\n            continue", ":\n            continue"),
    "D dups skip dead": ("not in DEAD_EXTS | SKIP_SORT_EXTS):", "not in SKIP_SORT_EXTS):"),
    "D large skips dead": ("and not (older_days and st.st_mtime > cutoff) and p.suffix.lower() not in DEAD_EXTS]", "and not (older_days and st.st_mtime > cutoff)]"),
    "E 2 percent": ("<= 0.02 * orig[1].st_size", "<= 0.5 * orig[1].st_size"),
    "E exclude": ("if not m or p in skip or not st.st_size or", "if not m or not st.st_size or"),
    "E original on disk": ("not r.original.is_file() or ", ""),
    "E original leaving": ("r.original in gone or ", ""),
    "E removing arg": (" | set(removing)", ""),
    "F sub-bucket images only": ('    if cat == "Images":', "    if True:"),
    "F chat regex": (r'IMG-\d{8}-WA\d+', "IMG-"),
    "F undo parent": ("for d in (dst.parent, dst.parent.parent)[:len(_bucket(dst))]:", "for d in (dst.parent,):"),
    "X dups skip shortcuts": ("not in DEAD_EXTS | SKIP_SORT_EXTS):", "not in DEAD_EXTS):"),
    "X redl skip shortcuts": (" or p.suffix.lower() in SKIP_SORT_EXTS:\n            continue", ":\n            continue"),
    "R1 size pair exact bytes": ('if x != y else f"{a:,} B vs {b:,} B"', 'if True else f"{a:,} B vs {b:,} B"'),
    "R1 ago plural": ("{'' if n == 1 else 's'}", "{'s'}"),
    "R1 peek folder": ('UndoInfo(Path(moves[0]["src"]).parent, len(moves), age)', 'UndoInfo(Path(moves[0]["src"]), len(moves), age)'),
    "R1 peek unreadable time": ("        age = None\n    return UndoInfo", "        age = 0.0\n    return UndoInfo"),
    "F category": ('return "/".join(_bucket(self.dst))', "return self.dst.parent.name"),
    "R4 marker .svn": ('".svn", ', ""),
    "R4 marker .hg": ('".hg", ', ""),
    "R4 marker venv": ('"venv", ', ""),
    "R4 marker pyproject": ('"pyproject.toml",\n                   "cargo.toml"}', '"cargo.toml"}'),
    "R4 marker cargo": ('"pyproject.toml",\n                   "cargo.toml"}', '"pyproject.toml"}'),
    "R4 app folder needs a dll": ('and any(n.endswith(".dll") for n in lower_names)', ""),
    "R4 app folder dll or exe": ('for n in lower_names) and any(n.endswith(".dll")', 'for n in lower_names) or any(n.endswith(".dll")'),
    "R4 app folders skipped in walk": ("if is_project(names) or (here != root_s and is_app_folder(names)):", "if is_project(names):"),
    "R4 chosen folder exempt": ("(here != root_s and is_app_folder(names))", "is_app_folder(names)"),
    "R4 skipped reported": ("                skipped.append(Path(here))", "                pass"),
    "R4 app folders not searched for empty": ("if is_project(names) or (d != root_s and is_app_folder(names)):", "if is_project(names):"),
}
bad = []
try:
    for name, (a, b) in M.items():
        assert orig.count(a) == 1, f"{name}: pattern found {orig.count(a)} times"
        core.write_text(orig.replace(a, b), encoding="utf-8")
        r = subprocess.run([sys.executable, "-m", "pytest", "-q", "-x", "-p", "no:cacheprovider"],
                           capture_output=True, text=True, cwd=ROOT)
        caught = r.returncode != 0
        print(f"{'caught ' if caught else 'MISSED '} {name}", flush=True)
        if not caught:
            bad.append(name)
finally:
    core.write_bytes(raw)
print("all caught" if not bad else f"missed: {bad}")
sys.exit(1 if bad else 0)
