"""D likely leftovers, E possible re-downloads, F sort sub-buckets, G pins. Real temp files only."""
import os
import zipfile
from pathlib import Path
from types import SimpleNamespace

import pytest

from declutter import core

from test_core import touch


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "messy"
    r.mkdir()
    return r


@pytest.fixture
def sent(monkeypatch):
    out = []
    monkeypatch.setattr(core, "send2trash", out.append)
    return out


def make_zip(path: Path, files: dict[str, bytes]) -> Path:
    with zipfile.ZipFile(path, "w") as z:
        for name, data in files.items():
            z.writestr(name, data)
    touch(path, path.read_bytes())  # backdate it like every other fixture file
    return path


def unpack(folder: Path, files: dict[str, bytes]) -> None:
    for name, data in files.items():
        touch(folder / name, data)


def leftovers(root, **kw) -> dict[str, str]:
    found = core.find_leftovers(root, entries=core.scan_entries(root), **kw)
    return {f.path.name: f.reason for f in found}


FILES = {"a.txt": b"hello", "sub/b.txt": b"world!"}
NOW = 1_800_000_000.0  # a fixed "now" for entries built by hand


# ---------- D. already-extracted zips ----------

def test_zip_with_all_entries_in_same_named_folder_is_a_leftover(root):
    make_zip(root / "pack.zip", FILES)
    unpack(root / "pack", FILES)
    assert leftovers(root) == {"pack.zip": core.EXTRACTED}


@pytest.mark.parametrize("case", ["name only", "wrong size", "missing file"])
def test_zip_needs_every_entry_to_match(root, case):
    make_zip(root / "pack.zip", FILES)
    if case == "name only":
        (root / "pack").mkdir()
    elif case == "wrong size":
        unpack(root / "pack", {"a.txt": b"hello", "sub/b.txt": b"WORLD!!"})
    else:
        unpack(root / "pack", {"a.txt": b"hello"})
    assert leftovers(root) == {}


def test_zip_is_not_matched_against_its_parent_folder(root):
    make_zip(root / "docs.zip", {"README.txt": b"readme"})
    touch(root / "README.txt", b"readme")  # same name and size, but there is no folder called "docs"
    assert leftovers(root) == {}


def test_zip_entry_escaping_its_folder_does_not_count(root):
    make_zip(root / "pack.zip", {"../x.txt": b"hello"})
    touch(root / "pack" / "y.txt", b"other")
    touch(root / "x.txt", b"hello")
    assert leftovers(root) == {}


def test_only_zip_archives_and_never_broken_or_empty_ones(root):
    touch(root / "bad.zip", b"not a zip")
    unpack(root / "bad", {"a.txt": b"hello"})
    make_zip(root / "dirs.zip", {"only/": b""})
    (root / "dirs").mkdir()
    for ext in (".7z", ".rar"):  # only .zip is looked at, even if the bytes happen to be a zip
        make_zip(root / f"pack{ext}", FILES)
    unpack(root / "pack", FILES)
    assert leftovers(root) == {}


def test_zero_byte_entry_needs_a_file_not_a_folder(root):
    make_zip(root / "pack.zip", {"empty.txt": b""})
    (root / "pack" / "empty.txt").mkdir(parents=True)  # a directory of that name also has size 0
    assert leftovers(root) == {}


def test_a_young_dead_download_entry_is_not_a_leftover(root):
    young = SimpleNamespace(st_size=1, st_mtime=NOW - 86400, st_birthtime=NOW - 86400)
    old = SimpleNamespace(st_size=1, st_mtime=NOW - 4 * 86400, st_birthtime=NOW - 4 * 86400)
    entries = [(root / "young.crdownload", young), (root / "old.crdownload", old)]
    got = core.find_leftovers(root, entries=entries, now=NOW)
    assert [x.path.name for x in got] == ["old.crdownload"]


# ---------- D. old installers ----------

def test_old_installers_in_downloads_only(root, monkeypatch):
    dl = root / "Downloads"
    monkeypatch.setattr(core, "_known_folder_path", lambda folder_id: dl)
    for n in ("a.exe", "b.MSI", "c.iso"):
        touch(dl / n, age_days=8)
    touch(dl / "young.exe", age_days=6)
    touch(dl / "notes.txt", age_days=30)
    touch(root / "elsewhere" / "old.exe", age_days=30)
    assert leftovers(root) == {n: core.OLD_INSTALLER for n in ("a.exe", "b.MSI", "c.iso")}


# ---------- D. dead downloads ----------

def test_dead_downloads_are_found_after_3_days(root):
    touch(root / "a.crdownload", age_days=4)
    touch(root / "b.part", age_days=4)
    touch(root / "young.crdownload", age_days=2)
    touch(root / "old.tmp", age_days=10)  # other partial types are never offered
    assert leftovers(root) == {"a.crdownload": core.DEAD_DOWNLOAD, "b.part": core.DEAD_DOWNLOAD}


def test_dead_downloads_are_only_offered_as_leftovers(root, sent):
    a = touch(root / "a.crdownload", b"same", age_days=4)
    touch(root / "b.crdownload", b"same", age_days=5)
    entries = core.scan_entries(root)
    assert {p.name for p, _ in entries} == {"a.crdownload", "b.crdownload"}
    assert list(core.plan_sort(root, entries=entries)) == []
    assert core.find_duplicates(root, entries=entries) == []
    assert core.find_large_old(root, min_mb=0, entries=entries) == []
    assert core.to_recycle_bin([a], root) == (1, [])  # old enough to be removed
    young = touch(root / "young.part", age_days=1)
    ok, errs = core.to_recycle_bin([young], root)
    assert ok == 0 and len(errs) == 1 and sent == [os.path.abspath(a)]


