from declutter import theme  # conftest.py isolates the settings file


def test_save_settings_merges_keys():
    theme.save_mode("Dark")
    theme.save_settings(reopen=True, last_folder="C:/x")
    theme.save_settings(min_mb="50")
    assert theme.load_settings() == {"theme": "Dark", "reopen": True, "last_folder": "C:/x", "min_mb": "50"}
    assert theme.load_mode() == "Dark"


def test_damaged_or_odd_settings_fall_back(tmp_path):
    theme._SETTINGS.write_text("not json", encoding="utf-8")
    assert theme.load_settings() == {} and theme.load_mode() == "System"
    theme._SETTINGS.write_text('["a list"]', encoding="utf-8")
    assert theme.load_settings() == {}
    theme._SETTINGS.write_text('{"theme": "Purple"}', encoding="utf-8")
    assert theme.load_mode() == "System"
