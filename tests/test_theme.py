import re
from pathlib import Path

from declutter import theme

SRC = Path(__file__).resolve().parent.parent / "src" / "declutter"


def test_no_hex_colours_outside_theme():
    for f in SRC.glob("*.py"):
        if f.name != "theme.py":
            hits = re.findall(r"#[0-9A-Fa-f]{3,8}\b", f.read_text(encoding="utf-8"))
            assert not hits, f"{f.name} has hex colours {hits}; add a token to theme.py instead"


def test_colour_tokens_are_light_dark_pairs():
    colours = {k: v for k, v in vars(theme).items()
               if k.isupper() and isinstance(v, tuple) and any(str(c).startswith("#") for c in v)}
    assert len(colours) >= 10
    for name, v in colours.items():
        assert len(v) == 2 and all(re.fullmatch(r"#[0-9A-Fa-f]{6}", c) for c in v), name