def test_pinned_files_are_never_leftovers(root):
    make_zip(root / "pack.zip", FILES)
    unpack(root / "pack", FILES)
    touch(root / "a.crdownload", age_days=4)
    entries = core.scan_entries(root, ignore=[root / "PACK.zip", root / "a.crdownload"])  # case-insensitive
    assert core.find_leftovers(root, entries=entries) == []


# ---------- E. possible re-downloads ----------

def redl(root, exclude=()):
    found = core.find_redownloads(root, entries=core.scan_entries(root), exclude=exclude)
    return {r.copy.name: r.original.name for r in found}


def test_numbered_and_copy_names_within_2_percent(root):
    touch(root / "report.pdf", b"x" * 1000)
    touch(root / "report (1).pdf", b"y" * 1010)
    touch(root / "report - Copy.pdf", b"z" * 1019)
    touch(root / "report (2).pdf", b"w" * 1030)       # 3 % bigger: a different file
    touch(root / "report (3).txt", b"x" * 1000)       # other extension
    touch(root / "lonely (1).pdf", b"x" * 1000)       # no original next to it
    touch(root / "sub" / "report (1).pdf", b"x" * 1000)  # original is in another folder
    assert redl(root) == {"report (1).pdf": "report.pdf", "report - Copy.pdf": "report.pdf"}


def test_redownloads_show_size_and_date_of_both(root):
    touch(root / "a.pdf", b"x" * 1000, age_days=5)
    touch(root / "a (1).pdf", b"y" * 1010, age_days=1)
    (r,) = core.find_redownloads(root, entries=core.scan_entries(root))
    assert (r.orig_size, r.copy_size) == (1000, 1010) and r.orig_mtime < r.copy_mtime


def test_exact_duplicates_stay_in_the_duplicates_section(root):
    touch(root / "a.pdf", b"same", age_days=5)
    touch(root / "a (1).pdf", b"same")
    entries = core.scan_entries(root)
    groups = core.find_duplicates(root, entries=entries)
    assert len(groups) == 1
    exclude = {f for g in groups for f in g.files}
    assert core.find_redownloads(root, entries=entries, exclude=exclude) == []


def test_removing_redownloads_always_keeps_an_original(root, sent):
    a = touch(root / "a.pdf", b"x" * 1000)
    a1 = touch(root / "a (1).pdf", b"y" * 1005)
    a11 = touch(root / "a (1) (1).pdf", b"z" * 1008)
    pairs = core.find_redownloads(root, entries=core.scan_entries(root))
    assert {p.copy.name for p in pairs} == {"a (1).pdf", "a (1) (1).pdf"}
    ok, errs = core.trash_redownloads(pairs, root)  # both copies chosen: a (1) (1)'s original is going too
    assert ok == 1 and len(errs) == 1 and sent == [os.path.abspath(a1)] and a.exists() and a11.exists()


def test_redownload_removal_needs_its_original_on_disk(root, sent):
    a = touch(root / "a.pdf", b"x" * 1000)
    touch(root / "a (1).pdf", b"y" * 1005)
    pairs = core.find_redownloads(root, entries=core.scan_entries(root))
    a.unlink()
    ok, errs = core.trash_redownloads(pairs, root)
    assert ok == 0 and len(errs) == 1 and sent == []


def test_redownload_original_removed_by_the_duplicates_in_the_same_job(root, sent):
    a = touch(root / "a.pdf", b"x" * 1000)
    touch(root / "a (1).pdf", b"y" * 1005)
    pairs = core.find_redownloads(root, entries=core.scan_entries(root))
    ok, errs = core.trash_redownloads(pairs, root, removing={a})
    assert ok == 0 and len(errs) == 1 and sent == []


# ---------- F. sort sub-buckets ----------

def plan(root):
    return {m.src.name: m for m in core.plan_sort(root)}


def test_screenshot_and_chat_images_get_one_level_sub_buckets(root):
    names = {
        "Screenshot 2026-01-01.png": "Images/Screenshots", "Screenshot (3).PNG": "Images/Screenshots",
        "Snip.png": "Images/Screenshots", "snipping tool 2.jpg": "Images/Screenshots",
        "Capture.png": "Images/Screenshots",
        "IMG-20240115-WA0003.jpg": "Images/Chat", "WhatsApp Image 2024-01-15 at 10.20.30.jpeg": "Images/Chat",
        "IMG-1234.jpg": "Images", "IMG-2024011-WA1.jpg": "Images", "IMG-20240115-0003.jpg": "Images",
        "photo.png": "Images",
        "Screenshot notes.pdf": "Documents", "Capture.zip": "Archives", "Snip.txt": "Documents",
    }
    for n in names:
        touch(root / n)
    got = plan(root)
    assert {n: m.category for n, m in got.items()} == names
    assert got["Snip.png"].dst == root / "Images" / "Screenshots" / "Snip.png"
    assert got["IMG-20240115-WA0003.jpg"].dst.parent == root / "Images" / "Chat"


def test_sub_bucket_sort_and_undo_leave_no_empty_folders(root):
    touch(root / "Screenshot 1.png", b"1")
    touch(root / "WhatsApp Image 1.jpg", b"2")
    touch(root / "photo.png", b"3")
    assert core.apply_moves(core.plan_sort(root), root) == (3, [])
    assert (root / "Images" / "Screenshots" / "Screenshot 1.png").read_bytes() == b"1"
    assert core.undo_last() == (3, [])
    assert sorted(p.name for p in root.iterdir()) == ["Screenshot 1.png", "WhatsApp Image 1.jpg", "photo.png"]

