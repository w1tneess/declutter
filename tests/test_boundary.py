"""R4: project markers and the portable-app boundary. Skipped folders are never entered and are reported."""
import pytest

from declutter import core

from test_core import touch


@pytest.fixture
def root(tmp_path):
    r = tmp_path / "messy"
    r.mkdir()
    return r


def scan(root):
    skipped = []
    entries = core.scan_entries(root, skipped=skipped)
    return sorted(p.relative_to(root).as_posix() for p, _ in entries), sorted(p.name for p in skipped)


DIR_MARKERS = [".git", "node_modules", ".venv", "venv", ".svn", ".hg"]
FILE_MARKERS = ["package.json", "pyproject.toml", "Cargo.toml", "app.sln", "APP.SLN", "Cargo.TOML"]


@pytest.mark.parametrize("marker", DIR_MARKERS + FILE_MARKERS)
def test_project_folders_are_never_entered(root, marker):
    touch(root / "proj" / marker / "inner.txt") if marker in DIR_MARKERS else touch(root / "proj" / marker)
    touch(root / "proj" / "code.py")
    touch(root / "loose.txt")
    assert scan(root) == (["loose.txt"], ["proj"])
    assert core.find_empty_dirs(root) == []


def test_project_marker_as_a_root_is_refused(root):
    touch(root / "pyproject.toml")
    ok, reason = core.check_root(root)
    assert not ok and "project" in reason.lower()


@pytest.mark.parametrize("dll, exe", [("lib.dll", "run.exe"), ("LIB.DLL", "RUN.EXE")])
def test_a_folder_with_an_exe_and_a_dll_is_a_portable_app(root, dll, exe):
    touch(root / "tool" / exe)
    touch(root / "tool" / dll)
    touch(root / "tool" / "data" / "notes.txt")
    touch(root / "loose.txt")
    assert scan(root) == (["loose.txt"], ["tool"])


def test_only_an_exe_or_only_a_dll_is_a_plain_folder(root):
    touch(root / "setup" / "installer.exe")
    touch(root / "setup" / "readme.txt")
    touch(root / "libs" / "helper.dll")
    files, skipped = scan(root)
    assert files == ["libs/helper.dll", "setup/installer.exe", "setup/readme.txt"] and skipped == []


def test_an_app_folder_inside_another_is_counted_once(root):
    touch(root / "tool" / "run.exe")
    touch(root / "tool" / "lib.dll")
    touch(root / "tool" / "plugins" / "p.exe")
    touch(root / "tool" / "plugins" / "p.dll")
    assert scan(root) == ([], ["tool"])


def test_app_folders_are_not_searched_for_empty_folders(root):
    touch(root / "tool" / "run.exe")
    touch(root / "tool" / "lib.dll")
    (root / "tool" / "cache").mkdir()
    (root / "plain" / "hollow").mkdir(parents=True)
    assert core.find_empty_dirs(root) == [root / "plain" / "hollow", root / "plain"]  # nothing under tool/


def test_the_chosen_folder_itself_is_never_skipped(root):
    touch(root / "setup.exe")
    touch(root / "fix.dll")
    assert core.check_root(root)[0]
    assert scan(root) == (["fix.dll", "setup.exe"], [])


def test_skipped_is_optional(root):
    touch(root / "tool" / "run.exe")
    touch(root / "tool" / "lib.dll")
    touch(root / "a.txt")
    assert [p.name for p, _ in core.scan_entries(root)] == ["a.txt"]
