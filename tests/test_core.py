import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

from declutter import core


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "messy"
    r.mkdir()
    return r


def touch(p: Path, data: bytes = b"x", age_days: float = 0.1) -> Path:
    """Write a file, backdated: files changed in the last 5 minutes are skipped as in use."""
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(data)
    t = time.time() - age_days * 86400
    os.utime(p, (t, t))
    return p


def hide(p: Path) -> None:
    if sys.platform != "win32":
        pytest.skip("hidden attribute is Windows-only")
    subprocess.run(["attrib", "+h", str(p)], check=True)


# ---------- refused locations ----------

def test_refuses_drive_root():
    ok, _ = core.check_root(Path(Path.home().anchor))
    assert not ok


def test_refuses_home_itself(root, monkeypatch):
    monkeypatch.setattr(Path, "home", classmethod(lambda cls: root))
    assert not core.check_root(root)[0]
    sub = root / "Downloads"
    sub.mkdir()
    assert core.check_root(sub)[0]


@pytest.mark.parametrize("name", ["Windows", "Program Files", "ProgramData", "AppData", ".git", "node_modules"])
def test_refuses_protected_names(root, name):
    d = root / name / "inner"
    d.mkdir(parents=True)
    assert not core.check_root(d)[0]
    with pytest.raises(ValueError):
        core.plan_sort(d)


def test_refuses_missing_folder(root):
    assert not core.check_root(root / "nope")[0]


# ---------- sort ----------

def test_sort_plan_categories(root):
    for n in ["a.JPG", "b.pdf", "c.mp4", "d.mp3", "e.zip", "f.exe", "g.py", "h.xyz"]:
        touch(root / n)
    cats = {m.src.name: m.category for m in core.plan_sort(root)}
    assert cats == {"a.JPG": "Images", "b.pdf": "Documents", "c.mp4": "Videos", "d.mp3": "Audio",
                    "e.zip": "Archives", "f.exe": "Installers", "g.py": "Code", "h.xyz": "Other"}


def test_sort_collisions_on_disk(root):
    touch(root / "Documents" / "a.txt", b"old")  # already on disk
    touch(root / "Documents" / "a (1).txt", b"older")
    touch(root / "a.txt", b"1")
    assert [m.dst.name for m in core.plan_sort(root)] == ["a (2).txt"]


def test_sort_skips_hidden_and_symlinks(root):
    touch(root / ".secret.txt")
    h = touch(root / "hidden.txt")
    hide(h)
    touch(root / "ok.txt")
    assert [m.src.name for m in core.plan_sort(root)] == ["ok.txt"]


def test_sort_skips_symlink(root):
    target = touch(root.parent / "outside.txt")
    try:
        os.symlink(target, root / "link.txt")
    except OSError:
        pytest.skip("symlinks not permitted")
    assert core.plan_sort(root) == []


def test_apply_and_undo_round_trip(root):
    touch(root / "a.txt", b"A")
    touch(root / "b.jpg", b"B")
    moves = core.plan_sort(root)
    n, errs = core.apply_moves(moves)
    assert (n, errs) == (2, [])
    assert (root / "Documents" / "a.txt").read_bytes() == b"A"
    assert not (root / "a.txt").exists()
    assert core.can_undo()

    restored, errs = core.undo_last()
    assert (restored, errs) == (2, [])
    assert (root / "a.txt").read_bytes() == b"A"
    assert (root / "b.jpg").read_bytes() == b"B"
    assert not (root / "Documents").exists()  # emptied category folder removed
    assert not core.can_undo()


def test_undo_reports_missing_file(root):
    touch(root / "a.txt")
    core.apply_moves(core.plan_sort(root))
    (root / "Documents" / "a.txt").unlink()
    restored, errs = core.undo_last()
    assert restored == 0 and len(errs) == 1


def test_undo_with_nothing_logged():
    assert core.undo_last() == (0, ["Nothing to undo."])


# ---------- duplicates ----------

def test_duplicates_same_content_different_names(root):
    old = touch(root / "old.bin", b"same", age_days=10)
    new = touch(root / "sub" / "copy.bin", b"same", age_days=1)
    groups = core.find_duplicates(root)
    assert len(groups) == 1
    assert groups[0].files == [old, new]  # oldest first = suggested keeper


def test_same_size_different_content_not_duplicates(root):
    touch(root / "a.bin", b"aaaa")
    touch(root / "b.bin", b"bbbb")
    assert core.find_duplicates(root) == []


def test_empty_files_ignored(root):
    touch(root / "a", b"")
    touch(root / "b", b"")
    assert core.find_duplicates(root) == []


def test_dup_selection_never_all_copies(root):
    a = touch(root / "a.bin", b"same", age_days=5)
    b = touch(root / "b.bin", b"same")
    groups = core.find_duplicates(root)
    core.check_dup_selection(groups, {b})
    with pytest.raises(ValueError):
        core.check_dup_selection(groups, {a, b})


