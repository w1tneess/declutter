"""Declutter GUI (CustomTkinter). All file logic lives in core.py."""
from __future__ import annotations

import queue
import threading
from collections import Counter
from pathlib import Path
from tkinter import filedialog, messagebox
from typing import Callable

import customtkinter as ctk

from . import core

FONT = "Segoe UI"
ACCENT = "#2563eb"
GHOST = dict(fg_color="transparent", border_width=1, text_color=("gray10", "gray90"))
DANGER = "#c2410c"
# CustomTkinter widgets are slow to build, so long lists are capped to keep the window responsive.
MAX_ROWS = {"Duplicates": 100, "Large & Old": 200, "Empty Folders": 300}
NAMES = ("Home", "Sort", "Duplicates", "Large & Old", "Empty Folders")
HELP = {
    "Sort": "Moves loose files into folders by type (Images, Documents, …). Nothing is deleted.",
    "Duplicates": "Files with identical content. The oldest copy is kept; pick a different one if you prefer.",
    "Large & Old": "Big or long-untouched files. Nothing is selected for you, so tick what you want gone.",
    "Empty Folders": "Folders with no files inside, however deep.",
}
_KNOWN = {  # Windows Known Folder IDs, so OneDrive-redirected folders resolve correctly
    "Downloads": "{374DE290-123F-4565-9164-39C4925E467B}",
    "Desktop": "{B4BFCC3A-DB2C-424C-B029-7FE99A87C641}",
    "Documents": "{FDD39AD0-238F-46AF-ADB4-6C85480369C7}",
}


def file_size(p: Path) -> int:
    try:
        return p.stat().st_size
    except OSError:
        return 0


def plural(n: int, word: str) -> str:
    return f"{n} {word}" if n == 1 else f"{n} {word}s"


def known_folder(name: str) -> Path:
    """Real location of Downloads/Desktop/Documents; falls back to ~/<name>."""
    try:
        import ctypes
        from ctypes import wintypes
        import uuid
        guid = uuid.UUID(_KNOWN[name])
        g = (ctypes.c_byte * 16).from_buffer_copy(guid.bytes_le)
        out = ctypes.c_wchar_p()
        if ctypes.windll.shell32.SHGetKnownFolderPath(ctypes.byref(g), 0, None, ctypes.byref(out)) == 0:
            path = Path(out.value)
            ctypes.windll.ole32.CoTaskMemFree(out)
            return path
    except Exception:
        pass
    return Path.home() / name


