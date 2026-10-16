"""R2 diagnostics: a rotating local log (operation, counts, tracebacks), a copyable report, exception hooks."""
import logging
import platform
import sys
import threading

import pytest

from declutter import __version__, core, diag


@pytest.fixture(autouse=True)
def fresh_log():
    yield
    diag.close()  # release the file so Windows can clean the temp dir


def text() -> str:
    return diag.log_file().read_text(encoding="utf-8")


def test_log_lives_in_the_data_folder():
    diag.log_op("sort", moved=3)
    assert diag.log_file() == core.LOG_DIR / "logs" / "declutter.log" and diag.log_file().is_file()


def test_log_op_records_operation_and_counts_only():
    diag.log_op("sort", moved=3, errors=0)
    assert text().strip().endswith("op=sort moved=3 errors=0")


@pytest.mark.parametrize("bad", ["C:\\Users\\me\\secret.docx", b"bytes", None, 1.5])
def test_log_op_refuses_anything_but_whole_numbers(bad):
    with pytest.raises(TypeError):
        diag.log_op("sort", moved=bad)  # file names and contents can never end up in the log
    assert not diag.log_file().exists() or "secret" not in text()


def test_log_exception_keeps_the_full_traceback():
    try:
        raise ValueError("boom")
    except ValueError:
        diag.log_exception("scan")
    log = text()
    assert "scan" in log and "Traceback (most recent call last)" in log and "ValueError: boom" in log


def test_setup_twice_does_not_duplicate_lines():
    diag.setup()
    diag.setup()
    diag.log_op("undo", restored=1)
    assert text().count("op=undo") == 1


def test_log_rotates_and_stays_small(tmp_path):
    diag.setup(max_bytes=400, backups=2)
    for i in range(60):
        diag.log_op("scan", files=i)
    names = sorted(p.name for p in diag.log_file().parent.iterdir())
    assert names == ["declutter.log", "declutter.log.1", "declutter.log.2"], names
    assert sum(p.stat().st_size for p in diag.log_file().parent.iterdir()) < 3 * 400 + 400


def test_recent_lines_returns_the_last_n_in_order():
    for i in range(300):
        diag.log_op("scan", files=i)
    got = diag.recent_lines(200)
    assert len(got) == 200 and got[-1].endswith("files=299") and got[0].endswith("files=100")


def test_recent_lines_without_a_log_is_empty():
    assert diag.recent_lines(200) == []


def test_report_has_versions_and_the_log(monkeypatch):
    diag.log_op("sort", moved=2)
    report = diag.report()
    assert f"Declutter {__version__}" in report and platform.platform() in report
    assert f"Python {platform.python_version()}" in report and "op=sort moved=2" in report


def test_logging_problems_never_reach_the_app(monkeypatch):
    def boom(*a, **k):
        raise OSError("read-only disk")
    monkeypatch.setattr(logging.handlers, "RotatingFileHandler", boom)
    diag.close()
    diag.log_op("sort", moved=1)  # must not raise
    diag.log_exception("sort")
    assert diag.recent_lines(10) == []


def test_unhandled_thread_exceptions_are_logged(monkeypatch):
    monkeypatch.setattr(threading, "excepthook", threading.excepthook)  # restored after the test
    monkeypatch.setattr(sys, "excepthook", sys.excepthook)
    diag.install_hooks()

    def work():
        raise RuntimeError("worker died")

    t = threading.Thread(target=work, name="scan-worker")
    t.start()
    t.join()
    log = text()
    assert "scan-worker" in log and "RuntimeError: worker died" in log
