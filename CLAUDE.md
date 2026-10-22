# Declutter

Windows desktop app that declutters a messy folder safely. Full spec: `SPEC.md` (read it first).

## Commands (v0.2.2 Python app)
- Tests: `python -m pytest -q` (config in `pytest.ini`; temp dirs go to `.pytest_tmp/`)
- Mutation check (after changing rules in `core.py`): `python tools/mutation_check.py`; it must print `all caught`
- Run: `python -m declutter` from `src/` (or set `PYTHONPATH=src`)
- Deps: `requirements.txt` (runtime), `requirements-dev.txt` (tests + PyInstaller)

## Layout (v0.2.2 Python app, the oracle until 1.0 ships)
- `src/declutter/core.py` has all file logic, no GUI imports. Test it, don't trust it.
- `src/declutter/app.py` is the CustomTkinter UI. Long work runs on a thread and reports through a queue.
- 1.0 (branch `v1*`) is Rust (`crates/`) + Tauri 2 (`src-tauri/`) + React (`ui/`); read `docs/ARCHITECTURE.md` first. On the maintainer's PC Smart App Control blocks locally built Rust binaries, so do not run `cargo` there; CI is the compiler.

## Rules
- Safety rules in `SPEC.md` are non-negotiable. Ask before changing them.
- Never permanently delete; use the Recycle Bin. Re-check the disk right before every action.
- Tests must never touch real user folders.
- Smallest change that solves the problem. Don't invent library APIs; check the docs.
- Plan and backlog: see ROADMAP.md. Do not put status, test counts or PR numbers in this file.
- Develop and test on Python 3.12, the version CI and the release build use.
