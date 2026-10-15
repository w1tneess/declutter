# Declutter

A simple Windows app that tidies a messy folder. It previews everything first and never permanently deletes anything.

[![CI](../../actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)

## What it does
- **Sort by type**: moves loose files into Images, Documents, Videos, Audio, Archives, Installers, Code, Other. Undo supported.
- **Duplicates**: finds identical files (size, then SHA-256). Keeps the oldest copy unless you choose another.
- **Large & old**: finds big or long-untouched files.
- **Empty folders**: finds folders with no files at any depth.

Removals go to the Recycle Bin. Drive roots, your home folder, Windows, Program Files, AppData, `.git` and `node_modules` are refused. Symlinks, junctions and hidden files are skipped.

## Run from source
```
python -m pip install -r requirements.txt
cd src
python -m declutter
```

## Develop
```
python -m pip install -r requirements-dev.txt
python -m pytest -q
```

## Known limitations
- The `.exe` is unsigned, so Windows SmartScreen shows "Unknown publisher" (More info, then Run anyway).
- Antivirus may occasionally flag PyInstaller builds.
- Items sent to the Recycle Bin are restored from the Bin, not through the app's Undo.

## License
MIT