# ---------- large and old ----------

def test_large_old_filters(root):
    mb = 1024 * 1024
    touch(root / "big_new.bin", b"0" * (2 * mb), age_days=1)
    touch(root / "big_old.bin", b"0" * (2 * mb), age_days=100)
    touch(root / "small_old.bin", b"0", age_days=100)

    def names(**kw):
        return [f.path.name for f in core.find_large_old(root, **kw)]

    assert names(min_mb=1, older_days=30) == ["big_old.bin"]  # both must match
    assert sorted(names(min_mb=1, older_days=0)) == ["big_new.bin", "big_old.bin"]
    assert sorted(names(min_mb=0, older_days=30)) == ["big_old.bin", "small_old.bin"]
    assert len(names(min_mb=0, older_days=0)) == 3


def test_large_old_sorted_largest_first(root):
    touch(root / "m.bin", b"0" * 200)
    touch(root / "l.bin", b"0" * 900)
    touch(root / "s.bin", b"0" * 10)
    assert [f.path.name for f in core.find_large_old(root, min_mb=0)] == ["l.bin", "m.bin", "s.bin"]


# ---------- empty folders ----------

def test_empty_dirs_nested_deepest_first(root):
    (root / "a" / "b" / "c").mkdir(parents=True)
    touch(root / "full" / "keep.txt")
    (root / "full" / "empty").mkdir()
    (root / "has_file_deep" / "x").mkdir(parents=True)
    touch(root / "has_file_deep" / "x" / "f.txt")
    found = core.find_empty_dirs(root)
    assert set(found) == {root / "a", root / "a" / "b", root / "a" / "b" / "c", root / "full" / "empty"}
    assert found.index(root / "a" / "b" / "c") < found.index(root / "a" / "b") < found.index(root / "a")
    assert root not in found


def test_empty_dirs_hidden_and_protected_not_listed(root):
    (root / "AppData").mkdir()
    (root / ".cache").mkdir()
    (root / "plain").mkdir()
    assert core.find_empty_dirs(root) == [root / "plain"]


def test_dir_with_only_hidden_file_not_empty(root):
    touch(root / "d" / ".keep")
    assert core.find_empty_dirs(root) == []


# ---------- recycle bin ----------

def test_to_recycle_bin_calls_send2trash(root, monkeypatch):
    sent = []
    monkeypatch.setattr(core, "send2trash", sent.append)
    f = touch(root / "a.txt")
    ok, errs = core.to_recycle_bin([f, root / "missing.txt"])
    assert ok == 1 and errs == []
    assert sent == [os.path.abspath(f)]


# ---------- hardening ----------

def test_refuses_parent_of_home(root, monkeypatch):
    home = root / "profile"
    home.mkdir()
    monkeypatch.setattr(Path, "home", classmethod(lambda cls: home))
    assert not core.check_root(root)[0]


def test_junction_not_followed(root):
    if sys.platform != "win32":
        pytest.skip("junctions are Windows-only")
    target = root.parent / "elsewhere"
    touch(target / "secret.txt")
    link = root / "link"
    r = subprocess.run(["cmd", "/c", "mklink", "/J", str(link), str(target)], capture_output=True)
    if r.returncode:
        pytest.skip("cannot create junction")
    touch(root / "a.txt")
    assert [p.name for p, _ in core.scan_entries(root)] == ["a.txt"]
    assert core.find_empty_dirs(root) == []


def test_shared_entries_match_separate_walks(root):
    touch(root / "a.bin", b"same", age_days=3)
    touch(root / "s" / "b.bin", b"same")
    entries = core.scan_entries(root)
    assert [m.src for m in core.plan_sort(root, entries=entries)] == [m.src for m in core.plan_sort(root)]
    assert len(core.find_duplicates(root, entries=entries)) == 1


def test_big_duplicates_use_full_hash(root):
    end = b"h" * (70 * 1024)  # identical first and last 64 KB; only the middle differs
    touch(root / "a.bin", end + b"A" * 100 + end)
    touch(root / "b.bin", end + b"B" * 100 + end)
    touch(root / "c.bin", end + b"A" * 100 + end)
    groups = core.find_duplicates(root)
    assert len(groups) == 1 and {p.name for p in groups[0].files} == {"a.bin", "c.bin"}


def test_prefilter_uses_first_and_last_64kb(root, monkeypatch):
    head = b"h" * (200 * 1024)  # same size and first 64 KB; only the tail differs
    touch(root / "a.bin", head + b"A")
    touch(root / "b.bin", head + b"B")
    full = []
    real = core._hash
    monkeypatch.setattr(core, "_hash", lambda p, limit, cancel: (full.append(p) if limit is None else None)
                        or real(p, limit, cancel))
    assert core.find_duplicates(root) == []
    assert full == [], "files that differ in the last 64 KB must never need a full hash"


