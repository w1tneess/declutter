# Declutter

A simple Windows app that tidies a messy folder. It previews everything first and never permanently deletes anything.

[![CI](../../actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)

## Install
1. Download `Declutter-Setup.exe` from the [latest release](https://github.com/w1tneess/declutter/releases/latest). Prefer no installer? Take `Declutter-portable.zip`, unzip it anywhere and run `Declutter.exe`.
2. Run the installer. It installs for your account only, so no admin rights are needed.
3. The app is not code-signed yet, so Windows SmartScreen may say "Windows protected your PC" (Unknown publisher). Click **More info**, then **Run anyway**.

Uninstall from Windows Settings > Apps. Website: https://w1tneess.github.io/declutter/ (source in `site/`, published by `.github/workflows/pages.yml`).

## What it does
- **Sort by type**: moves loose files into Images, Documents, Videos, Audio, Archives, Installers, Code, Other. Undo supported.
- **Duplicates**: finds identical files (size, then SHA-256). Keeps the oldest copy unless you choose another.
- **Large & old**: finds big or long-untouched files.
- **Empty folders**: finds folders with no files at any depth.
- **Filter by name** on each list. Only the rows you can see count as selected, so nothing hidden is ever removed.
- **Double-click** a file or folder to see it in Explorer (it is never opened).
- **Settings**: theme (System, Light, Dark), reopen the last folder at start, the Large & old filters, and a "Never touch" list of pinned files and folders, all remembered.
- **Right-click a folder > Tidy with Declutter** (optional when installing) opens the app on that folder. `Declutter.exe <folder>` does the same.

![Duplicates, light theme](site/screenshot-duplicates.png)
![Large & old files, dark theme](site/screenshot-dark.png)

Removals go to the Recycle Bin, and only on local drives (USB sticks and network drives are refused, since they may have no Bin). Drive roots, your home folder, Windows, Program Files and AppData are refused. Left alone: code projects (folders with `.git`, `node_modules`, `.venv`, `package.json` or a `.sln`), downloads in progress, files changed in the last 5 minutes, OneDrive online-only files (nothing gets downloaded), symlinks, junctions and hidden files. If one file fails (locked, too long a path), the rest still go ahead and the failures are listed.

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

Before a release, also run the GUI checks (opens real windows, uses temp folders only): a 10,000-file folder without freezing, cancelling a scan, both themes, 100/125/150% scaling.
```
python tests/gui_check.py
```
With Pillow installed (`pip install pillow`) it saves screenshots to `.pytest_tmp/shots` for a visual check.

## Known limitations
- The `.exe` is unsigned, so Windows SmartScreen shows "Unknown publisher" (More info, then Run anyway).
- Antivirus may occasionally flag PyInstaller builds.
- Items sent to the Recycle Bin are restored from the Bin, not through the app's Undo.

## License
MIT