class Tab(ctk.CTkFrame):
    """Title, optional header row, scrollable checkbox list, and a footer with one action."""

    def __init__(self, master, title: str, action: str, on_action: Callable[[], None],
                 size_of: Callable[[object], int] | None = None, subtitle: str = ""):
        super().__init__(master, fg_color="transparent")
        self.size_of = size_of
        self.locked = False  # True while a background job runs
        self.items: list[tuple[ctk.BooleanVar, object]] = []
        self.grid_columnconfigure(0, weight=1)
        self.grid_rowconfigure(2, weight=1)
        top = ctk.CTkFrame(self, fg_color="transparent")
        top.grid(row=0, sticky="ew")
        ctk.CTkLabel(top, text=title, font=(FONT, 22, "bold")).pack(anchor="w")
        ctk.CTkLabel(top, text=subtitle, font=(FONT, 13), text_color=("gray35", "gray65"),
                     wraplength=640, justify="left").pack(anchor="w", pady=(2, 0))
        self.head = ctk.CTkFrame(self, fg_color="transparent")
        self.head.grid(row=1, sticky="ew", pady=8)
        self.list = ctk.CTkScrollableFrame(self, corner_radius=12)
        self.list.grid(row=2, sticky="nsew")
        foot = ctk.CTkFrame(self, fg_color="transparent")
        foot.grid(row=3, sticky="ew", pady=(14, 0))
        self.sel_label = ctk.CTkLabel(foot, text="Selected: 0 items", font=(FONT, 13))
        self.sel_label.pack(side="left")
        self.action_btn = ctk.CTkButton(foot, text=action, corner_radius=10, command=on_action,
                                        state="disabled")
        self.action_btn.pack(side="right")
        ctk.CTkButton(foot, text="Select none", width=90, corner_radius=10, **GHOST,
                      command=lambda: self.select_all(False)).pack(side="right", padx=(0, 10))
        ctk.CTkButton(foot, text="Select all", width=90, corner_radius=10, **GHOST,
                      command=lambda: self.select_all(True)).pack(side="right", padx=(0, 6))
        self.clear("Scan a folder on Home first.")

    _gen = 0  # bumped on every rebuild so an older, still-running chunked build stops itself

    def clear(self, message: str) -> None:
        self._reset()
        ctk.CTkLabel(self.list, text=message, font=(FONT, 14)).pack(pady=40)
        self.update_selection()

    def _reset(self) -> None:
        self._gen += 1
        for w in self.list.winfo_children():
            w.destroy()
        self.items = []

    def _build_chunked(self, rows: list, make: Callable[[object], None], chunk: int = 12) -> None:
        """Create widgets a few at a time so the window keeps repainting and stays responsive."""
        gen = self._gen

        def step(i: int) -> None:
            if gen != self._gen:
                return
            for r in rows[i:i + chunk]:
                make(r)
            self.update_selection()
            if i + chunk < len(rows):
                self.after(5, step, i + chunk)

        step(0)

    def set_rows(self, rows: list[tuple[str, object]], checked: bool, note: str = "") -> None:
        if not rows:
            return self.clear("Nothing to clean. Nice.")
        self._reset()
        if note:
            ctk.CTkLabel(self.list, text=note, font=(FONT, 12), anchor="w").pack(fill="x", padx=12, pady=4)

        def make(row) -> None:
            text, obj = row
            v = ctk.BooleanVar(value=checked)
            self.items.append((v, obj))
            ctk.CTkCheckBox(self.list, text=text, variable=v, font=(FONT, 13),
                            command=self.update_selection).pack(anchor="w", padx=12, pady=4)

        self._build_chunked(rows, make)

    def select_all(self, on: bool) -> None:
        for v, _ in self.items:
            v.set(on)
        self.update_selection()

    def selected(self) -> list:
        return [o for v, o in self.items if v.get()]

    def update_selection(self) -> None:
        sel = self.selected()
        text = f"Selected: {plural(len(sel), 'item')}"
        if self.size_of and sel:
            text += f", {core.human_size(sum(self.size_of(o) for o in sel))}"
        self.sel_label.configure(text=text)
        self.action_btn.configure(state="normal" if sel and not self.locked else "disabled")


class DupTab(Tab):
    """One row per group: a radio per copy picks the keeper; a checkbox includes the group."""

    def __init__(self, master, on_action, size_of):
        self.groups: list[tuple[ctk.BooleanVar, ctk.IntVar, core.DupGroup]] = []
        super().__init__(master, "Duplicates", "Send to Recycle Bin", on_action, size_of=size_of,
                         subtitle=HELP["Duplicates"])

    def _reset(self) -> None:
        self.groups = []
        super()._reset()

    def set_groups(self, groups: list[core.DupGroup], base: Path) -> None:
        if not groups:
            return self.clear("Nothing to clean. Nice.")
        self._reset()
        cap = MAX_ROWS["Duplicates"]
        if len(groups) > cap:
            ctk.CTkLabel(self.list, text=f"Showing the {cap} groups that free the most space.",
                         font=(FONT, 12)).pack(anchor="w", padx=12, pady=4)

        def make(g: core.DupGroup) -> None:
            box = ctk.CTkFrame(self.list, corner_radius=10)
            box.pack(fill="x", padx=6, pady=6)
            inc, keeper = ctk.BooleanVar(value=True), ctk.IntVar(value=0)
            self.groups.append((inc, keeper, g))
            ctk.CTkCheckBox(box, text=f"{g.files[0].name} · {len(g.files)} copies · "
                                      f"{core.human_size(g.size)} each", variable=inc, font=(FONT, 13, "bold"),
                            command=self.update_selection).pack(anchor="w", padx=10, pady=(8, 2))
            for i, f in enumerate(g.files):
                try:
                    shown = str(f.relative_to(base))
                except ValueError:
                    shown = str(f)
                ctk.CTkRadioButton(box, text=("Keep   " if i == 0 else "") + shown, value=i, variable=keeper,
                                   font=(FONT, 12), command=self.update_selection).pack(anchor="w", padx=28, pady=2)

        self._build_chunked(groups[:cap], make, chunk=4)

    def select_all(self, on: bool) -> None:
        for inc, _, _ in self.groups:
            inc.set(on)
        self.update_selection()

    def selected(self) -> list[Path]:
        return [f for inc, keep, g in self.groups if inc.get()
                for i, f in enumerate(g.files) if i != keep.get()]


