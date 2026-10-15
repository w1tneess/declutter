"""Design tokens from DESIGN.md. Colours are (light, dark) pairs; widget code never holds hex values."""
from __future__ import annotations

import json

from . import core

FONT = "Segoe UI"

BG = ("#F3F5F9", "#0B0F17")
SURFACE = ("#FFFFFF", "#121826")
SURFACE_ALT = ("#EEF1F6", "#161D2D")
BORDER = ("#DFE4EC", "#232B3B")
BORDER_CONTROL = ("#CBD2DE", "#2E384C")
DIVIDER = ("#E8ECF2", "#1C2434")
TEXT = ("#0F1623", "#E8ECF4")
TEXT_MUTED = ("#566176", "#9AA4B5")
TEXT_NAV = ("#36415A", "#B4BDCC")
ACCENT = ("#2F6FED", "#2F6FED")
ON_ACCENT = ("#FFFFFF", "#FFFFFF")
ACCENT_TEXT = ("#1F4FB8", "#8DB6FF")
ACCENT_SOFT = ("#E6EEFF", "#1A2744")

ACCENT_HOVER = ("#2559C4", "#2559C4")  # not in DESIGN.md: a darker accent so hover is visible; check on the canvas

R_CARD, R_BUTTON, R_NAV, R_DIALOG = 14, 10, 8, 16
SIDEBAR_W = 232
PAD_X, PAD_Y, GAP, CARD_GAP = 40, 32, 20, 16

MODES = ("System", "Light", "Dark")
_SETTINGS = core.LOG_DIR / "settings.json"


def load_mode() -> str:
    try:
        mode = json.loads(_SETTINGS.read_text(encoding="utf-8")).get("theme")
    except (OSError, ValueError, AttributeError):
        return "System"
    return mode if mode in MODES else "System"


def save_mode(mode: str) -> None:
    try:
        _SETTINGS.parent.mkdir(parents=True, exist_ok=True)
        _SETTINGS.write_text(json.dumps({"theme": mode}), encoding="utf-8")
    except OSError:
        pass  # the choice just isn't remembered; never worth an error dialog
