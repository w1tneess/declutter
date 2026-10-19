"""R5 UI rules that need no window: one themed dialog, and the Windows animation setting is honoured live."""
import re
from pathlib import Path

from declutter import app as A

APP_PY = Path(A.__file__)


def test_native_message_boxes_only_remain_as_the_fallback():
    """Every message goes through the themed alert(); a native box is only the last resort inside _show_error."""
    src = APP_PY.read_text(encoding="utf-8")
    assert re.findall(r"messagebox\.show\w+\(", src) == ["messagebox.showerror("]
    fallback = src[src.index("def _show_error"):]
    assert fallback.index("messagebox.showerror(") < fallback.index("\n    def ", 10)


class Win:
    def __init__(self):
        self.calls = []

    def attributes(self, *a):
        self.calls.append(a)

    def after(self, *a):
        self.calls.append(("after",) + a[:1])

    def winfo_exists(self):
        return True


class Label(Win):
    def __init__(self):
        super().__init__()
        self.text = None

    def configure(self, **kw):
        self.text = kw["text"]

    def after(self, *a):
        self.calls.append(("after",))


def test_fade_in_does_nothing_when_windows_animations_are_off(monkeypatch):
    monkeypatch.setattr(A, "animations_on", lambda: False)
    w = Win()
    A.fade_in(w)
    assert w.calls == []


def test_fade_in_starts_when_windows_animations_are_on(monkeypatch):
    monkeypatch.setattr(A, "animations_on", lambda: True)
    w = Win()
    A.fade_in(w)
    assert w.calls and w.calls[0] == ("-alpha", 0.0)


def test_count_up_jumps_to_the_number_when_animations_are_off(monkeypatch):
    monkeypatch.setattr(A, "animations_on", lambda: False)
    lab = Label()
    A.count_up(lab, 1234)
    assert lab.text == "1,234" and lab.calls == []


def test_the_setting_is_read_each_time_not_once_at_start(monkeypatch):
    state = {"on": True}
    monkeypatch.setattr(A, "animations_on", lambda: state["on"])
    first = Win()
    A.fade_in(first)
    state["on"] = False  # the user turns animations off in Windows while the app is open
    second = Win()
    A.fade_in(second)
    assert first.calls and second.calls == []