class App(ctk.CTk):
    def __init__(self) -> None:
        super().__init__()
        ctk.set_appearance_mode("system")  # follows the Windows theme
        self.title("Declutter")
        self.geometry("960x640")
        self.minsize(800, 540)

        self.folder: Path | None = None
        self.moves: list[core.Move] = []
        self._events: queue.Queue = queue.Queue()
        self._cancel = threading.Event()
        self._busy = False
        self.sizes: dict[Path, int] = {}  # filled by the scan, so ticking boxes never touches the disk
        self.cat_sizes: dict[str, int] = {}

        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(0, weight=1)
        self._build_sidebar()
        self.pages: dict[str, ctk.CTkFrame] = {"Home": self._build_home()}
        self.sort = Tab(self, "Sort by type", "Move files", self.move, size_of=self._category_size,
                        subtitle=HELP["Sort"])
        self._build_sort_header()
        self.dups = DupTab(self, self.remove_dups, size_of=self._size)
        self.large = Tab(self, "Large & old files", "Send to Recycle Bin",
                         lambda: self.recycle(self.large.selected()), size_of=self._size,
                         subtitle=HELP["Large & Old"])
        self._build_large_header()
        self.empty = Tab(self, "Empty folders", "Send to Recycle Bin",
                         lambda: self.recycle_empty(self.empty.selected()),
                         subtitle=HELP["Empty Folders"])
        self.tabs = (self.sort, self.dups, self.large, self.empty)
        self.pages.update({"Sort": self.sort, "Duplicates": self.dups,
                           "Large & Old": self.large, "Empty Folders": self.empty})
        self._build_statusbar()
        self._refresh_undo()
        self.show("Home")
        self.report_callback_exception = self._on_ui_error
        self.after(100, self._poll)

    def _on_ui_error(self, exc_type, exc, tb) -> None:
        """Last line of defence: a bug in a callback shows a dialog instead of freezing the app."""
        messagebox.showerror("Something went wrong", f"{exc}\n\nThe app is still running.")

    # ---------- layout ----------

    def _build_sidebar(self) -> None:
        bar = ctk.CTkFrame(self, width=190, corner_radius=0)
        bar.grid(row=0, column=0, rowspan=2, sticky="ns")
        ctk.CTkLabel(bar, text="Declutter", font=(FONT, 20, "bold")).pack(padx=20, pady=(24, 16))
        self.nav = {}
        for name in NAMES:
            b = ctk.CTkButton(bar, text=name, anchor="w", font=(FONT, 14), fg_color="transparent",
                              text_color=("gray10", "gray90"), hover_color=("gray80", "gray25"),
                              corner_radius=8, command=lambda n=name: self.show(n))
            b.pack(fill="x", padx=12, pady=3)
            self.nav[name] = b

    def show(self, name: str) -> None:
        for n, page in self.pages.items():
            page.grid_forget()
            self.nav[n].configure(fg_color=ACCENT if n == name else "transparent",
                                  text_color="white" if n == name else ("gray10", "gray90"))
        self.pages[name].grid(row=0, column=1, sticky="nsew", padx=28, pady=(24, 8))

    def _build_statusbar(self) -> None:
        """Always visible, so progress and Cancel are reachable from every tab."""
        bar = ctk.CTkFrame(self, corner_radius=0, height=44)
        bar.grid(row=1, column=1, sticky="ew")
        bar.grid_columnconfigure(0, weight=1)
        self.status = ctk.CTkLabel(bar, text="Choose a folder on Home to begin.", font=(FONT, 13), anchor="w")
        self.status.grid(row=0, column=0, sticky="ew", padx=16, pady=10)
        self.progress = ctk.CTkProgressBar(bar, mode="indeterminate", width=140)
        self.cancel_btn = ctk.CTkButton(bar, text="Cancel", width=80, fg_color="gray40",
                                        command=self._cancel.set)

    def _set_busy(self, busy: bool) -> None:
        self._busy = busy
        for t in self.tabs:
            t.locked = busy
            t.update_selection()
        self.scan_btn.configure(state="disabled" if busy or not self.folder else "normal")
        if busy:
            self.progress.grid(row=0, column=1, padx=8)
            self.cancel_btn.grid(row=0, column=2, padx=(0, 16))
            self.progress.start()
        else:
            self.progress.stop()
            self.progress.grid_forget()
            self.cancel_btn.grid_forget()

    def _build_home(self) -> ctk.CTkFrame:
        f = ctk.CTkFrame(self, fg_color="transparent")
        f.grid_columnconfigure((0, 1), weight=1, uniform="c")
        ctk.CTkLabel(f, text="Tidy up a folder", font=(FONT, 22, "bold")).grid(columnspan=2, sticky="w")
        ctk.CTkLabel(f, text="1. Pick a folder   2. Scan   3. Review each tab. Nothing changes until you confirm.",
                     font=(FONT, 13), text_color=("gray35", "gray65")).grid(columnspan=2, sticky="w", pady=(2, 0))
        row = ctk.CTkFrame(f, fg_color="transparent")
        row.grid(columnspan=2, sticky="w", pady=16)
        for label in ("Downloads", "Desktop", "Documents"):
            ctk.CTkButton(row, text=label, font=(FONT, 14), corner_radius=10, width=120,
                          command=lambda l=label: self.set_folder(known_folder(l))).pack(side="left", padx=(0, 10))
        ctk.CTkButton(row, text="Choose folder…", font=(FONT, 14), corner_radius=10, width=140,
                      command=self.choose_folder, **GHOST).pack(side="left")
        self.folder_label = ctk.CTkLabel(f, text="No folder chosen yet", font=(FONT, 14), anchor="w")
        self.folder_label.grid(columnspan=2, sticky="w")
        self.scan_btn = ctk.CTkButton(f, text="Scan", font=(FONT, 16, "bold"), height=44, width=160,
                                      corner_radius=12, state="disabled", command=self.scan)
        self.scan_btn.grid(columnspan=2, sticky="w", pady=20)
        self.cards: dict[str, ctk.CTkButton] = {}
        for name in NAMES[1:]:
            self.cards[name] = ctk.CTkButton(
                f, text="", font=(FONT, 16), height=80, corner_radius=14, fg_color=("gray85", "gray20"),
                text_color=("gray10", "gray90"), hover_color=("gray75", "gray30"),
                command=lambda n=name: self.show(n))  # gridded after the first scan
        return f

    def _build_sort_header(self) -> None:
        self.recursive = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(self.sort.head, text="Include subfolders", variable=self.recursive, font=(FONT, 13),
                        command=self.resort).pack(side="left")
        self.undo_btn = ctk.CTkButton(self.sort.head, text="Undo last sort", corner_radius=10,
                                      command=self.undo, **GHOST)
        self.undo_btn.pack(side="right")

    def _build_large_header(self) -> None:
        self.min_mb, self.older = ctk.StringVar(value="100"), ctk.StringVar(value="0")
        for label, var in (("Bigger than (MB)", self.min_mb), ("Older than (days)", self.older)):
            ctk.CTkLabel(self.large.head, text=label, font=(FONT, 13)).pack(side="left", padx=(0, 6))
            ctk.CTkEntry(self.large.head, textvariable=var, width=70).pack(side="left", padx=(0, 16))
        ctk.CTkLabel(self.large.head, text="0 turns a filter off", font=(FONT, 12)).pack(side="left")
        ctk.CTkButton(self.large.head, text="Apply", width=80, corner_radius=10,
                      command=self.refilter).pack(side="right")

    # ---------- folder + background work ----------

    def choose_folder(self) -> None:
        d = filedialog.askdirectory(title="Choose a folder to tidy")
        if d:
            self.set_folder(Path(d))

    def set_folder(self, p: Path) -> None:
        if self._busy:
            return self.status.configure(text="Please wait for the current job to finish (or press Cancel).")
        ok, reason = core.check_root(p)
        if not ok:
            messagebox.showwarning("Can't use this folder", reason)
            return
        # core returns paths under the resolved folder, so keep the same form to make relative paths work
        self.folder = p.resolve()
        self.folder_label.configure(text=f"Folder: {self.folder}")
        self.scan_btn.configure(state="normal")
        for c in self.cards.values():
            c.grid_forget()
        for name, t in zip(NAMES[1:], self.tabs):
            t.clear("Press Scan on Home to look through this folder.")
            self.nav[name].configure(text=name)
        self.status.configure(text="Folder chosen. Press Scan.")
        self.show("Home")

    def rel(self, p: Path) -> str:
        try:
            return str(p.relative_to(self.folder))
        except (ValueError, TypeError):
            return str(p)

    def _progress(self, text: str) -> None:
        self._events.put((None, text))

    def _run(self, work, done) -> None:
        """Run work() on a thread; call done(result) on the UI thread, or show the error."""
        if self._busy:
            return self.status.configure(text="Still working. Please wait a moment.")
        self._cancel.clear()
        self._set_busy(True)

        def target():
            try:
                self._events.put((done, work()))
            except Exception as e:  # shown in a dialog, never a stack trace
                self._events.put((done, e))

        threading.Thread(target=target, daemon=True).start()

    def _poll(self) -> None:
        try:
            while True:
                done, result = self._events.get_nowait()
                if done is None:
                    self.status.configure(text=result)
                    continue
                self._set_busy(False)
                self.status.configure(text="")
                try:
                    if isinstance(result, Exception):
                        messagebox.showerror("Something went wrong", str(result))
                    elif self._cancel.is_set():
                        self.status.configure(text="Cancelled. Nothing was changed by the scan.")
                    else:
                        done(result)
                except Exception as e:  # keep the poll loop alive whatever a callback does
                    messagebox.showerror("Something went wrong", str(e))
        except queue.Empty:
            pass
        self.after(100, self._poll)

    # ---------- scan ----------

    def _large_params(self) -> tuple[float, int] | None:
        try:
            mb, days = float(self.min_mb.get() or 0), int(float(self.older.get() or 0))
            if mb < 0 or days < 0:
                raise ValueError
            return mb, days
        except ValueError:
            messagebox.showwarning("Check the filters", "Size and age must be numbers, 0 or more.")
            return None

    def scan(self) -> None:
        params = self._large_params()
        if not self.folder or not params:
            return
        folder, rec, c, p = self.folder, self.recursive.get(), self._cancel.is_set, self._progress

        def work():
            p("Reading the folder…")
            entries = core.scan_entries(folder, cancel=c)  # one walk shared by all checks
            moves = core.plan_sort(folder, rec, cancel=c, entries=entries)
            dups = core.find_duplicates(folder, progress=p, cancel=c, entries=entries)
            large = core.find_large_old(folder, *params, cancel=c, entries=entries)
            p("Looking for empty folders…")
            return moves, dups, large, core.find_empty_dirs(folder, cancel=c)

        self.status.configure(text="Scanning…")
        self._run(work, self._scan_done)

    def _scan_done(self, r) -> None:
        moves, dups, large, empty = r
        self.sizes = {}
        self._fill_sort(moves)
        self._fill_dups(dups)
        self._fill_large(large)
        self._fill_empty(empty)
        extra = sum(len(g.files) - 1 for g in dups)
        reclaim = sum(g.size * (len(g.files) - 1) for g in dups)
        counts = {"Sort": len(moves), "Duplicates": extra, "Large & Old": len(large), "Empty Folders": len(empty)}
        texts = {
            "Sort": f"{plural(len(moves), 'file')} to sort",
            "Duplicates": f"{plural(extra, 'duplicate')}, {core.human_size(reclaim)} reclaimable",
            "Large & Old": f"{plural(len(large), 'large or old file')}",
            "Empty Folders": f"{plural(len(empty), 'empty folder')}",
        }
        for i, (name, card) in enumerate(self.cards.items()):
            card.configure(text=texts[name] + ("\nNothing to clean. Nice." if not counts[name] else "\nClick to review"))
            card.grid(row=3 + i // 2, column=i % 2, sticky="ew", padx=6, pady=6)
            self.nav[name].configure(text=f"{name}  ({counts[name]})" if counts[name] else name)
        total = sum(counts.values())
        self.status.configure(text="Scan finished. Review the cards below." if total
                              else "Scan finished. This folder is already tidy.")

    def _fill_sort(self, moves: list[core.Move]) -> None:
        self.moves = moves
        by_cat: dict[str, list[str]] = {}
        self.cat_sizes = {}
        for m in moves:
            by_cat.setdefault(m.category, []).append(m.src.name)
            self.cat_sizes[m.category] = self.cat_sizes.get(m.category, 0) + m.size
        rows = []
        for cat, names in sorted(by_cat.items()):
            eg = ", ".join(names[:3]) + (", …" if len(names) > 3 else "")
            rows.append((f"{cat}: {plural(len(names), 'file')}   (e.g. {eg})", cat))
        self.sort.set_rows(rows, True)

    def _category_size(self, cat: str) -> int:
        return self.cat_sizes.get(cat, 0)

    def _size(self, p: Path) -> int:
        return self.sizes[p] if p in self.sizes else file_size(p)

    def _sort_selected(self) -> list[core.Move]:
        on = set(self.sort.selected())
        return [m for m in self.moves if m.category in on]

    def _fill_dups(self, groups: list[core.DupGroup]) -> None:
        for g in groups:
            for f in g.files:
                self.sizes[f] = g.size
        self.dups.set_groups(groups, self.folder)

    def _fill_large(self, large: list[core.FileInfo]) -> None:
        cap = MAX_ROWS["Large & Old"]
        self.sizes.update({f.path: f.size for f in large})
        rows = [(f"{core.human_size(f.size)} · {f.age_days} days old · {self.rel(f.path)}", f.path)
                for f in large[:cap]]
        self.large.set_rows(rows, False, f"Showing the {cap} biggest." if len(large) > cap else "")

    def _fill_empty(self, empty: list[Path]) -> None:
        cap = MAX_ROWS["Empty Folders"]
        rows = [(self.rel(p), p) for p in empty[:cap]]
        self.empty.set_rows(rows, True, f"Showing {cap} of {len(empty)}." if len(empty) > cap else "")

    # ---------- sort actions ----------

    def resort(self) -> None:
        if not self.folder:
            return
        folder, rec, c = self.folder, self.recursive.get(), self._cancel.is_set
        self._run(lambda: core.plan_sort(folder, rec, cancel=c, entries=core.scan_entries(folder, c)),
                  self._fill_sort)

    def move(self) -> None:
        sel = self._sort_selected()
        if sel and messagebox.askyesno(
                "Move files?", f"Move {plural(len(sel), 'file')} into type folders inside\n{self.folder}?\n\n"
                "Nothing is deleted. You can undo this with \"Undo last sort\"."):
            folder = self.folder
            self._run(lambda: core.apply_moves(sel, folder), self._move_done)

    def _move_done(self, result: tuple[int, list[str]]) -> None:
        self._report("Done", f"Moved {plural(result[0], 'file')}.", result[1])
        self._refresh_undo()
        self.scan()

    def undo(self) -> None:
        if not core.can_undo():
            return messagebox.showinfo("Undo", "Nothing to undo.")
        if messagebox.askyesno("Undo last sort?", "Put the files from the last sort back where they were?"):
            self._run(core.undo_last, self._undo_done)

    def _undo_done(self, result: tuple[int, list[str]]) -> None:
        self._report("Undo", f"Restored {plural(result[0], 'file')}.", result[1])
        self._refresh_undo()
        self.scan()

    def _refresh_undo(self) -> None:
        self.undo_btn.configure(state="normal" if core.can_undo() else "disabled")

    # ---------- recycle-bin actions ----------

    def refilter(self) -> None:
        params = self._large_params()
        if not self.folder or not params:
            return
        folder, c = self.folder, self._cancel.is_set
        self._run(lambda: core.find_large_old(folder, *params, cancel=c), self._fill_large)

    def remove_dups(self) -> None:
        removals = [(g, [f for i, f in enumerate(g.files) if i != keep.get()])
                    for inc, keep, g in self.dups.groups if inc.get()]
        try:  # safety rule 6; the radio buttons already make this impossible
            core.check_dup_selection([g for _, _, g in self.dups.groups],
                                     {f for _, rem in removals for f in rem})
        except ValueError as e:
            return messagebox.showerror("Keep one copy", str(e))
        n = sum(len(rem) for _, rem in removals)
        if n and self._confirm_bin(n):
            folder = self.folder
            self._run(lambda: core.trash_duplicates(removals, folder), self._recycled)

    @staticmethod
    def _confirm_bin(n: int) -> bool:
        return messagebox.askyesno(
            "Send to Recycle Bin?", f"Send {plural(n, 'item')} to the Recycle Bin?\n\n"
            "They are not deleted. You can restore them from the Recycle Bin.")

    def recycle(self, paths: list[Path]) -> None:
        if paths and self._confirm_bin(len(paths)):
            folder = self.folder
            self._run(lambda: core.to_recycle_bin(paths, folder), self._recycled)

    def recycle_empty(self, paths: list[Path]) -> None:
        if paths and self._confirm_bin(len(paths)):
            folder = self.folder
            self._run(lambda: core.trash_empty_dirs(paths, folder), self._recycled)

    def _recycled(self, result: tuple[int, list[str]]) -> None:
        self._report("Done", f"Sent {plural(result[0], 'item')} to the Recycle Bin. Restore them from the Bin if needed.", result[1])
        self.scan()

    @staticmethod
    def _report(title: str, msg: str, errors: list[str]) -> None:
        if errors:
            msg += f"\n\n{len(errors)} had problems:\n" + "\n".join(errors[:8])
        messagebox.showinfo(title, msg)


def main() -> None:
    App().mainloop()
