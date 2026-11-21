#!/usr/bin/env python3
r"""Generate a deterministic synthetic folder tree for the performance harness (docs/PERF.md). Standard library only.

    python tools/gen_tree.py D:\perf\tree100k            # 100,000 files
    python tools/gen_tree.py D:\perf\tree10k --files 10000

Same arguments, same tree: names, sizes, contents, folder shape and modified times are all derived from --seed.
About 5% of the files are exact copies of an earlier one (the duplicates), sizes are mostly small with a long
tail, folders nest up to 8 deep, and every file is months old so "leave recently changed files alone" never
hides one. Next to the N counted files there are a few folders a scan must NOT enter (project folders, a
portable app, hidden folders); their contents are extra and not part of N.

The destination must not exist or must be empty, and must not be a drive root, your home folder (or a parent of
it) or anything inside AppData: nothing is ever written into a folder that already holds something.
"""
import argparse
import os
import random
import sys
import time
from pathlib import Path

EXTS = [".txt", ".pdf", ".jpg", ".png", ".docx", ".zip", ".mp3", ".mp4", ".py", ".csv", ".log", ".exe"]
BASE_MTIME = 1_750_000_000  # fixed, so the tree does not change with the day it is generated
# Folders the scan must skip (SPEC rule 9, hidden folders); the self-check leaves them out of the count.
SKIPPED = {"proj_node", "proj_git", "proj_py", "portable_app", "HiddenFolder"}


def refusal(dest: Path) -> str | None:
    home = Path(os.path.expanduser("~")).resolve()
    if dest == Path(dest.anchor):
        return "that is a drive root"
    if dest == home or dest in home.parents:
        return "that is your home folder or a parent of it"
    for appdata in {home / "AppData", *(Path(p).resolve() for p in (os.environ.get("APPDATA"), os.environ.get("LOCALAPPDATA")) if p)}:
        if dest == appdata or appdata in dest.parents:
            return "that is inside AppData"
    if dest.exists() and (not dest.is_dir() or any(dest.iterdir())):
        return "that exists and is not an empty folder"
    return None


def pick_size(rng: random.Random) -> int:
    r = rng.random()
    if r < 0.90:
        return rng.randint(100, 4 * 1024)
    if r < 0.99:
        return rng.randint(4 * 1024, 64 * 1024)
    if r < 0.9995:
        return rng.randint(64 * 1024, 1024 * 1024)
    return rng.randint(1024 * 1024, 8 * 1024 * 1024)


def write(path: Path, data: bytes, mtime: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    os.utime(path, (mtime, mtime))


def extras(root: Path) -> None:
    """Folders a scan must not enter, each with a few files."""
    markers = {"proj_node": "package.json", "proj_git": ".git/HEAD", "proj_py": "pyproject.toml"}
    for folder, marker in markers.items():
        write(root / folder / marker, b"x", BASE_MTIME)
        for i in range(10):
            write(root / folder / "src" / f"m{i}.txt", b"project file %d" % i, BASE_MTIME)
    write(root / "portable_app" / "tool.exe", b"MZ", BASE_MTIME)
    write(root / "portable_app" / "tool.dll", b"MZ", BASE_MTIME)
    for i in range(10):
        write(root / ".hidden_stash" / f"h{i}.txt", b"hidden %d" % i, BASE_MTIME)
        write(root / "HiddenFolder" / f"h{i}.txt", b"hidden %d" % i, BASE_MTIME)
    if os.name == "nt":
        import ctypes
        ctypes.windll.kernel32.SetFileAttributesW(str(root / "HiddenFolder"), 0x2)  # FILE_ATTRIBUTE_HIDDEN


def generate(root: Path, n: int, seed: int) -> tuple[int, int]:
    rng = random.Random(seed)
    dirs = [(root, 0)]
    for i in range(max(1, n // 40)):
        parent, depth = rng.choice(dirs)
        if depth < 8:
            dirs.append((parent / f"folder_{i:05d}", depth + 1))
    originals: list[tuple[int, int, str]] = []  # (content id, size, extension)
    total = 0
    for i in range(n):
        folder = rng.choice(dirs)[0]
        mtime = BASE_MTIME - rng.randint(0, 1500 * 86400)
        if originals and rng.random() < 0.05:
            cid, size, ext = rng.choice(originals)
            name = f"copy_{i:06d}{ext}"
        else:
            cid, size, ext = len(originals), pick_size(rng), rng.choice(EXTS)
            originals.append((cid, size, ext))
            name = f"file_{i:06d}{ext}"
        write(folder / name, random.Random(f"{seed}:{cid}").randbytes(size), mtime)
        total += size
    extras(root)
    return len(dirs), total


def counted_files(root: Path) -> int:
    count = 0
    for _, subdirs, files in os.walk(root):
        subdirs[:] = [d for d in subdirs if d not in SKIPPED and not d.startswith(".")]
        count += len(files)
    return count


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dest", help="folder to create the tree in (must not exist or be empty)")
    ap.add_argument("--files", type=int, default=100_000, help="number of files (default 100000)")
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    if args.files < 1:
        ap.error("--files must be at least 1")
    dest = Path(args.dest).resolve()
    why = refusal(dest)
    if why:
        print(f"Refusing to write to {dest}: {why}.", file=sys.stderr)
        return 2
    started = time.time()
    ndirs, total = generate(dest, args.files, args.seed)
    found = counted_files(dest)
    assert found == args.files, f"expected {args.files} files, found {found}"
    print(f"{dest}: {found} files in {ndirs} folders, {total / 2**20:.0f} MB, {time.time() - started:.0f} s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
