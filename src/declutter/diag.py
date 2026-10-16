"""Local diagnostics: a small rotating log and a report the user can read before sharing it.

Nothing here touches the network. The log holds operation names, whole-number counts and tracebacks:
`log_op` accepts integers only, so file names and contents cannot be written through it. Logging never
raises: a read-only disk must not break a sort.
"""
from __future__ import annotations

import logging
import logging.handlers
import platform
import sys
import threading
from collections import deque
from pathlib import Path

from . import __version__, core

_NAME = "declutter"


def log_file() -> Path:
    return core.LOG_DIR / "logs" / "declutter.log"


def _logger() -> logging.Logger:
    return logging.getLogger(_NAME)


def close() -> None:
    """Release the log file (tests, and a changed data folder)."""
    log = _logger()
    for h in list(log.handlers):
        log.removeHandler(h)
        h.close()


def setup(max_bytes: int = 256 * 1024, backups: int = 3) -> None:
    """Attach a rotating file handler for the current log path. Safe to call again."""
    log = _logger()
    path = str(log_file())
    if any(getattr(h, "baseFilename", None) == path for h in log.handlers):
        return
    close()
    log.setLevel(logging.INFO)
    log.propagate = False  # keep other libraries' records out of our file
    try:
        log_file().parent.mkdir(parents=True, exist_ok=True)
        h = logging.handlers.RotatingFileHandler(path, maxBytes=max_bytes, backupCount=backups,
                                                 encoding="utf-8")
    except OSError:
        return  # no log, but the app carries on
    h.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    log.addHandler(h)


def log_op(op: str, **counts: int) -> None:
    """One line: the operation and its counts, e.g. `op=sort moved=3 errors=0`."""
    for k, v in counts.items():
        if type(v) is not int:
            raise TypeError(f"{k}: only whole numbers are logged, not {type(v).__name__}")
    setup()
    _logger().info("op=%s %s", op, " ".join(f"{k}={v}" for k, v in counts.items()))


def log_exception(where: str, exc_info=None) -> None:
    """Log the exception being handled (or `exc_info`) with its full traceback."""
    setup()
    _logger().error("error in %s", where, exc_info=exc_info or sys.exc_info())


def recent_lines(n: int = 200) -> list[str]:
    """The last n lines of the current log file, oldest first; empty if there is none."""
    try:
        with open(log_file(), encoding="utf-8", errors="replace") as f:
            return [line.rstrip("\n") for line in deque(f, maxlen=n)]
    except OSError:
        return []


def report(n: int = 200) -> str:
    """Text for "Copy diagnostics": versions plus the last log lines. The user reads it before sharing."""
    head = [f"Declutter {__version__}", f"Windows {platform.platform()}", f"Python {platform.python_version()}", ""]
    return "\n".join(head + (recent_lines(n) or ["(the log is empty)"]))


def install_hooks() -> None:
    """Log exceptions nobody caught: in any thread, and outside Tk callbacks."""
    def thread_hook(args) -> None:
        if args.exc_type is not SystemExit:
            log_exception(f"thread {args.thread.name if args.thread else '?'}",
                          (args.exc_type, args.exc_value, args.exc_traceback))

    def main_hook(exc_type, exc, tb) -> None:
        log_exception("main", (exc_type, exc, tb))
        sys.__excepthook__(exc_type, exc, tb)

    threading.excepthook = thread_hook
    sys.excepthook = main_hook
