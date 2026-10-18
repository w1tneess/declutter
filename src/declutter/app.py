"""Declutter GUI (CustomTkinter). All file logic lives in core.py."""
from __future__ import annotations

import gc
import queue
import subprocess
import sys
import threading
import time
import webbrowser
from pathlib import Path
from tkinter import filedialog, messagebox
from typing import Callable

import customtkinter as ctk

from . import __version__, core, theme as T
from .theme import FONT

SITE = "https://w1tneess.github.io/declutter/"
REPO = "https://github.com/w1tneess/declutter"
SUPPORT = "https://support.advaitachandra.in"

GHOST = dict(fg_color="transparent", border_width=1, border_color=T.BORDER_CONTROL,
             text_color=T.TEXT_NAV, hover_color=T.SURFACE_ALT)
PRIMARY = dict(fg_color=T.ACCENT, text_color=T.ON_ACCENT, hover_color=T.ACCENT_HOVER, text_color_disabled=T.TEXT_MUTED)
PLAIN = dict(fg_color="transparent", text_color=T.ACCENT_TEXT, hover_color=T.SURFACE_ALT)  # HIG: low-emphasis actions are text-only
# CustomTkinter widgets are slow to build, so long lists are capped to keep the window responsive.
MAX_ROWS = {"Sort": 300, "Duplicates": 100, "Large & Old": 200, "Empty Folders": 300}
NAMES = ("Home", "Sort", "Duplicates", "Large & Old", "Empty Folders")
NAV_LABEL = {"Sort": "Sort by type", "Large & Old": "Large & old", "Empty Folders": "Empty folders"}
HELP = {
    "Sort": "Moves loose files into folders by type (Images, Documents, …). Nothing is deleted.",
    "Duplicates": "Files with identical content. The oldest copy is kept; pick a different one if you prefer. "
                  "Double-click a file to see it in Explorer.",
    "Large & Old": "Big or long-untouched files. Nothing is selected for you, so tick what you want gone. "
                   "Double-click a file to see it in Explorer.",
    "Empty Folders": "Folders with no files inside, however deep. Double-click one to see it in Explorer.",
}
SORT_HINT = "Only loose files directly in this folder are sorted. Shortcuts and config files stay."


def file_size(p: Path) -> int:
    try:
        return p.stat().st_size
    except OSError:
        return 0


def plural(n: int, word: str) -> str:
    return f"{n} {word}" if n == 1 else f"{n} {word}s"


def date_text(ts: float) -> str:
    return time.strftime("%d %b %Y", time.localtime(ts))


def pill(master, text: str) -> ctk.CTkLabel:
    """A small tag saying why a row was flagged."""
    return ctk.CTkLabel(master, text=text, font=(FONT, 12), text_color=T.ACCENT_TEXT, fg_color=T.ACCENT_SOFT,
                        corner_radius=T.R_PILL, height=24)


def set_primary(btn, on: bool) -> None:
    """A disabled filled button must not stay bright blue, or it reads as clickable."""
    btn.configure(state="normal" if on else "disabled", fg_color=T.ACCENT if on else T.SURFACE_ALT)


def keyboard_focus() -> None:
    """CustomTkinter buttons, checkboxes, radios and switches can't be reached with Tab and never show
    focus. Make them focusable (Space or Enter activates) with a ring visible in both themes. Runs once."""
    if getattr(ctk.CTkButton, "_keyboard", False):
        return

    def border(w, on: bool) -> None:
        if on:
            w._unfocused = (w.cget("border_width"), w.cget("border_color"))
            w.configure(border_width=2, border_color=T.FOCUS)
        elif hasattr(w, "_unfocused"):
            width, color = w._unfocused
            w.configure(border_width=width, border_color=color)

    def underline(w, on: bool) -> None:
        f = w.cget("font")
        if isinstance(f, tuple):
            plain = tuple(x for x in f if x != "underline")
            w.configure(font=plain + ("underline",) if on else plain)

    def into_view(w) -> None:
        """Tabbing through a long list must scroll the focused row into sight."""
        sf = w.master
        while sf is not None and not isinstance(sf, ctk.CTkScrollableFrame):
            sf = sf.master
        if sf is None:
            return
        canvas = sf._parent_canvas  # CTkScrollableFrame keeps its scrolling canvas here
        total = sf.winfo_height()
        y, h, view = w.winfo_rooty() - sf.winfo_rooty(), w.winfo_height(), canvas.winfo_height()
        top = canvas.yview()[0] * total
        if total > 0 and (y < top or y + h > top + view):
            canvas.yview_moveto(max(0.0, y - view / 3) / total)

    def wrap(cls, activate: str, ring) -> None:
        orig = cls.__init__

        def __init__(self, *args, **kwargs):
            orig(self, *args, **kwargs)
            t = getattr(self, "_text_label", None) or self._canvas  # what CTk's own focus_set() targets
            t.configure(takefocus=1)
            t.bind("<FocusIn>", lambda _e: (ring(self, True), into_view(self)), add="+")
            t.bind("<FocusOut>", lambda _e: ring(self, False), add="+")
            for key in ("<space>", "<Return>"):
                t.bind(key, lambda _e: (getattr(self, activate)(), "break")[1], add="+")

        cls.__init__ = __init__

    for cls, activate, ring in ((ctk.CTkButton, "invoke", border), (ctk.CTkSwitch, "toggle", border),
                                (ctk.CTkCheckBox, "toggle", underline), (ctk.CTkRadioButton, "invoke", underline)):
        wrap(cls, activate, ring)
    ctk.CTkButton._keyboard = True


