"""GUI release checks from SPEC.md Testing: a 10,000+ file folder without freezing, cancelling a scan,
theme switching, 100/125/150% scaling. Uses temp folders only; APPDATA and LOCALAPPDATA are redirected.

Not collected by pytest (it opens real windows). Run from the repo root:
    python tests/gui_check.py
Screenshots go to .pytest_tmp/shots when Pillow is installed (pip install pillow).
"""
import os
import shutil
import sys
import tempfile
import time
import zipfile
from pathlib import Path

repo = Path(__file__).resolve().parent.parent
scratch = repo / ".pytest_tmp"  # %TEMP% is under AppData, which the app refuses
scratch.mkdir(exist_ok=True)
base = Path(tempfile.mkdtemp(prefix="declutter_check_", dir=scratch))
shots = scratch / "shots"
shots.mkdir(exist_ok=True)
os.environ["APPDATA"] = str(base / "appdata")
os.environ["LOCALAPPDATA"] = str(base / "localappdata")
sys.path.insert(0, str(repo / "src"))

import _tkinter  # noqa: E402

import customtkinter as ctk  # noqa: E402

try:
    from PIL import ImageGrab
except ImportError:
    ImageGrab = None

from declutter import app as A, core  # noqa: E402

assert str(core.LOG_DIR).startswith(str(base)), core.LOG_DIR
A.reveal = lambda p: None

# ---- 10,000+ file folder: 100 subfolders x 101 files, 300 duplicate pairs, 2 big files, 20 empty folders
big = base / "big"
t0 = time.time()
for d in range(100):
    sub = big / f"folder{d:03}"
    sub.mkdir(parents=True)
    for i in range(101):
        ext = (".jpg", ".pdf", ".txt", ".mp3", ".zip", ".py")[i % 6]
        content = f"dup{(d * 101 + i) % 300}" if (d * 101 + i) < 600 else f"unique {d} {i}"
        (sub / f"file{i:03}{ext}").write_text(content)
for i in range(2):
    (big / f"huge{i}.bin").write_bytes(b"z" * (2_000_000 + i))
for i in range(20):
    (big / "empties" / f"e{i}").mkdir(parents=True)
old = time.time() - 3600  # files changed in the last 5 minutes are skipped as in use
files = [f for f in big.rglob("*") if f.is_file()]
for f in files:
    os.utime(f, (old, old))

# ---- small folder for leftovers, re-downloads, sub-buckets and the grace period (scanned near the end)
small = base / "small"


def put(rel, data=b"x", days=10):
    p = small / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(data)
    t = time.time() - days * 86400
    os.utime(p, (t, t))
    return p


put("Downloads/setup.exe", b"MZ", days=10)
put("Downloads/dead.crdownload", b"half", days=5)
with zipfile.ZipFile(small / "pack.zip", "w") as z:
    z.writestr("a.txt", b"hello")
put("pack/a.txt", b"hello")
os.utime(small / "pack.zip", (old, old))
put("docs/report.pdf", b"x" * 1000)
put("docs/report (1).pdf", b"y" * 1005)
for n in ("Screenshot 1.png", "WhatsApp Image 1.jpg", "photo.png", "a.lnk"):
    put(n, n.encode())  # different bytes, so they are not exact duplicates of each other
n_files = len(files)
print(f"made {n_files} files in {time.time() - t0:.1f}s")
assert n_files >= 10_000

app = A.App()
app.geometry("1100x720+40+40")
app.recent_text.set("0")  # the 24 h default would hold back files this script just created
app._set_recent()
app.update()

worst = 0.0


def pump(until, timeout=120):
    """Process one Tk event at a time, like mainloop, and record the slowest one: that is the freeze a user feels.
    (app.update() would run a whole chain of after() callbacks in one call and overstate it.)"""
    global worst
    end = time.time() + timeout
    while time.time() < end:
        s = time.perf_counter()
        busy = app.tk.dooneevent(_tkinter.DONT_WAIT)
        worst = max(worst, time.perf_counter() - s)
        if until():
            return
        if not busy:
            time.sleep(0.002)
    raise AssertionError(f"timed out; busy={app._busy}, status={app.status.cget('text')!r}")


def settle(ms=600):
    end = time.time() + ms / 1000
    while time.time() < end:
        app.update()
        time.sleep(0.01)


