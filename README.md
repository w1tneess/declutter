# Declutter

A simple Windows app that tidies a messy folder. It previews everything first and never permanently deletes anything.

[![CI](../../actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)

## Install (v0.2.2, the current release)
1. Download `Declutter-Setup.exe` from the [latest release](https://github.com/w1tneess/declutter/releases/latest). Prefer no installer? Take `Declutter-portable.zip`, unzip it anywhere and run `Declutter.exe`.
2. Run the installer. It installs for your account only, so no admin rights are needed.
3. The app is not code-signed yet, so Windows SmartScreen may say "Windows protected your PC" (Unknown publisher). Click **More info**, then **Run anyway**.

Uninstall from Windows Settings > Apps. Website source is in `website/` (deployed with Vercel, see `vercel.json`).

## Declutter 1.0 (in development)
A rewrite is in progress on the branch `v1-rust`: a Rust core, a Tauri 2 shell and a React/TypeScript UI, with the same safety rules (`SPEC.md`). It is **not released**; the install steps above are still for v0.2.2, which stays the shipping app (and the test oracle for the rewrite) until 1.0 is out. A first beta is planned as v0.3.0.

New on that branch: similar-picture groups, sort presets (built-in Downloads, Photos, Work, plus your own), CSV export, a "space freed" tally, and an optional weekly reminder (a Task Scheduler task that runs `declutter --remind`; it only scans and shows a notification, it never moves anything). Not yet ported from v0.2.2: the right-click "Tidy with Declutter" entry, opening on a folder given on the command line, and the diagnostics log. Design: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`DESIGN.md`](DESIGN.md). Plan and status: [`ROADMAP.md`](ROADMAP.md).

The 1.0 installer has never been built end to end and 1.0 is not code-signed yet. With Windows **Smart App Control** on, Windows refuses to start unsigned apps and offers no per-app override; see [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md). Signing is planned through SignPath Foundation, after the first unsigned beta: [Code signing policy](docs/CODE_SIGNING_POLICY.md).

## What it does
- **Sort by type**: moves loose files (top level only) into Images, Documents, Videos, Audio, Archives, Installers, Code, Other. Screenshots (`Screenshot*`, `Snip*`, `Capture*`) go to Images/Screenshots and WhatsApp pictures (`IMG-########-WA####`, `WhatsApp Image*`) to Images/Chat. Shortcuts and config files (`.lnk`, `.url`, `.ini`) are never moved. Files you touched in the last 24 hours (setting, 0 to 72, on the Sort screen) are left alone, judged by the later of modified and created time, so a fresh copy of an old file counts as new. Every file is listed and ticked; untick it, or press **Never touch** on its row to pin it. Undo supported.
- **Duplicates**: finds identical files (size, then the first and last 64 KB, then SHA-256). Keeps the oldest copy unless you choose another. A separate **Possible re-downloads** section lists `name (1).ext` / `name - Copy.ext` next to `name.ext` when the size is within 2 %; nothing there is ticked for you and the original always stays.
- **Large & old**: finds big or long-untouched files. A **Likely leftovers** group at the top (never ticked for you, and not a promise they are safe) lists zips whose every file already sits, at the same size, in a same-named folder beside them; installers (`.exe` `.msi` `.iso`) over 7 days old in your Downloads folder; and `.crdownload` / `.part` files over 3 days old.
- **Empty folders**: finds folders with no files at any depth.
- **Filter by name** on each list. Only the rows you can see count as selected, so nothing hidden is ever removed.
- **Double-click** a file or folder to see it in Explorer (it is never opened).
- **Settings**: theme (System, Light, Dark), reopen the last folder at start, the Large & old filters, and a "Never touch" list of pinned files and folders, all remembered.
- **Diagnostics**: a small log (operations, counts and error details, which can include a file name but never file contents) is kept in `%LOCALAPPDATA%\Declutter\logs`. Settings > About has **Open log folder** and **Copy diagnostics**, a text you can read before you share it. Nothing is sent anywhere.
- **Right-click a folder > Tidy with Declutter** (optional when installing) opens the app on that folder. `Declutter.exe <folder>` does the same.

![Duplicates, light theme](site/screenshot-duplicates.png)
![Large & old files, dark theme](site/screenshot-dark.png)

Removals go to the Recycle Bin, and only on local drives (USB sticks and network drives are refused, since they may have no Bin). Drive roots, your home folder, Windows, Program Files and AppData are refused. Left alone: code projects (folders with `.git`, `.svn`, `.hg`, `node_modules`, `.venv`, `venv`, `package.json`, `pyproject.toml`, `Cargo.toml` or a `.sln`), portable programs (a subfolder with an `.exe` next to a `.dll`; after a scan the app says how many it skipped and lists them), downloads in progress, files changed in the last 5 minutes, OneDrive online-only files (nothing gets downloaded), symlinks, junctions and hidden files. If one file fails (locked, too long a path), the rest still go ahead and the failures are listed.

## Run from source
```
python -m pip install -r requirements.txt
cd src
python -m declutter
```

## Develop
v0.2.2 (Python):
```
python -m pip install -r requirements-dev.txt
python -m pytest -q
```

Before a release, also run the GUI checks (opens real windows, uses temp folders only): a 10,000-file folder without freezing, cancelling a scan, both themes, 100/125/150% scaling.
```
python tests/gui_check.py
```
With Pillow installed (`pip install pillow`) it saves screenshots to `.pytest_tmp/shots` for a visual check.

### Build and test 1.0
CI is the compiler. On a PC with Smart App Control in enforcement mode, locally built unsigned Rust binaries (build scripts, test executables, the app) fail to start with error 4551, so `cargo` cannot be run there. Push a `v1*` branch instead: the **Rust** workflow runs `cargo test --workspace --exclude declutter-shell --locked` (the golden fixtures in `tests/golden/`, generated from the Python app by `tools/gen_golden.py`), clippy with `-D warnings` and a check that the generated TypeScript bindings are current; the **UI** workflow runs lint, typecheck, vitest, the build and Playwright with axe accessibility checks.

The UI toolchain is not affected and runs locally (Node):
```
cd ui
npm ci
npm test          # vitest
npm run e2e       # build, then Playwright
npm run dev       # browser preview with the mock API
```
Release builds come from `.github/workflows/release-v1.yml`, which today fires only on `v1.*` tags (a `v0.3.0` beta tag would start the Python `release.yml` instead; see `ROADMAP.md`); see [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md) and the perf targets in [`docs/PERF.md`](docs/PERF.md).

## Known limitations
- The `.exe` is unsigned, so Windows SmartScreen shows "Unknown publisher" (More info, then Run anyway).
- Antivirus may occasionally flag PyInstaller builds.
- Items sent to the Recycle Bin are restored from the Bin, not through the app's Undo.

## Support
Declutter is free and stays free. If it saved you time, you can [support the developer](https://support.advaitachandra.in). It is optional.

## License
MIT
