"""M1 safety rules. Each test fails if its rule is removed from core.py."""
import json
import os
import sys
from pathlib import Path

import pytest

from declutter import core, theme

from test_core import hide, touch

REAL_LOG_DIR = core.LOG_DIR  # captured at import, before the autouse fixture patches it


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


def names(root):
    return sorted(p.name for p, _ in core.scan_entries(root))


# ---------- 1. active downloads and recently changed files ----------

@pytest.mark.parametrize("name", ["a.crdownload", "b.part", "c.tmp", "d.download", "e.opdownload", "f.!qB"])
def test_partial_downloads_skipped(root, name):
    touch(root / name, age_days=2)  # .crdownload / .part older than 3 days count as dead: see test_cleanup_rules
    touch(root / "ok.txt")
    assert names(root) == ["ok.txt"]
    assert [m.src.name for m in core.plan_sort(root)] == ["ok.txt"]


def test_recently_modified_files_skipped(root):
    touch(root / "fresh.txt", age_days=60 / 86400)  # one minute old
    touch(root / "settled.txt", age_days=6 * 60 / 86400)
    assert names(root) == ["settled.txt"]


def test_actions_recheck_for_recent_changes(root, sent):
    f = touch(root / "a.txt")
    moves = core.plan_sort(root)
    f.write_bytes(b"still downloading")  # changes after the scan
    n, errs = core.apply_moves(moves, root)
    assert n == 0 and len(errs) == 1 and f.exists()
    ok, errs = core.to_recycle_bin([f], root)
    assert ok == 0 and len(errs) == 1 and sent == []


# ---------- 2. project folders ----------

@pytest.mark.parametrize("marker", [".git", "node_modules", ".venv", "package.json", "App.sln"])
def test_project_folders_never_entered(root, marker):
    proj = root / "proj"
    if "." in marker[1:]:
        touch(proj / marker)
    else:
        (proj / marker).mkdir(parents=True)
    touch(proj / "main.py", b"same", age_days=3)
    (proj / "empty_sub").mkdir()
    touch(root / "copy.py", b"same", age_days=2)
    assert names(root) == ["copy.py"]
    assert core.find_duplicates(root) == []
    assert core.find_empty_dirs(root) == []


def test_project_folder_refused_as_root(root):
    touch(root / "package.json")
    assert not core.check_root(root)[0]


def test_project_folder_never_removed_as_empty(root, sent):
    (root / "proj" / ".git").mkdir(parents=True)  # an empty .git still marks a project
    ok, errs = core.trash_empty_dirs([root / "proj"], root)
    assert ok == 0 and len(errs) == 1 and sent == []


# ---------- 3. desktop.ini / Thumbs.db do not make a folder non-empty ----------

def test_folder_with_only_shell_junk_is_empty(root, sent):
    hide(touch(root / "a" / "desktop.ini"))
    touch(root / "a" / "b" / "Thumbs.db")
    found = core.find_empty_dirs(root)
    assert found == [root / "a" / "b", root / "a"]
    ok, errs = core.trash_empty_dirs(found, root)
    assert (ok, errs) == (2, []) and sent == [os.path.abspath(p) for p in found]


def test_other_hidden_file_still_counts(root):
    hide(touch(root / "a" / "notes.txt"))
    touch(root / "a" / "desktop.ini")
    assert core.find_empty_dirs(root) == []


# ---------- 4. sort touches top-level loose files only ----------

def test_sort_never_recurses(root):
    touch(root / "a.txt")
    touch(root / "sub" / "b.txt")
    assert [m.src.name for m in core.plan_sort(root)] == ["a.txt"]
    with pytest.raises(TypeError):
        core.plan_sort(root, recursive=True)


# ---------- 5. OneDrive online-only placeholders ----------

@pytest.mark.parametrize("flag", [0x400000, 0x40000])  # RECALL_ON_DATA_ACCESS, RECALL_ON_OPEN
def test_cloud_placeholders_never_hashed_or_moved(root, monkeypatch, flag):
    real = core._attrs
    monkeypatch.setattr(core, "_attrs", lambda st: real(st) | (flag if st.st_size == 7 else 0))
    hashed = []
    monkeypatch.setattr(core, "_hash", lambda p, limit, cancel: hashed.append(p))
    touch(root / "cloud1.bin", b"1234567", age_days=3)
    touch(root / "cloud2.bin", b"1234567")
    touch(root / "local.txt")
    assert names(root) == ["local.txt"]
    assert core.find_duplicates(root) == [] and hashed == []
    mv = core.Move(root / "cloud1.bin", root / "Other" / "cloud1.bin")
    n, errs = core.apply_moves([mv], root)
    assert n == 0 and len(errs) == 1 and (root / "cloud1.bin").exists()


# ---------- 6. no removals on network or removable drives ----------

@pytest.mark.parametrize("kind", [0, 1, 2, 4, 5, 6])  # unknown, no root, removable, network, CD, RAM disk
def test_recycle_refused_off_fixed_drives(root, monkeypatch, sent, kind):
    monkeypatch.setattr(core, "_drive_type", lambda p: kind)
    f = touch(root / "a.txt")
    (root / "empty").mkdir()
    ok, errs = core.to_recycle_bin([f], root)
    assert ok == 0 and len(errs) == 1 and sent == [] and f.exists()
    ok, errs = core.trash_empty_dirs([root / "empty"], root)
    assert ok == 0 and len(errs) == 1 and sent == []


def test_recycle_allowed_on_fixed_drive(root, monkeypatch, sent):
    monkeypatch.setattr(core, "_drive_type", lambda p: 3)
    f = touch(root / "a.txt")
    assert core.to_recycle_bin([f], root) == (1, [])