def shoot(name, win=None):
    win = win or app
    win.attributes("-topmost", True)  # otherwise other windows end up in the picture
    win.lift()
    win.update()
    time.sleep(0.15)
    win.update()
    if ImageGrab is None:
        return
    x, y = win.winfo_rootx(), win.winfo_rooty()
    ImageGrab.grab(bbox=(x, y, x + win.winfo_width(), y + win.winfo_height()), all_screens=True).save(shots / f"{name}.png")


def clipped(widget):
    """True if the widget sticks out past the window's right or bottom edge."""
    r = widget.winfo_rootx() + widget.winfo_width() - (app.winfo_rootx() + app.winfo_width())
    b = widget.winfo_rooty() + widget.winfo_height() - (app.winfo_rooty() + app.winfo_height())
    return r > 1 or b > 1


# ---- cancel a scan
app.set_folder(big)
app.min_mb.set("1")
app.scan()
pump(lambda: app.cancel_btn.winfo_ismapped(), timeout=5)
app.cancel_btn.invoke()
pump(lambda: not app._busy)
assert app.status.cget("text").startswith("Cancelled"), app.status.cget("text")
print("cancel OK:", app.status.cget("text"))

# ---- full scan of 10k files, measuring the longest single UI update (a freeze shows up here)
worst = 0.0
t0 = time.time()
app.scan()
pump(lambda: not app._busy and app.entries is not None)
scan_s = time.time() - t0
pump(lambda: len(app.dups.rows) == 100, timeout=30)  # chunked row building, still timed per event
print(f"scan {scan_s:.1f}s, longest UI update {worst * 1000:.0f} ms, dup groups {len(app.dups.groups)}, "
      f"large {len(app.large.rows)}, empty rows {len(app.empty.rows)}")
assert worst < 0.5, f"UI blocked for {worst:.2f}s"
assert len(app.dups.groups) == 100  # capped at MAX_ROWS
assert len(app.large.rows) == 2

# ---- layout: Home cards sit below the Scan button; an empty tab header takes no space
app.show("Home")
settle(300)
assert all(c.winfo_rooty() >= app.scan_btn.winfo_rooty() + app.scan_btn.winfo_height() for c, _, _ in app.cards.values())
settle(800)  # count-up finished
assert [n.cget("text") for _, n, _ in app.cards.values()] == ["2", "300", "2", "21"], \
    [n.cget("text") for _, n, _ in app.cards.values()]
assert float(app.attributes("-alpha")) == 1.0, "window must end fully opaque after the fade-in"

# ---- motion and feedback: disabled primary looks disabled, toast comes and goes, shortcuts
assert app.large.action_btn.cget("state") == "disabled" and app.large.action_btn.cget("fg_color") != A.T.ACCENT
app.large.select_all(True)
assert app.large.action_btn.cget("state") == "normal" and app.large.action_btn.cget("fg_color") == A.T.ACCENT
app.large.select_all(False)
app.toast("Moved 3 files.", ms=600)
settle(300)
assert app._toast.winfo_ismapped()
shoot("toast")
settle(1200)
assert not app._toast.winfo_exists(), "toast must hide itself"
app.focus_force()
app.event_generate("<Control-Key-2>")
settle(100)
assert app.current == "Sort", app.current
app.show("Duplicates")
app.event_generate("<Control-f>")
settle(100)
assert app.focus_get() is not None and str(app.focus_get()).startswith(str(app.dups.search))
print("motion + shortcuts OK")
app.show("Duplicates")
settle(300)
assert app.dups.head.winfo_height() <= 2, app.dups.head.winfo_height()
assert app.large.head.winfo_height() > 20
print("layout OK")

# ---- "Keep" label follows the chosen copy
box = app.dups.rows[0][0]
radios = [w for w in box.winfo_children() if isinstance(w, ctk.CTkRadioButton)]
assert radios[0].cget("text").startswith("Keep") and not radios[1].cget("text").startswith("Keep")
radios[1].invoke()
assert not radios[0].cget("text").startswith("Keep") and radios[1].cget("text").startswith("Keep")
g = app.dups.groups[0][2]
assert g.files[1] not in app.dups.selected() and g.files[0] in app.dups.selected()
radios[0].invoke()
print("keeper label OK")

