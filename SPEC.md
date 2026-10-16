# Declutter: Windows file and folder decluttering app

Spec and working rules for Claude Code. Read this fully before writing code.

## Goal
A modern, easy-to-use Windows desktop app that declutters a messy folder safely. Installed via a normal `Setup.exe`. Hosted on GitHub with automated builds.

## Non-negotiable safety rules
1. Every operation shows a preview. Nothing changes until the user confirms.
2. Never permanently delete. Removals go to the Recycle Bin (`send2trash`).
3. Sort moves are logged to `%LOCALAPPDATA%\Declutter\undo.json` (an old `%APPDATA%` log is migrated); "Undo last sort" must work. Moves never overwrite: name clashes get ` (1)`.
4. Refuse: drive roots, the home folder itself, Windows, Program Files, ProgramData, AppData, `.git`, `node_modules`.
5. Skip symlinks, hidden files and hidden folders.
6. A duplicate group may never have all its copies selected.
7. Never claim a feature works without testing it (see Testing).
8. Leave alone: active downloads (`.crdownload`, `.part`, `.tmp`, `.download`, `.opdownload`, `.!qB`; a `.crdownload` or `.part` untouched for 3 days counts as dead and is offered only in "Likely leftovers"), files modified in the last 5 minutes, and OneDrive online-only placeholders (never hashed or moved).
9. Project folders (containing `.git`, `node_modules`, `.venv`, `package.json` or a `.sln` file) are never entered, sorted, counted as empty or removed.
10. Removals only on local fixed drives; network and removable drives are refused (the Recycle Bin may not exist there).
11. One failing item never aborts a batch; failures are collected and listed to the user.

## Features (v1)
| Feature | Behaviour |
|---|---|
| Sort by type | Top-level loose files only (no recursion) go to Images, Documents, Videos, Audio, Archives, Installers, Code, Other. Name collisions get ` (1)`. Screenshots and chat pictures go to `Images/Screenshots` and `Images/Chat` (one level). `.lnk`, `.url`, `.ini` are never moved. Files last touched (later of modified and created time) within N hours, default 24, setting 0 to 72, are left alone. |
| Duplicates | Group by size, then first + last 64 KB, then SHA-256. `.lnk`, `.url`, `.ini` are never listed, here or as re-downloads. Oldest-modified copy is the suggested keeper. User can change the keeper per group. A separate, never-preselected "Possible re-downloads" section lists `name (1).ext` / `name - Copy.ext` siblings within 2 % size; the original always stays. |
| Large and old | Filters: min size (MB) and older than (days). Both must match; 0 disables a filter. Sorted by size, largest first. A "Likely leftovers" group (never "safe", never preselected): zips fully extracted into a same-named sibling folder, installers over 7 days old in the Downloads known folder, `.crdownload` / `.part` over 3 days old. |
| Empty folders | Folders with no files at any depth (`desktop.ini` and `Thumbs.db` don't count). Delete deepest first; the whole folder goes to the Recycle Bin. |

Out of scope for v1: folder watching, custom rules, cloud drives, drag-and-drop, auto-update.

## UX (modern and simple)
- Single window, sidebar navigation (Home, Sort, Duplicates, Large & Old, Empty Folders, Settings), follows the Windows light/dark theme.
- **Home:** quick-pick buttons for Downloads, Desktop and Documents plus "Choose folder". One **Scan** button runs all four checks and shows summary cards, e.g. "212 files to sort", "38 duplicates, 1.4 GB reclaimable", "9 large files", "14 empty folders". Clicking a card opens its tab.
- Each tab: results table with checkboxes, a "Select all / none" control, a live "Selected: N items, X GB" label, and one clear action button ("Move files", "Send to Recycle Bin").
- Confirmation dialog states the exact count and says items go to the Recycle Bin and can be restored.
- Scans run in background threads with a progress label and a Cancel button. The UI must never freeze.
- Plain-language wording, no jargon. Errors are shown in a dialog, not a stack trace.
- Style: rounded corners, generous spacing, one accent colour, a single font (Segoe UI), clear empty states ("Nothing to clean. Nice.").

## Tech
- Python 3.12, `customtkinter`, `send2trash`. Core logic is in `core.py` with no GUI imports.
- Why CustomTkinter: small, modern enough, simple to package. Trade-off: less polished than Qt. If the look is not good enough, switch the UI layer to PySide6 (the logic stays untouched, installer grows to roughly 60-100 MB).
- Packaging: PyInstaller **onedir** (not onefile). Onefile unpacks to a temp folder on every launch, starts slowly and triggers more antivirus false positives.
- Installer: **Inno Setup** wraps the onedir build into `Declutter-Setup.exe` (Start menu shortcut, optional desktop icon, uninstaller, per-user install so no admin rights are needed).
- Also publish a portable `.zip` of the onedir folder.

## Repo layout
```
declutter/
  src/declutter/{__init__,core,app}.py
  tests/test_core.py
  installer/declutter.iss
  assets/icon.ico
  .github/workflows/release.yml
  requirements.txt  requirements-dev.txt
  build.bat  README.md  LICENSE  CLAUDE.md
```

## CI and release (GitHub Actions, `windows-latest`)
On a `v*` tag: install deps, run tests, build with PyInstaller, build the installer with Inno Setup (preinstalled on the runner, or via `choco install innosetup`), upload `Declutter-Setup.exe` and the portable zip to the GitHub Release. This also solves "I can't build Windows `.exe` on a Linux machine".

## Known limitations to state in the README
- The `.exe` is unsigned, so Windows SmartScreen will show "Unknown publisher" (More info, then Run anyway). Code signing costs money; revisit if others will use it.
- Antivirus may occasionally flag PyInstaller builds. Building publicly in CI and publishing from GitHub Releases helps trust.
- Items sent to the Recycle Bin are restored from the Bin, not through the app's Undo.

## Testing
- `pytest` unit tests for `core.py` on a temp folder tree: sort plan and collisions, apply and undo round trip, duplicates (same content with different names, same size with different content), large/old filters, nested empty folders, refused locations.
- Never let tests touch real user folders.
- GUI: launch manually on Windows against a **copy** of a messy folder before any release. Check theme switching, 125% and 150% display scaling, a folder with 10,000+ files (no freeze), and cancelling a scan.

## Build order for Claude Code
1. Scaffold the repo, add `core.py` (a draft exists, untested; review it, fix issues, do not trust it blindly) and write the tests until green.
2. Build Home plus the Sort tab end to end, including undo.
3. Add Duplicates, Large & Old, Empty Folders.
4. Polish the UI (spacing, empty states, errors, icon).
5. Add `build.bat`, the Inno Setup script and the GitHub Actions workflow.
6. Write the README with screenshots and the limitations above.

## Working rules
- Smallest change that solves the problem. No speculative features.
- Do not invent library APIs; check the docs (customtkinter, send2trash, PyInstaller, Inno Setup).
- Ask before changing the safety rules.