def test_drive_root_for_letters_and_unc():
    assert core._drive_root(r"C:\Users\me\x.txt") == "C:\\"
    assert core._drive_root(r"\\server\share\dir\x.txt") == "\\\\server\\share\\"


@pytest.mark.skipif(sys.platform != "win32", reason="GetDriveTypeW is Windows-only")
def test_real_drive_type_of_temp_folder(root):
    assert core._drive_type(root) == 3  # DRIVE_FIXED


# ---------- 7. one failure never aborts a batch ----------

@pytest.mark.skipif(sys.platform != "win32", reason="open files are only locked on Windows")
def test_locked_file_does_not_abort_moves(root):
    for n in ("a.txt", "b.txt", "c.txt"):
        touch(root / n)
    moves = core.plan_sort(root)
    with open(root / "b.txt", "rb"):  # Windows refuses to move an open file
        n, errs = core.apply_moves(moves, root)
    assert n == 2 and len(errs) == 1 and errs[0].startswith("b.txt")
    assert (root / "b.txt").exists()


def test_too_long_path_does_not_abort_batch(root):
    a, b = touch(root / "a.txt"), touch(root / "b.txt")
    long_dst = root / ("x" * 120) / ("y" * 120) / ("z" * 120) / "a.txt"  # over 260 chars
    n, errs = core.apply_moves([core.Move(a, long_dst), core.Move(b, root / "Documents" / "b.txt")], root)
    assert n + len(errs) == 2  # fails or works depending on the LongPathsEnabled policy, never raises
    assert (root / "Documents" / "b.txt").exists()


def test_recycle_error_does_not_abort_batch(root, monkeypatch):
    sent = []

    def fake(path):
        if path.endswith("b.txt"):
            raise OSError("locked")
        sent.append(path)

    monkeypatch.setattr(core, "send2trash", fake)
    files = [touch(root / n) for n in ("a.txt", "b.txt", "c.txt")]
    ok, errs = core.to_recycle_bin(files, root)
    assert ok == 2 and len(errs) == 1 and len(sent) == 2


def test_failing_group_does_not_abort_duplicates(root, monkeypatch, sent):
    touch(root / "a1.bin", b"aaaa", age_days=3)
    touch(root / "a2.bin", b"aaaa")
    touch(root / "b1.bin", b"bbbbb", age_days=3)
    touch(root / "b2.bin", b"bbbbb")
    groups = core.find_duplicates(root)
    real, calls = core.to_recycle_bin, []

    def flaky(paths, base):
        calls.append(paths)
        if len(calls) == 1:
            raise RuntimeError("boom")
        return real(paths, base)

    monkeypatch.setattr(core, "to_recycle_bin", flaky)
    ok, errs = core.trash_duplicates([(g, g.files[1:]) for g in groups], root)
    assert ok == 1 and len(errs) == 1 and len(calls) == 2
    assert sent == [os.path.abspath(groups[1].files[1])]


def test_undo_log_failure_is_reported_not_raised(root, monkeypatch):
    touch(root / "a.txt")

    def broken(done):
        raise OSError("disk full")

    monkeypatch.setattr(core, "_append_log", broken)
    n, errs = core.apply_moves(core.plan_sort(root), root)
    assert n == 1 and len(errs) == 1 and "undo" in errs[0].lower()


# ---------- 8. undo log lives in %LOCALAPPDATA% ----------

@pytest.mark.skipif("LOCALAPPDATA" not in os.environ, reason="Windows only")
def test_log_dir_is_local_appdata():
    assert REAL_LOG_DIR == Path(os.environ["LOCALAPPDATA"]) / "Declutter"


def test_old_undo_log_is_migrated(root):
    touch(root / "Documents" / "a.txt", b"A")
    core.OLD_LOG_DIR.mkdir()
    batch = [{"moves": [{"src": str(root / "a.txt"), "dst": str(root / "Documents" / "a.txt")}]}]
    (core.OLD_LOG_DIR / "undo.json").write_text(json.dumps(batch), encoding="utf-8")
    assert core.can_undo()
    assert core.UNDO_LOG.exists() and not (core.OLD_LOG_DIR / "undo.json").exists()
    assert core.undo_last() == (1, []) and (root / "a.txt").read_bytes() == b"A"


def test_new_log_wins_over_old(root):
    core.OLD_LOG_DIR.mkdir()
    (core.OLD_LOG_DIR / "undo.json").write_text("[]", encoding="utf-8")
    core.LOG_DIR.mkdir()
    core.UNDO_LOG.write_text('[{"moves": []}]', encoding="utf-8")
    assert core.can_undo() and (core.OLD_LOG_DIR / "undo.json").exists()


def test_old_settings_are_migrated():
    core.OLD_LOG_DIR.mkdir()
    (core.OLD_LOG_DIR / "settings.json").write_text('{"theme": "Dark"}', encoding="utf-8")
    assert theme.load_mode() == "Dark" and theme._SETTINGS.exists()


# ---------- 9. moves never overwrite ----------

def test_move_never_overwrites_file_that_appeared_after_plan(root):
    touch(root / "a.txt", b"new")
    moves = core.plan_sort(root)
    touch(root / "Documents" / "a.txt", b"existing")
    assert core.apply_moves(moves, root) == (1, [])
    assert (root / "Documents" / "a.txt").read_bytes() == b"existing"
    assert (root / "Documents" / "a (1).txt").read_bytes() == b"new"


def test_undo_never_overwrites(root):
    touch(root / "a.txt", b"moved")
    core.apply_moves(core.plan_sort(root), root)
    touch(root / "a.txt", b"newer")  # a new file took the old name
    assert core.undo_last() == (1, [])
    assert (root / "a.txt").read_bytes() == b"newer"
    assert (root / "a (1).txt").read_bytes() == b"moved"