# ---- Sort: a checkbox per file, the button says how many; a real sort shows the undo banner, undo hides it
assert app.sort.action_btn.cget("text") == "Sort 2 files", app.sort.action_btn.cget("text")
app.sort.files[0][0].set(False)
app.sort.update_selection()
assert app.sort.action_btn.cget("text") == "Sort 1 file"
app.sort.select_all(True)
assert not app.undo_bar.winfo_manager(), "no undo banner before any sort"
undo_asked = []
real_confirm, A.confirm = A.confirm, lambda *a: undo_asked.append(a) or True
app.show("Sort")
app.move()
pump(lambda: not app._busy)                          # the move
pump(lambda: not app._busy and app.sort.files == [])  # the rescan after it
assert app.undo_bar.winfo_manager() and "2 files" in app.undo_text.cget("text"), app.undo_text.cget("text")
assert (big / "Other" / "huge0.bin").exists()
shoot("sort_undo_banner")
app.undo()
pump(lambda: not app._busy)
pump(lambda: not app._busy and len(app.sort.files) == 2)
assert not app.undo_bar.winfo_manager() and (big / "huge0.bin").exists()
msg = undo_asked[-1][2]
assert msg.startswith(f"Undo the sort of 2 files in {big.name} (just now)?"), msg
A.confirm = real_confirm
print("sort + undo banner OK")

# ---- pins: a pinned folder disappears from every list, and is back after unpinning
app._set_pins([str(big / "folder000")])
pump(lambda: not app._busy and app.entries is not None)
assert not any(core.is_pinned(p, app.pins) for p, _ in app.entries)
app._set_pins([])
pump(lambda: not app._busy and app.entries is not None)
assert any(p.parent.name == "folder000" for p, _ in app.entries)
print("pins OK")

# ---- keyboard: Tab focus shows a ring, Space activates, and a focused row scrolls into view
app.show("Home")
app.focus_force()  # focus events only fire while this window has the OS focus
settle(200)
app.scan_btn.focus_set()
settle(100)
assert app.scan_btn.cget("border_width") == 2 and app.scan_btn.cget("border_color") == A.T.FOCUS
app.focus_get().tk_focusNext().focus_set()
settle(100)
assert app.scan_btn.cget("border_width") != 2, "ring must go when focus leaves"
app.show("Empty Folders")
pump(lambda: len(app.empty.rows) == 21)
settle(300)
last = app.empty.rows[-1][0]
last.focus_set()
settle(300)
assert "underline" in last.cget("font"), last.cget("font")
assert app.empty.list._parent_canvas.yview()[0] > 0, "focused row must scroll into view"
before = app.empty.items[-1][0].get()
last._text_label.event_generate("<space>")
assert app.empty.items[-1][0].get() != before, "Space toggles a focused checkbox"
last._text_label.event_generate("<space>")
for mode in ("Light", "Dark"):
    app._set_theme(mode)
    settle(300)
    shoot(f"focus_{mode}")
app.show("Home")
app.scan_btn.focus_set()
settle(200)
shoot("focus_button_Light")
app._set_theme("Light")
print("keyboard focus OK")

# ---- actions get no Cancel button, and their result is always reported
seen, done = [], []
app._run(lambda: (time.sleep(0.4), (0, []))[1], done.append, cancellable=False)
pump(lambda: (seen.append(app.cancel_btn.winfo_ismapped()) or not app._busy))
assert done == [(0, [])] and not any(seen), (done, seen)
print("non-cancellable job OK")

# ---- screenshots: both themes, 100/125/150 % scaling, all pages, at the minimum window size too
for scale in (1.0, 1.25, 1.5):
    ctk.set_widget_scaling(scale)
    ctk.set_window_scaling(scale)
    for mode in ("Light", "Dark"):
        app._set_theme(mode)
        for size in ("1100x720", "860x560"):
            app.geometry(size)
            settle(400)
            for page in ("Home", "Duplicates", "Large & Old", "Empty Folders", "Sort", "Settings"):
                app.show(page)
                settle(250)
                if size == "1100x720" or page != "Home":
                    tab = app.pages[page]
                    btn = getattr(tab, "action_btn", None)
                    if btn is not None and clipped(btn):
                        print(f"CLIPPED: {page} action button at {scale} {mode} {size}")
                if size == "1100x720" and (mode == "Light" or scale == 1.25):
                    shoot(f"{int(scale * 100)}_{mode}_{page.replace(' & ', '_').replace(' ', '_')}")
            if size == "860x560":
                shoot(f"{int(scale * 100)}_{mode}_min_Settings")
                app.show("Duplicates")
                settle(250)
                shoot(f"{int(scale * 100)}_{mode}_min_Duplicates")
