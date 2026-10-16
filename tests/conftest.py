import pytest

from declutter import core, theme


@pytest.fixture(autouse=True)
def isolated_data(tmp_path, monkeypatch):
    """Undo log, settings and the pre-0.2 %APPDATA% folder all live in tmp, so tests
    (and the migration code) never read or move the real user's files."""
    log_dir = tmp_path / "_log"
    monkeypatch.setattr(core, "LOG_DIR", log_dir)
    monkeypatch.setattr(core, "UNDO_LOG", log_dir / "undo.json")
    monkeypatch.setattr(core, "OLD_LOG_DIR", tmp_path / "_old")
    monkeypatch.setattr(theme, "_SETTINGS", tmp_path / "settings.json")