def animations_on() -> bool:
    """Windows Settings > Accessibility > Animation effects. Motion is skipped when it is off."""
    try:
        import ctypes
        on = ctypes.c_int(1)
        ctypes.windll.user32.SystemParametersInfoW(0x1042, 0, ctypes.byref(on), 0)  # SPI_GETCLIENTAREAANIMATION
        return bool(on.value)
    except Exception:
        return True


ANIMATE = animations_on()


def fade_in(win, ms: int = 180) -> None:
    """Fade a window in. Any failure leaves it fully visible, never stuck transparent."""
    if not ANIMATE:
        return
    steps = 9

    def step(i: int) -> None:
        try:
            if win.winfo_exists():
                win.attributes("-alpha", i / steps)
                if i < steps:
                    win.after(ms // steps, step, i + 1)
        except Exception:
            win.attributes("-alpha", 1.0)

    try:
        win.attributes("-alpha", 0.0)
        win.after(10, step, 1)
    except Exception:
        pass


def count_up(label, n: int, ms: int = 450) -> None:
    """Animate a number label from 0 to n with an ease-out, ending exactly on n."""
    if not ANIMATE or n <= 0:
        return label.configure(text=f"{n:,}")
    steps = 15

    def step(i: int) -> None:
        if label.winfo_exists():
            label.configure(text=f"{round(n * (1 - (1 - i / steps) ** 3)):,}")
            if i < steps:
                label.after(ms // steps, step, i + 1)

    step(1)


def reveal(p: Path) -> None:
    """Show the item selected in Explorer. Never opens it: that could run an installer."""
    if p.exists():
        subprocess.Popen(["explorer", "/select,", str(p)])  # list args: no hand-built quoting


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
    fade_in(win, 140)
    win.after(50, lambda: win.winfo_exists() and (win.grab_set(), cancel.focus_set()))  # Esc within 50 ms
    master.wait_window(win)
    return result["ok"]


class Tab(ctk.CTkFrame):
    """Title, optional header row, scrollable checkbox list, and a footer with one action.

    With a filter typed, only matching rows are shown, and only shown rows count as selected,
    so an action never touches something the user can't see.
    """

    def __init__(self, master, title: str, action: str, on_action: Callable[[], None],
                 size_of: Callable[[object], int] | None = None, subtitle: str = "", search: bool = True,
                 reclaim: bool = False):
        super().__init__(master, fg_color="transparent")
        self.size_of = size_of
        self.reclaim = reclaim  # the footer reads "Selected to reclaim: X" instead of "Selected: N items, X"
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
        self.head = ctk.CTkFrame(self, fg_color="transparent", height=0)  # default 200 px leaves a gap when empty
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

    def set_rows(self, rows: list[tuple], checked: bool, note: str = "") -> None:
        """Rows are (text, object) or (text, object, why): `why` is shown as a pill beside the row."""
        if not rows:
            return self.clear("Nothing to clean. Nice.")
        self._reset()
        if note:
            ctk.CTkLabel(self.list, text=note, font=(FONT, 13), text_color=T.TEXT_MUTED,
                         anchor="w").pack(fill="x", padx=12, pady=4)

        def make(row) -> None:
            text, obj, *why = row
            if obj is None:  # a heading: a row like the others (so filtering keeps the order), never selectable
                self.items.append((ctk.BooleanVar(value=False), None))
                self._add_row(ctk.CTkLabel(self.list, text=text, font=(FONT, 14, "bold"), text_color=T.TEXT,
                                           anchor="w"), "", fill="x", padx=12, pady=(10, 2))
                return
            v = ctk.BooleanVar(value=checked)
            self.items.append((v, obj))
            line = ctk.CTkFrame(self.list, fg_color="transparent") if why else self.list
            cb = ctk.CTkCheckBox(line, text=text, variable=v, font=(FONT, 15), text_color=T.TEXT,
                                 fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER, border_color=T.BORDER_CONTROL,
                                 checkmark_color=T.ON_ACCENT, command=self.update_selection)
            if isinstance(obj, Path):
                cb.bind("<Double-Button-1>", lambda _e, p=obj: reveal(p))
            if not why:
                return self._add_row(cb, text, anchor="w", padx=12, pady=6)
            cb.pack(side="left")
            pill(line, why[0]).pack(side="right", padx=(12, 0))
            self._add_row(line, f"{text} {why[0]}", fill="x", padx=12, pady=6)

        self._build_chunked(rows, make)

    def select_all(self, on: bool) -> None:
        for (v, _), shown in zip(self.items, self._shown()):
            if shown:
                v.set(on)
        self.update_selection()

    def selected(self) -> list:
        return [o for (v, o), shown in zip(self.items, self._shown()) if shown and v.get() and o is not None]

    def update_selection(self) -> None:
        sel = self.selected()
        total = sum(self.size_of(o) for o in sel) if self.size_of else 0
        if self.reclaim:
            text = f"Selected to reclaim: {core.human_size(total)}" + (f" · {plural(len(sel), 'item')}" if sel else "")
        else:
            text = f"Selected: {plural(len(sel), 'item')}" + (f", {core.human_size(total)}" if self.size_of and sel else "")
        if self._q:
            text += f"  ·  {sum(self._shown())} of {len(self.rows)} shown"
        self.sel_label.configure(text=text)
        set_primary(self.action_btn, bool(sel) and not self.locked)


class DupTab(Tab):
    """One row per group: a radio per copy picks the keeper; a checkbox includes the group."""

    def __init__(self, master, on_action, size_of):
        self.groups: list[tuple[ctk.BooleanVar, ctk.IntVar, core.DupGroup]] = []
        self.copies: list[tuple[ctk.BooleanVar, core.Redownload, int]] = []  # (ticked, pair, index in self.rows)
        super().__init__(master, "Duplicates", "Send to Recycle Bin", on_action, size_of=size_of,
                         subtitle=HELP["Duplicates"], reclaim=True)

    def _reset(self) -> None:
        self.groups = []
        self.copies = []
        super()._reset()

    def set_groups(self, groups: list[core.DupGroup], base: Path, redownloads: list[core.Redownload] = ()) -> None:
        if not groups and not redownloads:
            return self.clear("Nothing to clean. Nice.")
        self._reset()
        cap = MAX_ROWS["Duplicates"]
        if len(groups) > cap:
            ctk.CTkLabel(self.list, text=f"Showing the {cap} groups that free the most space.",
                         font=(FONT, 13), text_color=T.TEXT_MUTED).pack(anchor="w", padx=12, pady=4)

        def make_copy(r: core.Redownload) -> None:
            box = ctk.CTkFrame(self.list, corner_radius=T.R_BUTTON, fg_color=T.SURFACE_ALT)
            v = ctk.BooleanVar(value=False)  # never preselected
            self.copies.append((v, r, len(self.rows)))
            try:
                where = str(r.copy.parent.relative_to(base))
            except ValueError:
                where = str(r.copy.parent)
            ctk.CTkCheckBox(box, text=f"{r.copy.name}" + ("" if where == "." else f"   in {where}"), variable=v,
                            font=(FONT, 14, "bold"), text_color=T.TEXT, fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER,
                            border_color=T.BORDER_CONTROL, checkmark_color=T.ON_ACCENT,
                            command=self.update_selection).pack(anchor="w", padx=10, pady=(8, 2))
            pill(box, f"Numbered copy · {core.size_pair(r.copy_size, r.orig_size)}").pack(anchor="w", padx=34, pady=(0, 4))
            cols = ctk.CTkFrame(box, fg_color="transparent")  # the two files side by side
            cols.pack(anchor="w", padx=34, pady=(0, 8))
            for text in (f"This copy: {core.human_size(r.copy_size)} · {date_text(r.copy_mtime)}",
                         f"Original (stays): {r.original.name} · {core.human_size(r.orig_size)} · "
                         f"{date_text(r.orig_mtime)}"):
                ctk.CTkLabel(cols, text=text, font=(FONT, 13), text_color=T.TEXT_MUTED).pack(side="left", padx=(0, 28))
            self._add_row(box, f"{r.copy.name} {r.original.name}", fill="x", padx=6, pady=6)

        def make(g) -> None:
            if isinstance(g, core.Redownload):
                return make_copy(g)
            if isinstance(g, str):  # section heading
                return self._add_row(ctk.CTkLabel(
                    self.list, text=g, font=(FONT, 13), text_color=T.TEXT_MUTED, anchor="w", justify="left",
                    wraplength=620), "possible re-downloads", fill="x", padx=12, pady=(14, 4))
            box = ctk.CTkFrame(self.list, corner_radius=T.R_BUTTON, fg_color=T.SURFACE_ALT)
            inc, keeper = ctk.BooleanVar(value=True), ctk.IntVar(value=0)
            self.groups.append((inc, keeper, g))
            ctk.CTkCheckBox(box, text=f"{g.files[0].name} · {len(g.files)} copies · "
                                      f"{core.human_size(g.size)} each", variable=inc, font=(FONT, 15, "bold"),
                            text_color=T.TEXT, fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER,
                            border_color=T.BORDER_CONTROL, checkmark_color=T.ON_ACCENT,
                            command=self.update_selection).pack(anchor="w", padx=10, pady=(8, 2))
            radios = []

            def relabel() -> None:  # "Keep" must follow the chosen copy, or the label points at a file being removed
                for i, (rb, shown) in enumerate(radios):
                    keep = i == keeper.get()
                    rb.configure(text=("Keep   " if keep else "") + shown,
                                 text_color=T.ACCENT_TEXT if keep else T.TEXT_MUTED)
                self.update_selection()

            for i, f in enumerate(g.files):
                try:
                    shown = str(f.relative_to(base))
                except ValueError:
                    shown = str(f)
                rb = ctk.CTkRadioButton(box, text="", value=i, variable=keeper,
                                        font=(FONT, 13), text_color=T.TEXT_MUTED, fg_color=T.ACCENT,
                                        hover_color=T.ACCENT_HOVER, border_color=T.BORDER_CONTROL,
                                        command=relabel)
                rb.bind("<Double-Button-1>", lambda _e, p=f: reveal(p))
                rb.pack(anchor="w", padx=28, pady=3)
                radios.append((rb, shown))
            relabel()
            self._add_row(box, " ".join(f.name for f in g.files), fill="x", padx=6, pady=6)

        redl = ([f"Possible re-downloads: {len(redownloads)}. Same folder and type, named like a copy, almost the "
                 "same size, but not identical. Nothing is selected: look at each pair. The original always stays."]
                + list(redownloads)) if redownloads else []
        self._build_chunked(list(groups[:cap]) + redl, make, chunk=4)

    def visible_groups(self) -> list[tuple[ctk.BooleanVar, ctk.IntVar, core.DupGroup]]:
        return [grp for grp, shown in zip(self.groups, self._shown()) if shown]

    def visible_copies(self) -> list[core.Redownload]:
        shown = self._shown()
        return [r for v, r, i in self.copies if v.get() and shown[i]]

    def select_all(self, on: bool) -> None:
        for inc, _, _ in self.visible_groups():
            inc.set(on)
        if not on:  # "Select all" covers exact duplicates only; re-downloads are ticked one by one
            shown = self._shown()
            for v, _, i in self.copies:
                if shown[i]:
                    v.set(False)
        self.update_selection()

    def selected(self) -> list[Path]:
        return [f for inc, keep, g in self.visible_groups() if inc.get()
                for i, f in enumerate(g.files) if i != keep.get()] + [r.copy for r in self.visible_copies()]


class SortTab(Tab):
    """A box per type folder with a checkbox per file. The filter shows or hides whole boxes,
    and everything in a shown box is visible, so only visible files can be selected."""

    def __init__(self, master, on_action):
        self.files: list[tuple[ctk.BooleanVar, core.Move, int]] = []  # (ticked, move, index of its box row)
        super().__init__(master, "Sort by type", "Sort files", on_action, size_of=lambda m: m.size,
                         subtitle=HELP["Sort"])

    def _reset(self) -> None:
        self.files = []
        super()._reset()

    def set_moves(self, moves: list[core.Move]) -> None:
        if not moves:
            return self.clear("Nothing to sort. Nice.")
        self._reset()
        cap = MAX_ROWS["Sort"]
        if len(moves) > cap:
            ctk.CTkLabel(self.list, text=f"Showing {cap} of {len(moves)} files. Sort these, then scan again for the rest.",
                         font=(FONT, 13), text_color=T.TEXT_MUTED).pack(anchor="w", padx=12, pady=4)
        by_cat: dict[str, list[core.Move]] = {}
        for m in moves[:cap]:
            by_cat.setdefault(m.category, []).append(m)
        flat: list = []  # a category name opens a box; the moves after it fill that box
        for cat in sorted(by_cat):
            flat += [cat] + by_cat[cat]
        box: dict[str, ctk.CTkFrame] = {}

        def make(item) -> None:
            if isinstance(item, str):
                ms = by_cat[item]
                b = box["now"] = ctk.CTkFrame(self.list, corner_radius=T.R_BUTTON, fg_color=T.SURFACE_ALT)
                ctk.CTkLabel(b, text=f"{item} · {plural(len(ms), 'file')} · {core.human_size(sum(m.size for m in ms))}",
                             font=(FONT, 15, "bold"), text_color=T.TEXT).pack(anchor="w", padx=12, pady=(8, 2))
                self._add_row(b, " ".join([item] + [m.src.name for m in ms]), fill="x", padx=6, pady=6)
                return
            v = ctk.BooleanVar(value=True)
            self.files.append((v, item, len(self.rows) - 1))
            cb = ctk.CTkCheckBox(box["now"], text=item.src.name, variable=v, font=(FONT, 14), text_color=T.TEXT,
                                 fg_color=T.ACCENT, hover_color=T.ACCENT_HOVER, border_color=T.BORDER_CONTROL,
                                 checkmark_color=T.ON_ACCENT, command=self.update_selection)
            cb.bind("<Double-Button-1>", lambda _e, p=item.src: reveal(p))
            cb.pack(anchor="w", padx=28, pady=(3, 6))

        self._build_chunked(flat, make)

    def _visible(self) -> list[tuple[ctk.BooleanVar, core.Move]]:
        shown = self._shown()
        return [(v, m) for v, m, i in self.files if shown[i]]

    def select_all(self, on: bool) -> None:
        for v, _ in self._visible():
            v.set(on)
        self.update_selection()

    def selected(self) -> list[core.Move]:
        return [m for v, m in self._visible() if v.get()]

    def update_selection(self) -> None:
        super().update_selection()
        n = len(self.selected())
        self.action_btn.configure(text=f"Sort {plural(n, 'file')}" if n else "Sort files")


class App(ctk.CTk):
    def __init__(self) -> None:
        keyboard_focus()
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
        # The last scan's file list, so filter changes don't walk the folder again. Actions still
        # re-check the disk, and every action rescans afterwards.
        self.entries: list[core.Entry] | None = None
        self.settings = T.load_settings()
        pins = self.settings.get("pins")  # "never touch" files and folders; the file is user-editable
        self.pins: list[str] = [p for p in pins if isinstance(p, str)] if isinstance(pins, list) else []
        self._recent = core.recent_hours_setting(self.settings.get("recent_hours"))
        self.recent_text = ctk.StringVar(value=str(self._recent))
        self.leftovers: list[core.Leftover] = []
        self._large: list[core.FileInfo] = []

        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(0, weight=1)
        self._build_sidebar()
        self.pages: dict[str, ctk.CTkFrame] = {"Home": self._build_home()}
        self.sort = SortTab(self, self.move)
        self._build_sort_header()
        self.dups = DupTab(self, self.remove_dups, size_of=self._size)
        self.large = Tab(self, "Large & old files", "Send to Recycle Bin",
                         lambda: self.recycle(self.large.selected()), size_of=self._size,
                         subtitle=HELP["Large & Old"], reclaim=True)
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
        self.after(300, self._fit_screen)
        fade_in(self, 220)
        self.bind("<Control-f>", lambda _e: self._focus_filter())
        for i, name in enumerate(NAMES + ("Settings",), 1):
            self.bind(f"<Control-Key-{i}>", lambda _e, n=name: self.show(n))
        last = self.settings.get("last_folder")
        if self.settings.get("reopen") and last and core.check_root(last)[0]:
            self.set_folder(Path(last))

    def _fit_screen(self) -> None:
        """1100x720 at 150% scaling is taller than a 1080p screen, hiding the action buttons; fill the screen then."""
        if self.winfo_height() > self.winfo_screenheight() - 80 or self.winfo_width() > self.winfo_screenwidth():
            self.state("zoomed")

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
            b = ctk.CTkButton(bar, text=NAV_LABEL.get(name, name), anchor="w", font=(FONT, 15), height=40, fg_color="transparent",
                              text_color=T.TEXT_NAV, hover_color=T.SURFACE_ALT,
                              corner_radius=T.R_NAV, command=lambda n=name: self.show(n))
            if name == "Settings":  # macOS-style: settings sit apart, at the bottom
                b.pack(side="bottom", fill="x", padx=12, pady=(0, 16))
            else:
                b.pack(fill="x", padx=12, pady=2)
            self.nav[name] = b

    def _build_settings(self) -> ctk.CTkFrame:
        """Grouped rows in the style of macOS System Settings. Every change saves immediately.

        Scrollable, because at 150% display scaling in a small window the last rows don't fit.
        """
        f = ctk.CTkScrollableFrame(self, fg_color="transparent")
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

        pin_card = group("Never touch")

        def fill_pins() -> None:
            for w in pin_card.winfo_children():
                w.destroy()
            r = row(pin_card, "Pinned files and folders", "Skipped by every scan. Never moved or sent to the Bin.")
            for text, ask in (("Add file…", filedialog.askopenfilename), ("Add folder…", filedialog.askdirectory)):
                ctk.CTkButton(r, text=text, width=110, height=34, corner_radius=T.R_BUTTON, font=(FONT, 14),
                              command=lambda a=ask: self._add_pin(a(title="Never touch this")), **GHOST
                              ).pack(side="right", padx=(8, 0))
            for p in self.pins:
                line = ctk.CTkFrame(pin_card, fg_color="transparent")
                line.pack(fill="x", padx=16, pady=(0, 6))
                ctk.CTkLabel(line, text=p, font=(FONT, 13.5), text_color=T.TEXT, anchor="w",
                             wraplength=520, justify="left").pack(side="left")
                ctk.CTkButton(line, text="Remove", width=80, height=30, corner_radius=T.R_BUTTON, font=(FONT, 13),
                              command=lambda q=p: self._set_pins([x for x in self.pins if x != q]), **PLAIN
                              ).pack(side="right")

        self._fill_pins = fill_pins
        fill_pins()

        card = group("About")
        r = row(card, f"Declutter {__version__}", "Free and open source (MIT). Your files never leave this computer.")
        for text, url in (("Support", SUPPORT), ("Source code", REPO), ("Website", SITE)):
            ctk.CTkButton(r, text=text, width=100, height=34, corner_radius=T.R_BUTTON, font=(FONT, 14),
                          command=lambda u=url: webbrowser.open(u), **PLAIN).pack(side="right", padx=(8, 0))
        return f

    def _add_pin(self, picked: str) -> None:
        if not picked:
            return  # dialog cancelled
        p = str(Path(picked).resolve())
        if not core.is_pinned(p, self.pins):
            self._set_pins(self.pins + [p])

    def _set_pins(self, pins: list[str]) -> None:
        self.pins = pins
        T.save_settings(pins=pins)
        self._fill_pins()
        if self.entries is not None and not self._busy:
            self.scan()  # refresh results so pinned items disappear from the lists
        elif self.entries is not None:
            self.status.configure(text="Pins saved. Scan again to update the lists.")

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
        self.current = name

    def _focus_filter(self) -> None:
        search = getattr(self.pages[self.current], "search", None)
        if search is not None:
            search.focus_set()

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

    def _set_busy(self, busy: bool, cancellable: bool = True) -> None:
        self._busy = busy
        for t in self.tabs:
            t.locked = busy
            t.update_selection()
        set_primary(self.scan_btn, not busy and self.folder is not None)
        if busy:
            self.progress.grid(row=0, column=1, padx=(8, 8 if cancellable else 16))
            if cancellable:
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
                          command=lambda l=label: self.set_folder(core.known_folder(l)), **GHOST
                          ).pack(side="left", padx=(0, 10))
        ctk.CTkButton(row, text="Choose folder…", font=(FONT, 15), corner_radius=T.R_BUTTON, width=140, height=40,
                      command=self.choose_folder, **GHOST).pack(side="left")
        self.folder_label = ctk.CTkLabel(f, text="No folder chosen yet", font=(FONT, 15),
                                         text_color=T.TEXT_MUTED, anchor="w")
        self.folder_label.grid(columnspan=2, sticky="w")
        self.scan_btn = ctk.CTkButton(f, text="Scan", font=(FONT, 16, "bold"), height=44, width=160,
                                      corner_radius=T.R_BUTTON, state="disabled", command=self.scan, **PRIMARY)
        self.scan_btn.grid(columnspan=2, sticky="w", pady=T.GAP)
        set_primary(self.scan_btn, False)
        # Summary cards: big number over a muted caption (DESIGN.md "card number 34"). Gridded after a scan.
        self.cards: dict[str, tuple[ctk.CTkFrame, ctk.CTkLabel, ctk.CTkLabel]] = {}
        for name in NAMES[1:]:
            card = ctk.CTkFrame(f, corner_radius=T.R_CARD, fg_color=T.SURFACE, cursor="hand2")
            num = ctk.CTkLabel(card, text="0", font=(FONT, 34, "bold"), text_color=T.TEXT, cursor="hand2")
            cap = ctk.CTkLabel(card, text="", font=(FONT, 14), text_color=T.TEXT_MUTED, justify="left",
                               cursor="hand2")
            num.pack(anchor="w", padx=20, pady=(14, 0))
            cap.pack(anchor="w", padx=20, pady=(0, 16))
            for w in (card, num, cap):
                w.bind("<Button-1>", lambda _e, n=name: self.show(n))
                w.bind("<Enter>", lambda _e, c=card: c.configure(fg_color=T.SURFACE_ALT))
                w.bind("<Leave>", lambda _e, c=card: c.configure(fg_color=T.SURFACE))
            self.cards[name] = (card, num, cap)
        return f

    def _build_sort_header(self) -> None:
        self.sort_hint = ctk.CTkLabel(self.sort.head, text=SORT_HINT, font=(FONT, 13.5), text_color=T.TEXT_MUTED)
        self.sort_hint.pack(anchor="w")
        grace = ctk.CTkFrame(self.sort.head, fg_color="transparent")
        grace.pack(anchor="w", pady=(6, 0))
        ctk.CTkLabel(grace, text="Leave files touched in the last", font=(FONT, 14), text_color=T.TEXT).pack(
            side="left", padx=(0, 6))
        e = ctk.CTkEntry(grace, textvariable=self.recent_text, width=56, fg_color=T.SURFACE, text_color=T.TEXT,
                         border_color=T.BORDER_CONTROL, corner_radius=T.R_BUTTON)
        e.pack(side="left", padx=(0, 6))
        e.bind("<Return>", lambda _e: self._set_recent())
        e.bind("<FocusOut>", lambda _e: self._set_recent())
        ctk.CTkLabel(grace, text="hours alone (0 to 72, 0 turns it off)", font=(FONT, 13),
                     text_color=T.TEXT_MUTED).pack(side="left")
        # "Undo last sort" banner: shown while the undo log has a batch, also after a restart
        self.undo_bar = ctk.CTkFrame(self.sort.head, corner_radius=T.R_BUTTON, fg_color=T.ACCENT_SOFT)
        self.undo_text = ctk.CTkLabel(self.undo_bar, text="", font=(FONT, 14), text_color=T.TEXT)
        self.undo_text.pack(side="left", padx=14, pady=8)
        self.undo_btn = ctk.CTkButton(self.undo_bar, text="Undo last sort", corner_radius=T.R_BUTTON, height=34,
                                      font=(FONT, 14), command=self.undo, **GHOST)
        self.undo_btn.pack(side="right", padx=8, pady=6)

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
        self.large_hint = ctk.CTkLabel(self.large.head, text="0 turns a filter off", font=(FONT, 13),
                                       text_color=T.TEXT_MUTED)
        self.large_hint.pack(side="left")
        ctk.CTkButton(self.large.head, text="Apply", width=80, height=38, corner_radius=T.R_BUTTON,
                      font=(FONT, 14), command=self.refilter, **GHOST).pack(side="right")
        self.leftovers_only = False
        self.pill = ctk.CTkButton(self.large.head, text="Likely leftovers only", width=150, height=34,
                                  corner_radius=T.R_PILL, font=(FONT, 14), command=self._toggle_leftovers, **GHOST)
        self.pill.pack(side="right", padx=(0, 10))

    def _toggle_leftovers(self) -> None:
        on = self.leftovers_only = not self.leftovers_only
        self.pill.configure(fg_color=T.ACCENT_SOFT if on else "transparent",
                            text_color=T.ACCENT_TEXT if on else T.TEXT_NAV,
                            border_color=T.ACCENT_TEXT if on else T.BORDER_CONTROL)
        self.large_hint.configure(text="Extracted zips, installers over 7 days old in Downloads, dead downloads"
                                  if on else "0 turns a filter off")
        self._fill_large()

    def _set_recent(self) -> None:
        n = core.recent_hours_setting(self.recent_text.get().strip(), default=self._recent)
        self.recent_text.set(str(n))
        if n != self._recent:
            self._recent = n
            T.save_settings(recent_hours=n)
            self._replan()

    def _replan(self) -> None:
        """Re-plan the sort from the last scan (no new walk), e.g. after the grace period changed."""
        if not self.folder or self.entries is None:
            return
        folder, c, entries, hours = self.folder, self._cancel.is_set, self.entries, self._recent
        self._run(lambda: core.plan_sort(folder, cancel=c, entries=entries, recent_hours=hours), self._fill_sort)

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
        self.entries, self.leftovers, self._large = None, [], []
        self.sort_hint.configure(text=SORT_HINT)
        T.save_settings(last_folder=str(self.folder))
        self.folder_label.configure(text=f"Folder: {self.folder}")
        set_primary(self.scan_btn, True)
        for card, _, _ in self.cards.values():
            card.grid_forget()
        for name, t in zip(NAMES[1:], self.tabs):
            t.clear("Press Scan on Home to look through this folder.")
            self.nav[name].configure(text=NAV_LABEL.get(name, name))
        self.status.configure(text="Folder chosen. Press Scan.")
        self.show("Home")

    def rel(self, p: Path) -> str:
        try:
            return str(p.relative_to(self.folder))
        except (ValueError, TypeError):
            return str(p)

    def _progress(self, text: str) -> None:
        self._events.put((None, text))

    def _run(self, work, done, cancellable: bool = True) -> None:
        """Run work() on a thread; call done(result) on the UI thread, or show the error.

        Moves and Recycle Bin jobs can't be stopped halfway, so they get no Cancel button and
        their result is always reported.
        """
        if self._busy:
            return self.status.configure(text="Still working. Please wait a moment.")
        self._cancel.clear()
        # Free destroyed rows' Tk variables here: if the worker's garbage collection freed them,
        # tkinter would call into Tk from the wrong thread (stalls, "main thread is not in main loop").
        gc.collect()
        self._set_busy(True, cancellable)

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
        folder, c, p, pins, hours = self.folder, self._cancel.is_set, self._progress, list(self.pins), self._recent

        def work():
            p("Reading the folder…")
            entries = core.scan_entries(folder, cancel=c, ignore=pins)  # one walk shared by all checks
            moves = core.plan_sort(folder, cancel=c, entries=entries, recent_hours=hours)
            dups = core.find_duplicates(folder, progress=p, cancel=c, entries=entries)
            large = core.find_large_old(folder, *params, cancel=c, entries=entries)
            p("Looking for leftovers…")
            left = core.find_leftovers(folder, entries=entries, cancel=c)
            redl = core.find_redownloads(folder, entries=entries, cancel=c,
                                         exclude={f for g in dups for f in g.files})
            p("Looking for empty folders…")
            return entries, moves, dups, large, left, redl, core.find_empty_dirs(folder, cancel=c, ignore=pins)

        self.status.configure(text="Scanning…")
        self._run(work, self._scan_done)

    def _scan_done(self, r) -> None:
        self.entries, moves, dups, large, self.leftovers, redl, empty = r
        self.sizes = {}
        self._fill_sort(moves)
        self._fill_dups(dups, redl)
        self._fill_large(large)
        self._fill_empty(empty)
        extra = sum(len(g.files) - 1 for g in dups)
        reclaim = sum(g.size * (len(g.files) - 1) for g in dups)
        n_large = len({f.path for f in large} | {x.path for x in self.leftovers})
        counts = {"Sort": len(moves), "Duplicates": extra + len(redl), "Large & Old": n_large,
                  "Empty Folders": len(empty)}

        def word(n: int, w: str) -> str:
            return w if n == 1 else w + "s"

        captions = {
            "Sort": f"{word(len(moves), 'file')} to sort",
            "Duplicates": f"{word(extra, 'duplicate')} · {core.human_size(reclaim)} reclaimable"
                          + (f"\n+ {len(redl)} possible re-download{'s' * (len(redl) != 1)}" if redl else ""),
            "Large & Old": f"large, old or leftover {word(n_large, 'file')}",
            "Empty Folders": f"empty {word(len(empty), 'folder')}",
        }
        for i, (name, (card, num, cap)) in enumerate(self.cards.items()):
            cap.configure(text=captions[name] + ("\nNothing to clean. Nice." if not counts[name] else "\nClick to review"))
            card.grid(row=5 + i // 2, column=i % 2, sticky="ew", padx=6, pady=6)  # rows 0-4: title … Scan button
            count_up(num, counts[name])
            label = NAV_LABEL.get(name, name)
            self.nav[name].configure(text=f"{label}  ({counts[name]})" if counts[name] else label)
        total = sum(counts.values())
        self.status.configure(text="Scan finished. Review the cards below." if total
                              else "Scan finished. This folder is already tidy.")

    def _fill_sort(self, moves: core.SortPlan) -> None:
        self.moves = moves
        n = moves.recent_left
        self.sort_hint.configure(text=SORT_HINT + (f" {plural(n, 'recent file')} left alone." if n else ""))
        self.sort.set_moves(moves)

    def _size(self, p: Path) -> int:
        return self.sizes[p] if p in self.sizes else file_size(p)

    def _unpinned(self, paths: list) -> list:
        """Re-check right before acting: a pin added after the scan still protects its files."""
        return [p for p in paths if not core.is_pinned(getattr(p, "src", p), self.pins)]

    def _fill_dups(self, groups: list[core.DupGroup], redownloads: list[core.Redownload] = ()) -> None:
        for g in groups:
            for f in g.files:
                self.sizes[f] = g.size
        self.sizes.update({r.copy: r.copy_size for r in redownloads})
        self.dups.set_groups(groups, self.folder, redownloads)

    def _fill_large(self, large: list[core.FileInfo] | None = None) -> None:
        """Likely leftovers first (heading, nothing ticked), then the large/old files that aren't leftovers."""
        if large is not None:
            self._large = large
        cap = MAX_ROWS["Large & Old"]
        left = self.leftovers
        mine = {x.path for x in left}
        rest = [f for f in self._large if f.path not in mine]
        self.sizes.update({f.path: f.size for f in self._large})
        self.sizes.update({x.path: x.size for x in left})
        days = lambda ts: int((time.time() - ts) / 86400)  # noqa: E731
        rows: list[tuple[str, object]] = []
        if left:
            rows.append((f"Likely leftovers ({len(left)}). Look at each one before you tick it. Nothing is ticked.",
                         None))
            rows += [(f"{core.human_size(x.size)} · {days(x.mtime)} days old · {self.rel(x.path)}", x.path,
                      x.reason + (f" ({x.path.stem})" if x.reason == core.EXTRACTED else "")) for x in left[:cap]]
        if not self.leftovers_only:
            if left and rest:
                rows.append(("Large and old files", None))
            rows += [(f"{core.human_size(f.size)} · {f.age_days} days old · {self.rel(f.path)}", f.path)
                     for f in rest[:cap]]
        self.large.set_rows(rows, False, f"Showing the {cap} biggest." if len(rest) > cap else "")

    def _fill_empty(self, empty: list[Path]) -> None:
        cap = MAX_ROWS["Empty Folders"]
        rows = [(self.rel(p), p) for p in empty[:cap]]
        self.empty.set_rows(rows, True, f"Showing {cap} of {len(empty)}." if len(empty) > cap else "")

    # ---------- sort actions ----------

    def move(self) -> None:
        sel = self._unpinned(self.sort.selected())
        if sel and confirm(
                self, "Sort files?", f"Move {plural(len(sel), 'file')} into type folders inside\n{self.folder}?\n\n"
                "Nothing is deleted. You can undo this with \"Undo last sort\".", f"Sort {plural(len(sel), 'file')}"):
            folder, hours = self.folder, self._recent
            self._run(lambda: core.apply_moves(sel, folder, recent_hours=hours), self._move_done, cancellable=False)

    def _move_done(self, result: tuple[int, list[str]]) -> None:
        self._report("Done", f"Moved {plural(result[0], 'file')}.", result[1])
        self._refresh_undo()
        self.scan()

    def undo(self) -> None:
        info = core.peek_undo()
        if not info:
            return messagebox.showinfo("Undo", "Nothing to undo.")
        when = "" if info.age_seconds is None else f" ({core.ago(info.age_seconds)})"
        if confirm(self, "Undo last sort?", f"Undo the sort of {plural(info.count, 'file')} in "
                   f"{info.folder.name or info.folder}{when}?\n\nThe files go back where they were.",
                   f"Undo {plural(info.count, 'file')}"):
            self._run(core.undo_last, self._undo_done, cancellable=False)

    def _undo_done(self, result: tuple[int, list[str]]) -> None:
        self._report("Undo", f"Restored {plural(result[0], 'file')}.", result[1])
        self._refresh_undo()
        self.scan()

    def _refresh_undo(self) -> None:
        n = core.undo_count()
        if n:
            self.undo_text.configure(text=f"Your last sort moved {plural(n, 'file')}.")
            self.undo_bar.pack(fill="x", pady=(0, 8), before=self.sort_hint)
        else:
            self.undo_bar.pack_forget()

    # ---------- recycle-bin actions ----------

    def refilter(self) -> None:
        params = self._large_params()
        if not self.folder or not params:
            return
        folder, c, entries = self.folder, self._cancel.is_set, self.entries
        self._run(lambda: core.find_large_old(folder, *params, cancel=c, entries=entries), self._fill_large)

    def remove_dups(self) -> None:
        removals = [(g, self._unpinned([f for i, f in enumerate(g.files) if i != keep.get()]))
                    for inc, keep, g in self.dups.visible_groups() if inc.get()]
        try:  # safety rule 6; the radio buttons already make this impossible
            core.check_dup_selection([g for _, _, g in self.dups.groups],
                                     {f for _, rem in removals for f in rem})
        except ValueError as e:
            return messagebox.showerror("Keep one copy", str(e))
        copies = [r for r in self.dups.visible_copies() if not core.is_pinned(r.copy, self.pins)]
        n = sum(len(rem) for _, rem in removals) + len(copies)
        if n and self._confirm_bin(n):
            folder, going = self.folder, {f for _, rem in removals for f in rem}

            def work() -> tuple[int, list[str]]:
                a, b = core.trash_duplicates(removals, folder), core.trash_redownloads(copies, folder, removing=going)
                return a[0] + b[0], a[1] + b[1]

            self._run(work, self._recycled, cancellable=False)

    def _confirm_bin(self, n: int) -> bool:
        return confirm(
            self, "Send to Recycle Bin?", f"Send {plural(n, 'item')} to the Recycle Bin?\n\n"
            "They are not deleted. You can restore them from the Recycle Bin.", f"Send {plural(n, 'item')}")

    def recycle(self, paths: list[Path]) -> None:
        paths = self._unpinned(paths)
        if paths and self._confirm_bin(len(paths)):
            folder = self.folder
            self._run(lambda: core.to_recycle_bin(paths, folder), self._recycled, cancellable=False)

    def recycle_empty(self, paths: list[Path]) -> None:
        paths = self._unpinned(paths)
        if paths and self._confirm_bin(len(paths)):
            folder = self.folder
            self._run(lambda: core.trash_empty_dirs(paths, folder), self._recycled, cancellable=False)

    def _recycled(self, result: tuple[int, list[str]]) -> None:
        self._report("Done", f"Sent {plural(result[0], 'item')} to the Recycle Bin. Restore them from the Bin if needed.", result[1])
        self.scan()

    def _report(self, title: str, msg: str, errors: list[str]) -> None:
        """Problems need reading, so they get a dialog; plain success is a toast that needs no click."""
        if errors:
            msg += f"\n\n{len(errors)} had problems:\n" + "\n".join(errors[:8])
            if len(errors) > 8:
                msg += f"\n…and {len(errors) - 8} more."
            return messagebox.showwarning(title, msg)
        self.toast(msg)

    def toast(self, text: str, ms: int = 4500) -> None:
        """Slides up above the status bar, then slides away by itself."""
        old = getattr(self, "_toast", None)
        if old is not None and old.winfo_exists():
            old.destroy()
        t = self._toast = ctk.CTkFrame(self, corner_radius=T.R_BUTTON, fg_color=T.TEXT)
        ctk.CTkLabel(t, text=text, font=(FONT, 14), text_color=T.BG, wraplength=460,
                     justify="left").pack(padx=18, pady=12)
        rest = -(48 + 16)  # just above the 48 px status bar

        def slide(start: float, end: float, then=None, i: int = 1, steps: int = 10) -> None:
            if not t.winfo_exists():
                return
            y = end if not ANIMATE else start + (end - start) * (1 - (1 - i / steps) ** 3)
            t.place(relx=1.0, rely=1.0, x=-T.PAD_X, y=y, anchor="se")
            if ANIMATE and i < steps:
                self.after(18, slide, start, end, then, i + 1)
            elif then:
                then()

        slide(rest + 36, rest)
        self.after(ms, lambda: t.winfo_exists() and slide(rest, rest + 120, t.destroy))


def main() -> None:
    app = App()
    if len(sys.argv) > 1:  # a folder given on the command line wins over "reopen last folder"
        app.after(0, lambda: app.set_folder(Path(sys.argv[1])))
    app.mainloop()