ctk.set_widget_scaling(1.0)
ctk.set_window_scaling(1.0)
app._set_theme("Light")
app.geometry("1100x720")
settle(400)

# ---- confirm dialog screenshot (Esc closes it, so the answer must be False)
for mode in ("Light", "Dark"):
    app._set_theme(mode)
    settle(300)
    tops = []
    app.after(700, lambda m=mode: (shoot(f"dialog_{m}", [w for w in app.winfo_children()
                                                        if isinstance(w, ctk.CTkToplevel)][-1]),
                                   tops.append(1),
                                   [w for w in app.winfo_children() if isinstance(w, ctk.CTkToplevel)][-1]
                                   .event_generate("<Escape>")))
    assert app._confirm_bin(3) is False and tops
print("dialog OK")

# ---- leftovers, re-downloads, sub-buckets, grace period on the small folder (Downloads is mocked to its subfolder)
A.core._known_folder_path = lambda folder_id: small / "Downloads"
sent, asked = [], []
A.core.send2trash = sent.append
A.confirm = lambda *a: asked.append(a) or True
worst = 0.0
app.set_folder(small)
app.scan()
pump(lambda: not app._busy and app.entries is not None)
names = lambda tab: sorted(o.name for _, o in tab.items if o is not None)  # noqa: E731
assert names(app.large) == ["dead.crdownload", "pack.zip", "setup.exe"], names(app.large)
assert not any(v.get() for v, _ in app.large.items), "leftovers must start unticked"
assert app.large.rows[0][0].cget("text").startswith("Likely leftovers (3)"), app.large.rows[0][0].cget("text")
assert {m.category for m in app.moves} == {"Images/Screenshots", "Images/Chat", "Images", "Archives"}, \
    {m.category for m in app.moves}
assert "a.lnk" not in {m.src.name for m in app.moves}
assert [c[1].copy.name for c in app.dups.copies] == ["report (1).pdf"] and not app.dups.copies[0][0].get()
assert app.dups.selected() == [], app.dups.selected()
assert "possible re-download" in app.cards["Duplicates"][2].cget("text"), app.cards["Duplicates"][2].cget("text")
labels = lambda frame: [c.cget("text") for c in frame.winfo_children() if isinstance(c, ctk.CTkLabel)]  # noqa: E731
assert sorted(t for w, _, _ in app.large.rows for t in labels(w)) == [
    "Old installer", "Unfinished download", "Unpacked .zip (pack)"], [labels(w) for w, _, _ in app.large.rows]
pill = [t for w, _, _ in app.dups.rows for t in labels(w) if t.startswith("Numbered copy")]
assert len(pill) == 1 and " vs " in pill[0], pill
app._toggle_leftovers()
assert names(app.large) == ["dead.crdownload", "pack.zip", "setup.exe"]
app._toggle_leftovers()
app.show("Large & Old")
settle(300)
shoot("leftovers")
app.show("Duplicates")
settle(300)
shoot("redownloads")
app.dups.select_all(True)
assert app.dups.selected() == [], "Select all must not tick possible re-downloads"
assert app.dups.sel_label.cget("text") == "Selected to reclaim: 0 B", app.dups.sel_label.cget("text")
app.dups.copies[0][0].set(True)
app.dups.update_selection()
assert app.dups.sel_label.cget("text").startswith("Selected to reclaim: ") and \
    app.dups.sel_label.cget("text").endswith("· 1 item"), app.dups.sel_label.cget("text")
app.remove_dups()
pump(lambda: not app._busy)
assert asked[-1][3] == "Send 1 item" and sent == [os.path.abspath(small / "docs" / "report (1).pdf")], (asked[-1], sent)
app.show("Large & Old")
app.scan()
pump(lambda: not app._busy and app.entries is not None)
app.large.select_all(True)
assert app.large.sel_label.cget("text").startswith("Selected to reclaim: ") and \
    app.large.sel_label.cget("text").endswith("· 3 items"), app.large.sel_label.cget("text")
