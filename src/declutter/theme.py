"""Design tokens from DESIGN.md. Colours are (light, dark) pairs; widget code never holds hex values."""
from __future__ import annotations

import json

from . import core

FONT = "Segoe UI"

# Apple system look: grouped background, white/elevated surfaces, system grays, hairline separators.
BG = ("#F2F2F7", "#000000")              # systemGroupedBackground
SURFACE = ("#FFFFFF", "#1C1C1E")         # secondarySystemGroupedBackground
SURFACE_ALT = ("#F2F2F7", "#2C2C2E")     # tertiary fill: group headers, hover
BORDER = ("#D1D1D6", "#38383A")          # hairline separator (sidebar, status bar)
BORDER_CONTROL = ("#C7C7CC", "#48484A")  # systemGray3/4: checkbox and bordered-button outlines
DIVIDER = ("#E5E5EA", "#2C2C2E")
TEXT = ("#1D1D1F", "#F5F5F7")
TEXT_MUTED = ("#6E6E73", "#98989D")      # darker than secondaryLabel so it passes 4.5:1
TEXT_NAV = ("#1D1D1F", "#F5F5F7")
# Deeper than #007AFF: white on system blue is ~4:1, below 4.5:1 for normal text.
ACCENT = ("#0071E3", "#0071E3")
ON_ACCENT = ("#FFFFFF", "#FFFFFF")
ACCENT_TEXT = ("#0066CC", "#2997FF")
ACCENT_SOFT = ("#E3EEFC", "#1A2F4A")
ACCENT_HOVER = ("#0077ED", "#0077ED")
FOCUS = TEXT  # keyboard focus ring: the highest-contrast colour in both themes, also on accent fills

R_CARD, R_BUTTON, R_NAV, R_DIALOG, R_PILL = 16, 10, 8, 14, 18
TITLE = 32
SIDEBAR_W = 232
PAD_X, PAD_Y, GAP, CARD_GAP = 40, 32, 20, 16

MODES = ("System", "Light", "Dark")
_SETTINGS = core.LOG_DIR / "settings.json"


def load_settings() -> dict:
    core.migrate(core.OLD_LOG_DIR / _SETTINGS.name, _SETTINGS)
    try:
        data = json.loads(_SETTINGS.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def save_settings(**changes) -> None:
    """Merge changes into the settings file, keeping keys not mentioned."""
    try:
        _SETTINGS.parent.mkdir(parents=True, exist_ok=True)
        _SETTINGS.write_text(json.dumps({**load_settings(), **changes}), encoding="utf-8")
    except OSError:
        pass  # the choice just isn't remembered; never worth an error dialog


def load_mode() -> str:
    mode = load_settings().get("theme")
    return mode if mode in MODES else "System"


def save_mode(mode: str) -> None:
    save_settings(theme=mode)
