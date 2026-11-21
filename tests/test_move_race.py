"""A file that appears at the destination between the name check and the move must never be
overwritten (SPEC safety rule 3: moves never overwrite). The race is forced by creating the
clashing file at the moment the move is attempted."""
import errno
import os
import shutil

import pytest

from declutter import core

from test_core import touch

REAL_RENAME = os.rename


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "messy"
    r.mkdir()
    return r


def plan_one(root):
    touch(root / "a.txt", b"mine", age_days=2)
    moves = core.plan_sort(root)
    assert [m.dst for m in moves] == [root / "Documents" / "a.txt"]
    return moves


def clash_then(monkeypatch, clash, then):
    """On the first rename, create `clash` first (the other program winning the race), then
    behave as `then`: None = the real rename, an errno = fail like a cross-volume move."""
    state = {"fired": False}

    def fake(src, dst, *a, **kw):
        if not state["fired"]:
            state["fired"] = True
            clash.parent.mkdir(parents=True, exist_ok=True)
            clash.write_bytes(b"theirs")
        if then is not None:
            raise OSError(then, "simulated: not the same disk")
        return REAL_RENAME(src, dst, *a, **kw)

    monkeypatch.setattr(os, "rename", fake)
    return state


def assert_both_kept(root):
    assert (root / "Documents" / "a.txt").read_bytes() == b"theirs", "the file that appeared was overwritten"
    assert (root / "Documents" / "a (1).txt").read_bytes() == b"mine"
    assert not (root / "a.txt").exists()


def test_same_disk_race_never_overwrites(root, monkeypatch):
    moves = plan_one(root)
    clash_then(monkeypatch, root / "Documents" / "a.txt", then=None)
    n, errs = core.apply_moves(moves)
    assert (n, errs) == (1, [])
    assert_both_kept(root)


def test_cross_disk_race_never_overwrites(root, monkeypatch):
    moves = plan_one(root)
    # every rename fails like a cross-disk move, and the clash appears just as the first one is tried
    clash_then(monkeypatch, root / "Documents" / "a.txt", then=errno.EXDEV)
    n, errs = core.apply_moves(moves)
    assert (n, errs) == (1, [])
    assert_both_kept(root)


def test_cross_disk_move_copies_and_keeps_the_timestamp(root, monkeypatch):
    moves = plan_one(root)
    before = os.stat(root / "a.txt").st_mtime
    monkeypatch.setattr(os, "rename", lambda s, d, *a, **k: (_ for _ in ()).throw(OSError(errno.EXDEV, "other disk")))
    n, errs = core.apply_moves(moves)
    assert (n, errs) == (1, [])
    assert (root / "Documents" / "a.txt").read_bytes() == b"mine"
    assert os.stat(root / "Documents" / "a.txt").st_mtime == pytest.approx(before, abs=2)
    assert not (root / "a.txt").exists()


def test_failed_copy_leaves_the_original_and_no_partial_file(root, monkeypatch):
    moves = plan_one(root)
    monkeypatch.setattr(os, "rename", lambda s, d, *a, **k: (_ for _ in ()).throw(OSError(errno.EXDEV, "other disk")))

    def disk_full(src, dst, *a, **k):
        dst.write(b"par")  # a half-written copy
        raise OSError(errno.ENOSPC, "No space left on device")

    monkeypatch.setattr(shutil, "copyfileobj", disk_full)
    n, errs = core.apply_moves(moves)
    assert n == 0 and len(errs) == 1 and "a.txt" in errs[0]
    assert (root / "a.txt").read_bytes() == b"mine"
    assert not (root / "Documents" / "a.txt").exists(), "a partial copy was left behind"
    assert not core.can_undo()


def test_source_that_cannot_be_deleted_keeps_one_copy_only(root, monkeypatch):
    moves = plan_one(root)
    src = root / "a.txt"
    monkeypatch.setattr(os, "rename", lambda s, d, *a, **k: (_ for _ in ()).throw(OSError(errno.EXDEV, "other disk")))
    real_unlink = os.unlink

    def locked(p, *a, **k):
        if os.path.abspath(p) == str(src):
            raise PermissionError(errno.EACCES, "in use by another program")
        return real_unlink(p, *a, **k)

    monkeypatch.setattr(os, "unlink", locked)
    n, errs = core.apply_moves(moves)
    assert n == 0 and len(errs) == 1
    assert src.read_bytes() == b"mine"
    assert not (root / "Documents" / "a.txt").exists(), "a duplicate copy was left behind"


def test_a_name_that_never_frees_up_gives_an_error_not_a_hang(root, monkeypatch):
    moves = plan_one(root)

    def always_taken(src, dst, *a, **k):
        raise FileExistsError(errno.EEXIST, "exists")  # e.g. a dangling symlink exists() cannot see

    monkeypatch.setattr(os, "rename", always_taken)
    n, errs = core.apply_moves(moves)
    assert n == 0 and len(errs) == 1
    assert (root / "a.txt").read_bytes() == b"mine"


def test_undo_race_never_overwrites(root, monkeypatch):
    touch(root / "a.txt", b"mine", age_days=2)
    assert core.apply_moves(core.plan_sort(root)) == (1, [])
    clash_then(monkeypatch, root / "a.txt", then=None)  # something re-creates a.txt just as undo moves back
    restored, errs = core.undo_last()
    assert (restored, errs) == (1, [])
    assert (root / "a.txt").read_bytes() == b"theirs"
    assert (root / "a (1).txt").read_bytes() == b"mine"
