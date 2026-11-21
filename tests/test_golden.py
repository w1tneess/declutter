"""The committed golden fixtures must match what core.py produces today.

They are the behaviour contract for the Rust port (docs/CONTRACT.md). If this fails after a deliberate change to
core.py, regenerate with `python tools/gen_golden.py`, review the fixture diff, and update the Rust port.
"""
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "tests" / "golden"


@pytest.mark.skipif(sys.platform != "win32", reason="fixtures use Windows file attributes and creation times")
def test_golden_fixtures_are_current(tmp_path):
    sys.path.insert(0, str(ROOT / "tools"))
    try:
        import gen_golden
    finally:
        sys.path.pop(0)
    fresh = tmp_path / "golden"
    assert gen_golden.main(fresh) == 0
    committed = {p.name for p in GOLDEN.glob("*.json")}
    assert committed == {p.name for p in fresh.glob("*.json")}, "fixture set changed: regenerate and commit"
    stale = [n for n in sorted(committed) if (GOLDEN / n).read_text("utf-8") != (fresh / n).read_text("utf-8")]
    assert not stale, f"stale golden fixtures (regenerate with tools/gen_golden.py): {stale}"