app.large.action_btn.invoke()
pump(lambda: not app._busy)
assert asked[-1][3] == "Send 3 items" and len(sent) == 4, (asked[-1], sent)
app.recent_text.set("24")  # everything here was created a moment ago
app._set_recent()
pump(lambda: not app._busy)
assert "recent files left alone" in app.sort_hint.cget("text"), app.sort_hint.cget("text")
assert app.sort.files == [] and app.settings is not None and A.T.load_settings()["recent_hours"] == 24
print(f"leftovers + re-downloads + sub-buckets + grace period OK, longest UI update {worst * 1000:.0f} ms")

# ---- diagnostics: operations are logged, About has two buttons, errors are logged and shown in the themed dialog
from declutter import __version__, diag  # noqa: E402
log = "\n".join(diag.recent_lines(500))
for op in ("op=scan", "op=sort", "op=undo", "op=recycle"):
    assert op in log, (op, log[-600:])
buttons = []
walk = lambda w: [buttons.append(w.cget("text")) if isinstance(w, ctk.CTkButton) else None] and [walk(c) for c in w.winfo_children()]  # noqa: E731
walk(app.pages["Settings"])
assert "Open log folder" in buttons and "Copy diagnostics" in buttons, buttons
found = {}
find = lambda w: [found.update({w.cget("text"): w}) if isinstance(w, ctk.CTkButton) else None] and [find(c) for c in w.winfo_children()]  # noqa: E731
find(app.pages["Settings"])
app.show("Settings")
for mode in ("Light", "Dark"):
    app._set_theme(mode)
    settle(300)
    app.pages["Settings"]._parent_canvas.yview_moveto(1)  # the About card is at the bottom
    settle(300)
    for name in ("Copy diagnostics", "Open log folder"):
        b = found[name]
        assert b.winfo_ismapped() and not clipped(b) and b.winfo_rooty() >= app.winfo_rooty(), (mode, name)
    shoot(f"about_{mode}")
opened = []
A.os.startfile = opened.append
app.open_logs()
assert opened == [str(diag.log_file().parent)] and diag.log_file().parent.is_dir(), opened
app.copy_diagnostics()
clip = app.clipboard_get()
assert f"Declutter {__version__}" in clip and "op=scan" in clip, clip[:300]
alerts = []
real_alert, A.alert = A.alert, lambda master, title, message: alerts.append((title, message))
app._on_ui_error(ValueError, ValueError("kaboom"), None)
assert alerts[-1][0] == "Something went wrong" and "kaboom" in alerts[-1][1] and "Open log folder" in alerts[-1][1], alerts
assert "ValueError: kaboom" in "\n".join(diag.recent_lines(500))
app._run(lambda: 1 / 0, lambda r: None, cancellable=False)
pump(lambda: len(alerts) == 2)
assert "division by zero" in alerts[-1][1] and "ZeroDivisionError" in "\n".join(diag.recent_lines(500)), alerts
A.alert = real_alert
A.confirm = real_confirm  # the real dialog, for the screenshots
for mode in ("Light", "Dark"):
    app._set_theme(mode)
    settle(300)
    tops = []
    app.after(700, lambda m=mode: (shoot(f"alert_{m}", [w for w in app.winfo_children()
                                                       if isinstance(w, ctk.CTkToplevel)][-1]),
                                   tops.append(1),
                                   [w for w in app.winfo_children() if isinstance(w, ctk.CTkToplevel)][-1]
                                   .event_generate("<Escape>")))
    A.alert(app, "Something went wrong", "Example error text.\n\nThe app is still running.")
    assert tops
print("diagnostics OK")
app._set_theme("System")
app.destroy()

# ---- a window taller than the screen (here 200% scaling, like 1080p at 150%) is maximised
ctk.set_window_scaling(2.0)
ctk.set_widget_scaling(2.0)
app = A.App()
settle(1000)
print("2.0x window:", app.winfo_height(), "screen:", app.winfo_screenheight(), "state:", app.state())
assert app.state() == "zoomed" and not clipped(app.scan_btn)
app.destroy()
shutil.rmtree(base, ignore_errors=True)
print("CHECK OK", shots)
