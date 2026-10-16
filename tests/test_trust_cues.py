"""R1 trust cues: the data behind them (undo preview, ages, size pairs, reason names) and the UI copy rule."""
import time
from pathlib import Path

import pytest

from declutter import core

from test_core import touch

SRC = Path(__file__).resolve().parent.parent / "src" / "declutter"


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "Downloads"
    r.mkdir()
    return r


def log_batch(root, seconds_ago, moves=1):
    when = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(time.time() - seconds_ago))
    core._write_log([{"time": when, "moves": [{"src": str(root / f"f{i}.txt"), "dst": str(root / "Documents" / f"f{i}.txt")}
                                              for i in range(moves)]}])


# ---------- peek_undo ----------

def test_peek_undo_is_none_without_a_sort():
    assert core.peek_undo() is None


def test_peek_undo_describes_the_last_real_sort(root):
    for n in ("a.txt", "b.txt", "c.png"):
        touch(root / n, n.encode())
    assert core.apply_moves(core.plan_sort(root), root) == (3, [])
    info = core.peek_undo()
    assert (info.folder, info.count) == (root, 3) and 0 <= info.age_seconds < 60
    assert core.undo_last() == (3, [])  # peeking changed nothing
    assert core.peek_undo() is None


def test_peek_undo_age_comes_from_the_log(root):
    log_batch(root, 300, moves=14)
    info = core.peek_undo()
    assert info.count == 14 and 299 <= info.age_seconds < 330


@pytest.mark.parametrize("batch", [{"time": "yesterday", "moves": [{"src": "x"}]}, {"moves": [{"src": "x"}]}])
def test_peek_undo_without_a_readable_time_still_counts(batch):
    core._write_log([batch])
    info = core.peek_undo()
    assert info.count == 1 and info.age_seconds is None


@pytest.mark.parametrize("log", [[], ["junk"], [{"time": "2026-01-01 00:00:00", "moves": []}], [{"moves": "no"}]])
def test_peek_undo_ignores_broken_logs(log):
    core._write_log(log)
    assert core.peek_undo() is None


# ---------- ago ----------

@pytest.mark.parametrize("seconds, text", [
    (-5, "just now"), (0, "just now"), (59, "just now"), (60, "1 minute ago"), (300, "5 minutes ago"),
    (3599, "59 minutes ago"), (3600, "1 hour ago"), (7200, "2 hours ago"), (86400, "1 day ago"),
    (3 * 86400, "3 days ago"),
])
def test_ago(seconds, text):
    assert core.ago(seconds) == text


# ---------- size_pair ----------

def test_size_pair_shows_both_sizes():
    assert core.size_pair(2 * 1024**2, int(2.1 * 1024**2)) == "2.0 MB vs 2.1 MB"


def test_size_pair_uses_exact_bytes_when_rounding_would_hide_the_difference():
    assert core.size_pair(2_000_000, 2_001_000) == "2,000,000 B vs 2,001,000 B"


# ---------- why flagged ----------

def test_leftover_reasons_are_plain_words():
    assert (core.EXTRACTED, core.OLD_INSTALLER, core.DEAD_DOWNLOAD) == (
        "Unpacked .zip", "Old installer", "Unfinished download")


# ---------- copy ----------

def test_ui_copy_never_promises_safety():
    for f in SRC.glob("*.py"):
        text = f.read_text(encoding="utf-8").lower()
        for phrase in ("safe to delete", "guaranteed safe"):
            assert phrase not in text, f"{f.name} says {phrase!r}; say 'likely leftover'"
