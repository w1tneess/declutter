# Declutter

Windows desktop app that declutters a messy folder safely. Full spec: `SPEC.md` (read it first).

## Commands
- Tests: `python -m pytest -q` (config in `pytest.ini`; temp dirs go to `.pytest_tmp/`)
- Run: `python -m declutter` from `src/` (or set `PYTHONPATH=src`)
- Deps: `requirements.txt` (runtime), `requirements-dev.txt` (tests + PyInstaller)

## Layout
- `src/declutter/core.py` has all file logic, no GUI imports. Test it, don't trust it.
- `src/declutter/app.py` is the CustomTkinter UI. Long work runs on a thread and reports through a queue.

## Rules
- Safety rules in `SPEC.md` are non-negotiable. Ask before changing them.
- Never permanently delete; use the Recycle Bin. Re-check the disk right before every action.
- Tests must never touch real user folders.
- Smallest change that solves the problem. Don't invent library APIs; check the docs.