def test_ends_hash_covers_small_files_whole(root):
    # up to 128 KB the ends overlap, so the cheap pass alone must decide; a middle byte differs here
    a = touch(root / "a.bin", b"x" * 64 * 1024 + b"A" + b"y" * 60 * 1024)
    b = touch(root / "b.bin", b"x" * 64 * 1024 + b"B" + b"y" * 60 * 1024)
    assert core._hash(a, core._PARTIAL, None) != core._hash(b, core._PARTIAL, None)
    assert core.find_duplicates(root) == []


def test_apply_moves_refuses_outside_root(root, tmp_path):
    outside = touch(tmp_path / "outside.txt")
    mv = core.Move(outside, root / "Documents" / "outside.txt")
    n, errs = core.apply_moves([mv], root)
    assert n == 0 and len(errs) == 1 and outside.exists()


def test_apply_moves_skips_vanished_file(root):
    mv = core.Move(root / "gone.txt", root / "Documents" / "gone.txt")
    n, errs = core.apply_moves([mv], root)
    assert n == 0 and len(errs) == 1


def test_recycle_refuses_outside_root(root, tmp_path, monkeypatch):
    sent = []
    monkeypatch.setattr(core, "send2trash", sent.append)
    outside = touch(tmp_path / "outside.txt")
    inside = touch(root / "in.txt")
    ok, errs = core.to_recycle_bin([outside, inside, root], root)
    assert ok == 1 and len(errs) == 2 and sent == [os.path.abspath(inside)]


def test_trash_empty_dirs_rechecks(root, monkeypatch):
    sent = []
    monkeypatch.setattr(core, "send2trash", sent.append)
    (root / "still_empty").mkdir()
    (root / "got_a_file").mkdir()
    found = core.find_empty_dirs(root)
    touch(root / "got_a_file" / "new.txt")  # arrives after the scan
    ok, errs = core.trash_empty_dirs(found, root)
    assert ok == 1 and len(errs) == 1 and sent == [os.path.abspath(root / "still_empty")]


def test_trash_duplicates_needs_a_keeper(root, monkeypatch):
    sent = []
    monkeypatch.setattr(core, "send2trash", sent.append)
    a = touch(root / "a.bin", b"same", age_days=5)
    b = touch(root / "b.bin", b"same")
    group = core.find_duplicates(root)[0]
    a.unlink()  # the kept copy disappears after the scan
    ok, errs = core.trash_duplicates([(group, [b])], root)
    assert ok == 0 and len(errs) == 1 and sent == []


def test_undo_rejects_protected_paths_and_damaged_log(root):
    import json
    core.LOG_DIR.mkdir(parents=True, exist_ok=True)
    victim = touch(root / "x.txt")
    batch = [{"moves": [{"src": r"C:\Windows\x.txt", "dst": str(victim)}, {"bad": 1}]}]
    core.UNDO_LOG.write_text(json.dumps(batch), encoding="utf-8")
    restored, errs = core.undo_last()
    assert restored == 0 and len(errs) == 2 and victim.exists()


def test_log_write_is_atomic_file_replace(root):
    touch(root / "a.txt")
    core.apply_moves(core.plan_sort(root), root)
    assert core.UNDO_LOG.exists() and not core.UNDO_LOG.with_suffix(".tmp").exists()


# ---------- pins (never touch) ----------

def test_pinned_file_and_folder_are_skipped(root):
    touch(root / "keep.bin", b"same", age_days=3)
    touch(root / "copy.bin", b"same")
    touch(root / "pinned_dir" / "inside.txt")
    (root / "pinned_dir" / "empty").mkdir()
    (root / "outer" / "pinned_empty").mkdir(parents=True)
    pins = [str(root / "KEEP.bin"), root / "pinned_dir", root / "outer" / "pinned_empty"]  # case-insensitive
    assert sorted(p.name for p, _ in core.scan_entries(root, ignore=pins)) == ["copy.bin"]
    assert core.find_empty_dirs(root, ignore=pins) == []  # "outer" holds a pin, so it is not empty either
    assert core.find_duplicates(root, entries=core.scan_entries(root, ignore=pins)) == []


def test_is_pinned(root):
    pins = [root / "a", root / "b.txt"]
    assert core.is_pinned(root / "a" / "deep" / "x.txt", pins)
    assert core.is_pinned(root / "B.TXT", pins)
    assert not core.is_pinned(root / "ab.txt", pins)
    assert not core.is_pinned(root / "c.txt", [])


# ---------- undo count ----------

def test_undo_count_matches_last_batch(root):
    assert core.undo_count() == 0
    touch(root / "a.txt")
    touch(root / "b.jpg")
    core.apply_moves(core.plan_sort(root), root)
    assert core.undo_count() == 2
