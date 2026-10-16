"""Sort-by-type rules: recent-file grace period (A), shortcuts and config files (B), known folders (C).
Times are fixed numbers passed in as `now`; no test reads the real clock."""
import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from declutter import core

from test_core import touch

NOW = 1_800_000_000.0
H = 3600


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "messy"
    r.mkdir()
    return r


def entry(path, mtime, birth, size=1):
    return path, SimpleNamespace(st_size=size, st_mtime=mtime, st_birthtime=birth)


# ---------- A. recent-file grace period ----------

def test_grace_period_skips_at_24h_and_moves_at_0(root):
    e = [entry(root / "a.txt", NOW - H, NOW - H)]  # touched one hour ago
    plan = core.plan_sort(root, entries=e, recent_hours=24, now=NOW)
    assert list(plan) == [] and plan.recent_left == 1
    plan = core.plan_sort(root, entries=e, recent_hours=0, now=NOW)
    assert [m.src.name for m in plan] == ["a.txt"] and plan.recent_left == 0


def test_copy_with_old_mtime_but_new_creation_time_is_skipped(root):
    copy = entry(root / "copy.pdf", NOW - 30 * 86400, NOW - 600)  # copying keeps mtime, resets creation
    old = entry(root / "old.pdf", NOW - 30 * 86400, NOW - 30 * 86400)
    plan = core.plan_sort(root, entries=[copy, old], recent_hours=24, now=NOW)
    assert [m.src.name for m in plan] == ["old.pdf"] and plan.recent_left == 1


def test_creation_time_falls_back_to_ctime(root):
    st = SimpleNamespace(st_size=1, st_mtime=NOW - 30 * 86400, st_ctime=NOW - 600)  # no st_birthtime
    plan = core.plan_sort(root, entries=[(root / "a.txt", st)], recent_hours=24, now=NOW)
    assert list(plan) == [] and plan.recent_left == 1


def test_recent_count_only_counts_files_that_would_be_sorted(root):
    e = [entry(root / "new1.txt", NOW - H, NOW - H),
         entry(root / "new2.txt", NOW - 30 * 86400, NOW - 2 * H),
         entry(root / "old.txt", NOW - 30 * 86400, NOW - 30 * 86400),
         entry(root / "new.lnk", NOW - H, NOW - H),            # never sorted, so not "left alone for being recent"
         entry(root / "sub" / "new3.txt", NOW - H, NOW - H)]   # not top level
    plan = core.plan_sort(root, entries=e, recent_hours=24, now=NOW)
    assert [m.src.name for m in plan] == ["old.txt"] and plan.recent_left == 2


def test_real_scan_stats_carry_a_creation_time(root):
    touch(root / "copy.txt", age_days=30)  # old mtime, created just now (what a copy looks like)
    entries = core.scan_entries(root)
    now = core.last_touched(entries[0][1]) + H
    plan = core.plan_sort(root, entries=entries, recent_hours=24, now=now)
    assert list(plan) == [] and plan.recent_left == 1


def test_apply_moves_rechecks_grace_period(root):
    f = touch(root / "a.txt", age_days=30)
    now = core.last_touched(os.stat(f)) + H
    mv = core.Move(f, root / "Documents" / "a.txt")
    n, errs = core.apply_moves([mv], root, recent_hours=24, now=now)
    assert n == 0 and len(errs) == 1 and f.exists()
    assert core.apply_moves([mv], root, recent_hours=0, now=now) == (1, [])


@pytest.mark.parametrize("raw, want", [(24, 24), ("12", 12), (0, 0), (-5, 0), (100, 72), (72, 72), (3.9, 3),
                                       ("abc", 24), (None, 24), (True, 24), ("nan", 24), ("inf", 24)])
def test_recent_hours_setting_is_clamped(raw, want):
    assert core.recent_hours_setting(raw) == want
    assert core.recent_hours_setting("junk", default=5) == 5


# ---------- B. shortcuts and config files are never moved ----------

def test_shortcuts_and_ini_files_are_never_sorted(root):
    for n in ("a.lnk", "b.url", "desktop.ini", "c.ini", "d.txt", "E.LNK"):
        touch(root / n)
    assert [m.src.name for m in core.plan_sort(root)] == ["d.txt"]


# ---------- C. known folders ----------

def test_downloads_comes_from_the_shell_not_a_name(root, monkeypatch):
    moved = root / "D-drive stuff"
    monkeypatch.setattr(core, "_known_folder_path", lambda folder_id: moved)
    assert core.known_folder("Downloads") == moved
    assert core.in_downloads(moved / "setup.exe")
    assert core.in_downloads(moved)
    assert not core.in_downloads(Path.home() / "Downloads" / "setup.exe")  # a string match would say yes
    assert not core.in_downloads(root / "other" / "setup.exe")


def test_known_folder_asks_for_the_downloads_id(monkeypatch):
    asked = []
    monkeypatch.setattr(core, "_known_folder_path", lambda folder_id: asked.append(folder_id))
    core.known_folder("Downloads")
    assert asked == ["{374DE290-123F-4565-9164-39C4925E467B}"]  # FOLDERID_Downloads


def test_known_folder_falls_back_to_home(root, monkeypatch):
    monkeypatch.setattr(core, "_known_folder_path", lambda folder_id: None)  # off Windows, or the call failed
    monkeypatch.setattr(Path, "home", classmethod(lambda cls: root))
    assert core.known_folder("Downloads") == root / "Downloads"
    assert core.in_downloads(root / "Downloads" / "x.exe")


def test_real_downloads_folder_is_an_absolute_path():
    p = core._known_folder_path(core.KNOWN_FOLDERS["Downloads"])
    if p is None:
        pytest.skip("the shell has no Downloads folder here")
    assert p.is_absolute()
