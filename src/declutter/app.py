"""Declutter GUI (CustomTkinter). All file logic lives in core.py."""
from __future__ import annotations

import queue
import subprocess
import threading
from collections import Counter
from pathlib import Path
from tkinter import filedialog, messagebox
from typing import Callable

import customtkinter as ctk

from . import core, theme as T
from .theme import FONT

GHOST = dict(fg_color="transparent", border_width=1, border_color=T.BORDER_CONTROL,
             text_color=T.TEXT_NAV, hover_color=T.SURFACE_ALT)
PRIMARY = dict(fg_color=T.ACCENT, text_color=T.ON_ACCENT, hover_color=T.ACCENT_HOVER)
PLAIN = dict(fg_color="transparent", text_color=T.ACCENT_TEXT, hover_color=T.SURFACE_ALT)  # HIG: low-emphasis actions are text-only
# CustomTkinter widgets are slow to build, so long lists are capped to keep the window responsive.
MAX_ROWS = {"Duplicates": 100, "Large & Old": 200, "Empty Folders": 300}
NAMES = ("Home", "Sort", "Duplicates", "Large & Old", "Empty Folders")
HELP = {
    "Sort": "Moves loose files into folders by type (Images, Documents, …). Nothing is deleted.",
    "Duplicates": "Files with identical content. The oldest copy is kept; pick a different one if you prefer. "
                  "Double-click a file to see it in Explorer.",
    "Large & Old": "Big or long-untouched files. Nothing is selected for you, so tick what you want gone. "
                   "Double-click a file to see it in Explorer.",
    "Empty Folders": "Folders with no files inside, however deep. Double-click one to see it in Explorer.",
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


def reveal(p: Path) -> None:
    """Show the item selected in Explorer. Never opens it: that could run an installer."""
    if p.exists():
        subprocess.Popen(f'explorer /select,"{p}"')


def confirm(master, title: str, message: str, ok_text: str) -> bool:
    """Modal confirm dialog in the app's theme. Cancel is the default; Esc cancels."""
    win = ctk.CTkToplevel(master, fg_color=T.BG)
    win.title(title)
    win.resizable(False, False)
    win.transient(master)
    result = {"ok": False}

    def close(ok: bool) -> None:
        result["ok"] = ok
        win.destroy()

    card = ctk.CTkFrame(win, corner_radius=T.R_DIALOG, fg_color=T.SURFACE)
    card.pack(padx=T.GAP, pady=T.GAP)
    ctk.CTkLabel(card, text=title, font=(FONT, 20, "bold"), text_color=T.TEXT).pack(
        anchor="w", padx=T.GAP + 4, pady=(T.GAP + 4, 6))
    ctk.CTkLabel(card, text=message, font=(FONT, 15), text_color=T.TEXT_MUTED, justify="left",
                 wraplength=420).pack(anchor="w", padx=T.GAP + 4)
    row = ctk.CTkFrame(card, fg_color="transparent")
    row.pack(fill="x", padx=T.GAP + 4, pady=T.GAP + 4)
    ctk.CTkButton(row, text=ok_text, height=38, corner_radius=T.R_BUTTON, font=(FONT, 14, "bold"),
                  command=lambda: close(True), **PRIMARY).pack(side="right")
    cancel = ctk.CTkButton(row, text="Cancel", width=90, height=38, corner_radius=T.R_BUTTON, font=(FONT, 14),
                           command=lambda: close(False), **GHOST)
    cancel.pack(side="right", padx=(0, 10))
    win.bind("<Escape>", lambda _e: close(False))
    win.protocol("WM_DELETE_WINDOW", lambda: close(False))
    win.update_idletasks()
    x = master.winfo_rootx() + (master.winfo_width() - win.winfo_reqwidth()) // 2
    y = master.winfo_rooty() + (master.winfo_height() - win.winfo_reqheight()) // 2
    win.geometry(f"+{max(x, 0)}+{max(y, 0)}")
    win.after(50, lambda: (win.grab_set(), cancel.focus_set()))
    master.wait_window(win)
    return result["ok"]


class Tab(ctk.CTkFrame):
    """Title, optional header row, scrollable checkbox list, and a footer with one action.

    With a filter typed, only matching rows are shown, and only shown rows count as selected,
    so an action never touches something the user can't see.
    """

    def __init__(self, master, title: str, action: str, on_action: Callable[[], None],
                 size_of: Callable[[object], int] | None = None, subtitle: str = "", search: bool = True):
        super().__init__(master, fg_color="transparent")
        self.size_of = size_of
        self.locked = False  # True while a background job runs
        self.items: list[tuple[ctk.BooleanVar, object]] = []
        self.rows: list[tuple[ctk.CTkBaseClass, str, dict]] = []  # (widget, lowercase text, pack options)
        self._q = ""
        self._filter_job = None
        self.grid_columnconfigure(0, weight=1)
        self.grid_rowconfigure(2, weight=1)
        top = ctk.CTkFrame(self, fg_color="transparent")
        top.grid(row=0, sticky="ew")
        top.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(top, text=title, font=(FONT, T.TITLE, "bold"), text_color=T.TEXT).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(top, text=subtitle, font=(FONT, 14), text_color=T.TEXT_MUTED,
                     wraplength=640, justify="left").grid(row=1, column=0, columnspan=2, sticky="w", pady=(2, 0))
        self.search = None
        if search:
            self.search = ctk.CTkEntry(top, placeholder_text="Filter by name", width=220, height=34, font=(FONT, 14),
                                       corner_radius=T.R_BUTTON, fg_color=T.SURFACE, text_color=T.TEXT,
                                       border_color=T.BORDER_CONTROL, placeholder_text_color=T.TEXT_MUTED)
            self.search.grid(row=0, column=1, sticky="e")
            self.search.bind("<KeyRelease>", lambda _e: self._filter_soon())
            self.search.bind("<Escape>", lambda _e: (self.search.delete(0, "end"), self._filter_soon()))
        self.head = ctk.CTkFrame(self, fg_color="transparent")
        self.head.grid(row=1, sticky="ew", pady=T.CARD_GAP // 2)
        self.list = ctk.CTkScrollableFrame(self, corner_radius=T.R_CARD, fg_color=T.SURFACE)
        self.list.grid(row=2, sticky="nsew")
        foot = ctk.CTkFrame(self, fg_color="transparent")
        foot.grid(row=3, sticky="ew", pady=(T.CARD_GAP, 0))
        self.sel_label = ctk.CTkLabel(foot, text="Selected: 0 items", font=(FONT, 14), text_color=T.TEXT_MUTED)
        self.sel_label.pack(side="left")
        self.action_btn = ctk.CTkButton(foot, text=action, corner_radius=T.R_BUTTON, command=on_action,
                                        state="disabled", font=(FONT, 14, "bold"), height=38, **PRIMARY)
        self.action_btn.pack(side="right")
        ctk.CTkButton(foot, text="Select none", width=90, height=38, corner_radius=T.R_BUTTON, **PLAIN,
                      font=(FONT, 14), command=lambda: self.select_all(False)).pack(side="right", padx=(0, 10))
        ctk.CTkButton(foot, text="Select all", width=90, height=38, corner_radius=T.R_BUTTON, **PLAIN,
                      font=(FONT, 14), command=lambda: self.select_all(True)).pack(side="right", padx=(0, 6))
        self.clear("Scan a folder on Home first.")

    _gen = 0  # bumped on every rebuild so an older, still-running chunked build stops itself

    def clear(self, message: str) -> None:
        self._reset()
        ctk.CTkLabel(self.list, text=message, font=(FONT, 15), text_color=T.TEXT_MUTED).pack(pady=40)
        self.update_selection()

    def _reset(self) -> None:
        self._gen += 1
        for w in self.list.winfo_children():
            w.destroy()
        self.items = []
        self.rows = []

    def _add_row(self, widget, text: str, **pack) -> None:
        self.rows.append((widget, text.lower(), pack))
        if self._q in text.lower():
            widget.pack(**pack)

    def _shown(self) -> list[bool]:
        return [self._q in t for _, t, _ in self.rows]

    def _filter_soon(self) -> None:
        """Debounced, so typing fast doesn't repack hundreds of rows per keystroke."""
        if self._filter_job:
            self.after_cancel(self._filter_job)
        self._filter_job = self.after(150, self._apply_filter)

    def _apply_filter(self) -> None:
        self._filter_job = None
        self._q = self.search.get().strip().lower() if self.search else ""
        for w, _, _ in self.rows:
            w.pack_forget()
        for w, t, pack in self.rows:
            if self._q in t:
                w.pack(**pack)
        self.update_selection()

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
            ctk.CTkLabel(self.list, text=note, font=(FONT, 13), text_color=T.TEXT_MUTED,
                         anchor="w").pack(fill="x", padx=12, pady=4)

        def make(row) -> None:
            text, obj = row
            v = ctk.BooleanVar(value=checked)
            self.items.append((v, obj))
            cb = ctk.CTkCheckBox(self.list, text=text, variable=v, font=(FONT, 15), text_color=T.TEXT,
                                 fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER, border_color=T.BORDER_CONTROL,
                                 checkmark_color=T.ON_ACCENT, command=self.update_selection)
            if isinstance(obj, Path):
                cb.bind("<Double-Button-1>", lambda _e, p=obj: reveal(p))
            self._add_row(cb, text, anchor="w", padx=12, pady=6)

        self._build_chunked(rows, make)

    def select_all(self, on: bool) -> None:
        for (v, _), shown in zip(self.items, self._shown()):
            if shown:
                v.set(on)
        self.update_selection()

    def selected(self) -> list:
        return [o for (v, o), shown in zip(self.items, self._shown()) if shown and v.get()]

    def update_selection(self) -> None:
        sel = self.selected()
        text = f"Selected: {plural(len(sel), 'item')}"
        if self.size_of and sel:
            text += f", {core.human_size(sum(self.size_of(o) for o in sel))}"
        if self._q:
            text += f"  ·  {sum(self._shown())} of {len(self.rows)} shown"
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
                         font=(FONT, 13), text_color=T.TEXT_MUTED).pack(anchor="w", padx=12, pady=4)

        def make(g: core.DupGroup) -> None:
            box = ctk.CTkFrame(self.list, corner_radius=T.R_BUTTON, fg_color=T.SURFACE_ALT)
            inc, keeper = ctk.BooleanVar(value=True), ctk.IntVar(value=0)
            self.groups.append((inc, keeper, g))
            ctk.CTkCheckBox(box, text=f"{g.files[0].name} · {len(g.files)} copies · "
                                      f"{core.human_size(g.size)} each", variable=inc, font=(FONT, 15, "bold"),
                            text_color=T.TEXT, fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER,
                            border_color=T.BORDER_CONTROL, checkmark_color=T.ON_ACCENT,
                            command=self.update_selection).pack(anchor="w", padx=10, pady=(8, 2))
            for i, f in enumerate(g.files):
                try:
                    shown = str(f.relative_to(base))
                except ValueError:
                    shown = str(f)
                rb = ctk.CTkRadioButton(box, text=("Keep   " if i == 0 else "") + shown, value=i, variable=keeper,
                                        font=(FONT, 13), text_color=T.TEXT_MUTED, fg_color=T.ACCENT,
                                        hover_color=T.ACCENT_HOVER, border_color=T.BORDER_CONTROL,
                                        command=self.update_selection)
                rb.bind("<Double-Button-1>", lambda _e, p=f: reveal(p))
                rb.pack(anchor="w", padx=28, pady=3)
            self._add_row(box, " ".join(f.name for f in g.files), fill="x", padx=6, pady=6)

        self._build_chunked(groups[:cap], make, chunk=4)

    def visible_groups(self) -> list[tuple[ctk.BooleanVar, ctk.IntVar, core.DupGroup]]:
        return [grp for grp, shown in zip(self.groups, self._shown()) if shown]

    def select_all(self, on: bool) -> None:
        for inc, _, _ in self.visible_groups():
            inc.set(on)
        self.update_selection()

    def selected(self) -> list[Path]:
        return [f for inc, keep, g in self.visible_groups() if inc.get()
                for i, f in enumerate(g.files) if i != keep.get()]


class App(ctk.CTk):
    def __init__(self) -> None:
        super().__init__()
        ctk.set_appearance_mode(T.load_mode().lower())  # "system" follows the Windows theme
        self.title("Declutter")
        self.configure(fg_color=T.BG)
        self.geometry("1100x720")
        self.minsize(860, 560)

        self.folder: Path | None = None
        self.moves: list[core.Move] = []
        self._events: queue.Queue = queue.Queue()
        self._cancel = threading.Event()
        self._busy = False
        self.sizes: dict[Path, int] = {}  # filled by the scan, so ticking boxes never touches the disk
        self.cat_sizes: dict[str, int] = {}
        # The last scan's file list, so filter changes don't walk the folder again. Actions still
        # re-check the disk, and every action rescans afterwards.
        self.entries: list[core.Entry] | None = None
        self.settings = T.load_settings()

        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(0, weight=1)
        self._build_sidebar()
        self.pages: dict[str, ctk.CTkFrame] = {"Home": self._build_home()}
        self.sort = Tab(self, "Sort by type", "Move files", self.move, size_of=self._category_size,
                        subtitle=HELP["Sort"], search=False)
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
                           "Large & Old": self.large, "Empty Folders": self.empty,
                           "Settings": self._build_settings()})
        self._build_statusbar()
        self._refresh_undo()
        self.show("Home")
        self.report_callback_exception = self._on_ui_error
        self.after(100, self._poll)
        last = self.settings.get("last_folder")
        if self.settings.get("reopen") and last and core.check_root(last)[0]:
            self.set_folder(Path(last))

    def _on_ui_error(self, exc_type, exc, tb) -> None:
        """Last line of defence: a bug in a callback shows a dialog instead of freezing the app."""
        messagebox.showerror("Something went wrong", f"{exc}\n\nThe app is still running.")

    # ---------- layout ----------

    def _build_sidebar(self) -> None:
        bar = ctk.CTkFrame(self, width=T.SIDEBAR_W, corner_radius=0, fg_color=T.SURFACE,
                           border_width=1, border_color=T.BORDER)
        bar.grid(row=0, column=0, rowspan=2, sticky="ns")
        bar.grid_propagate(False)
        ctk.CTkLabel(bar, text="Declutter", font=(FONT, 20, "bold"), text_color=T.TEXT).pack(
            anchor="w", padx=20, pady=(T.PAD_Y, T.GAP))
        self.nav = {}
        for name in NAMES + ("Settings",):
            b = ctk.CTkButton(bar, text=name, anchor="w", font=(FONT, 15), height=40, fg_color="transparent",
                              text_color=T.TEXT_NAV, hover_color=T.SURFACE_ALT,
                              corner_radius=T.R_NAV, command=lambda n=name: self.show(n))
            if name == "Settings":  # macOS-style: settings sit apart, at the bottom
                b.pack(side="bottom", fill="x", padx=12, pady=(0, 16))
            else:
                b.pack(fill="x", padx=12, pady=2)
            self.nav[name] = b

    def _build_settings(self) -> ctk.CTkFrame:
        """Grouped rows in the style of macOS System Settings. Every change saves immediately."""
        f = ctk.CTkFrame(self, fg_color="transparent")
        ctk.CTkLabel(f, text="Settings", font=(FONT, T.TITLE, "bold"), text_color=T.TEXT).pack(anchor="w")

        def group(title: str) -> ctk.CTkFrame:
            ctk.CTkLabel(f, text=title.upper(), font=(FONT, 12), text_color=T.TEXT_MUTED).pack(
                anchor="w", padx=12, pady=(T.GAP, 6))
            card = ctk.CTkFrame(f, corner_radius=T.R_CARD, fg_color=T.SURFACE)
            card.pack(fill="x")
            return card

        def row(card, label: str, note: str = "") -> ctk.CTkFrame:
            r = ctk.CTkFrame(card, fg_color="transparent")
            r.pack(fill="x", padx=16, pady=10)
            text = ctk.CTkFrame(r, fg_color="transparent")
            text.pack(side="left")
            ctk.CTkLabel(text, text=label, font=(FONT, 15), text_color=T.TEXT).pack(anchor="w")
            if note:
                ctk.CTkLabel(text, text=note, font=(FONT, 13), text_color=T.TEXT_MUTED).pack(anchor="w")
            return r

        card = group("Appearance")
        self.theme_pick = ctk.CTkSegmentedButton(
            row(card, "Theme", "System follows your Windows setting."), values=list(T.MODES),
            command=self._set_theme, font=(FONT, 13), fg_color=T.SURFACE_ALT, selected_color=T.ACCENT,
            selected_hover_color=T.ACCENT_HOVER, unselected_color=T.SURFACE_ALT,
            unselected_hover_color=T.BORDER, text_color=T.TEXT_NAV)
        self.theme_pick.set(T.load_mode())
        self.theme_pick.pack(side="right")

        card = group("Start-up")
        self.reopen = ctk.BooleanVar(value=bool(self.settings.get("reopen")))
        ctk.CTkSwitch(row(card, "Reopen the last folder", "Picks it again at start. It is not scanned until you press Scan."),
                      text="", variable=self.reopen, progress_color=T.ACCENT, fg_color=T.BORDER_CONTROL,
                      command=lambda: T.save_settings(reopen=self.reopen.get())).pack(side="right")

        card = group("Large & old files")
        for label, var in (("Bigger than (MB)", self.min_mb), ("Older than (days)", self.older)):
            e = ctk.CTkEntry(row(card, label, "0 turns this filter off. Remembered between sessions."),
                             textvariable=var, width=80, fg_color=T.SURFACE, text_color=T.TEXT,
                             border_color=T.BORDER_CONTROL, corner_radius=T.R_BUTTON)
            e.pack(side="right")
            e.bind("<FocusOut>", lambda _e: self._save_filters())
            e.bind("<Return>", lambda _e: self._save_filters())
        return f

    def _set_theme(self, mode: str) -> None:
        ctk.set_appearance_mode(mode.lower())
        T.save_mode(mode)

    def show(self, name: str) -> None:
        for n, page in self.pages.items():
            page.grid_forget()
            on = n == name
            self.nav[n].configure(fg_color=T.ACCENT_SOFT if on else "transparent",
                                  text_color=T.ACCENT_TEXT if on else T.TEXT_NAV)
        self.pages[name].grid(row=0, column=1, sticky="nsew", padx=T.PAD_X, pady=(T.PAD_Y, T.GAP))

    def _build_statusbar(self) -> None:
        """Always visible, so progress and Cancel are reachable from every tab."""
        bar = ctk.CTkFrame(self, corner_radius=0, height=48, fg_color=T.SURFACE,
                           border_width=1, border_color=T.BORDER)
        bar.grid(row=1, column=1, sticky="ew")
        bar.grid_columnconfigure(0, weight=1)
        self.status = ctk.CTkLabel(bar, text="Choose a folder on Home to begin.", font=(FONT, 14),
                                   text_color=T.TEXT_MUTED, anchor="w")
        self.status.grid(row=0, column=0, sticky="ew", padx=T.PAD_X, pady=12)
        self.progress = ctk.CTkProgressBar(bar, mode="indeterminate", width=140, progress_color=T.ACCENT,
                                           fg_color=T.SURFACE_ALT)
        self.cancel_btn = ctk.CTkButton(bar, text="Cancel", width=80, corner_radius=T.R_BUTTON,
                                        font=(FONT, 14), command=self._cancel.set, **GHOST)

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
        ctk.CTkLabel(f, text="Tidy up a folder", font=(FONT, T.TITLE, "bold"), text_color=T.TEXT).grid(
            columnspan=2, sticky="w")
        ctk.CTkLabel(f, text="1. Pick a folder   2. Scan   3. Review each tab. Nothing changes until you confirm.",
                     font=(FONT, 14), text_color=T.TEXT_MUTED).grid(columnspan=2, sticky="w", pady=(2, 0))
        row = ctk.CTkFrame(f, fg_color="transparent")
        row.grid(columnspan=2, sticky="w", pady=T.GAP)
        for label in ("Downloads", "Desktop", "Documents"):
            ctk.CTkButton(row, text=label, font=(FONT, 15), corner_radius=T.R_BUTTON, width=120, height=40,
                          command=lambda l=label: self.set_folder(known_folder(l)), **GHOST
                          ).pack(side="left", padx=(0, 10))
        ctk.CTkButton(row, text="Choose folder…", font=(FONT, 15), corner_radius=T.R_BUTTON, width=140, height=40,
                      command=self.choose_folder, **GHOST).pack(side="left")
        self.folder_label = ctk.CTkLabel(f, text="No folder chosen yet", font=(FONT, 15),
                                         text_color=T.TEXT_MUTED, anchor="w")
        self.folder_label.grid(columnspan=2, sticky="w")
        self.scan_btn = ctk.CTkButton(f, text="Scan", font=(FONT, 16, "bold"), height=44, width=160,
                                      corner_radius=T.R_BUTTON, state="disabled", command=self.scan, **PRIMARY)
        self.scan_btn.grid(columnspan=2, sticky="w", pady=T.GAP)
        self.cards: dict[str, ctk.CTkButton] = {}
        for name in NAMES[1:]:
            self.cards[name] = ctk.CTkButton(
                f, text="", font=(FONT, 16), height=88, corner_radius=T.R_CARD, fg_color=T.SURFACE,
                text_color=T.TEXT, hover_color=T.SURFACE_ALT,
                command=lambda n=name: self.show(n))  # gridded after the first scan
        return f

    def _build_sort_header(self) -> None:
        self.recursive = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(self.sort.head, text="Include subfolders", variable=self.recursive, font=(FONT, 15),
                        text_color=T.TEXT, fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER,
                        border_color=T.BORDER_CONTROL, checkmark_color=T.ON_ACCENT,
                        command=self.resort).pack(side="left")
        self.undo_btn = ctk.CTkButton(self.sort.head, text="Undo last sort", corner_radius=T.R_BUTTON, height=38,
                                      font=(FONT, 14), command=self.undo, **GHOST)
        self.undo_btn.pack(side="right")

    def _build_large_header(self) -> None:
        self.min_mb = ctk.StringVar(value=str(self.settings.get("min_mb", "100")))
        self.older = ctk.StringVar(value=str(self.settings.get("older_days", "0")))
        for label, var in (("Bigger than (MB)", self.min_mb), ("Older than (days)", self.older)):
            ctk.CTkLabel(self.large.head, text=label, font=(FONT, 14), text_color=T.TEXT).pack(
                side="left", padx=(0, 6))
            e = ctk.CTkEntry(self.large.head, textvariable=var, width=70, fg_color=T.SURFACE, text_color=T.TEXT,
                             border_color=T.BORDER_CONTROL, corner_radius=T.R_BUTTON)
            e.pack(side="left", padx=(0, 16))
            e.bind("<Return>", lambda _e: self.refilter())
        ctk.CTkLabel(self.large.head, text="0 turns a filter off", font=(FONT, 13),
                     text_color=T.TEXT_MUTED).pack(side="left")
        ctk.CTkButton(self.large.head, text="Apply", width=80, height=38, corner_radius=T.R_BUTTON,
                      font=(FONT, 14), command=self.refilter, **GHOST).pack(side="right")

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
        self.entries = None
        T.save_settings(last_folder=str(self.folder))
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
        except ValueError:
            messagebox.showwarning("Check the filters", "Size and age must be numbers, 0 or more.")
            return None
        T.save_settings(min_mb=self.min_mb.get(), older_days=self.older.get())
        return mb, days

    def _save_filters(self) -> None:
        """Settings page: remember valid values quietly; invalid ones are reported when used."""
        try:
            if float(self.min_mb.get() or 0) >= 0 and float(self.older.get() or 0) >= 0:
                T.save_settings(min_mb=self.min_mb.get(), older_days=self.older.get())
        except ValueError:
            pass

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
            return entries, moves, dups, large, core.find_empty_dirs(folder, cancel=c)

        self.status.configure(text="Scanning…")
        self._run(work, self._scan_done)

    def _scan_done(self, r) -> None:
        self.entries, moves, dups, large, empty = r
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
        folder, rec, c, entries = self.folder, self.recursive.get(), self._cancel.is_set, self.entries
        self._run(lambda: core.plan_sort(folder, rec, cancel=c, entries=entries or core.scan_entries(folder, c)),
                  self._fill_sort)

    def move(self) -> None:
        sel = self._sort_selected()
        if sel and confirm(
                self, "Move files?", f"Move {plural(len(sel), 'file')} into type folders inside\n{self.folder}?\n\n"
                "Nothing is deleted. You can undo this with \"Undo last sort\".", "Move files"):
            folder = self.folder
            self._run(lambda: core.apply_moves(sel, folder), self._move_done)

    def _move_done(self, result: tuple[int, list[str]]) -> None:
        self._report("Done", f"Moved {plural(result[0], 'file')}.", result[1])
        self._refresh_undo()
        self.scan()

    def undo(self) -> None:
        if not core.can_undo():
            return messagebox.showinfo("Undo", "Nothing to undo.")
        if confirm(self, "Undo last sort?", "Put the files from the last sort back where they were?", "Undo"):
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
        folder, c, entries = self.folder, self._cancel.is_set, self.entries
        self._run(lambda: core.find_large_old(folder, *params, cancel=c, entries=entries), self._fill_large)

    def remove_dups(self) -> None:
        removals = [(g, [f for i, f in enumerate(g.files) if i != keep.get()])
                    for inc, keep, g in self.dups.visible_groups() if inc.get()]
        try:  # safety rule 6; the radio buttons already make this impossible
            core.check_dup_selection([g for _, _, g in self.dups.groups],
                                     {f for _, rem in removals for f in rem})
        except ValueError as e:
            return messagebox.showerror("Keep one copy", str(e))
        n = sum(len(rem) for _, rem in removals)
        if n and self._confirm_bin(n):
            folder = self.folder
            self._run(lambda: core.trash_duplicates(removals, folder), self._recycled)

    def _confirm_bin(self, n: int) -> bool:
        return confirm(
            self, "Send to Recycle Bin?", f"Send {plural(n, 'item')} to the Recycle Bin?\n\n"
            "They are not deleted. You can restore them from the Recycle Bin.", f"Send {plural(n, 'item')}")

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
