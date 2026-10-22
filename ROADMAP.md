# Declutter roadmap

Keep CLAUDE.md short and stable (commands, layout, rules). This file holds the plan. Update it when items finish.

Rules for every item: tests first, core.py stays free of GUI imports, no hex colours outside theme.py, UI pause under 250 ms in tests/gui_check.py, one PR per item. (These are the rules for the Python app; the 1.0 rules are in `docs/ARCHITECTURE.md`.)

## Shipped in v0.2.x (Python app, the current release)
- **v0.2.0** (2026-10-16): themed UI, safety hardening in `core.py`, per-file Sort with undo banner, Never touch list, Likely leftovers and Possible re-downloads, Screenshots/Chat sub-folders, the first+last 64 KB duplicate prefilter, the right-click "Tidy with Declutter" entry, `.lnk`/`.url`/`.ini` never listed as duplicates.
- **v0.2.1** (2026-10-19): R1 to R5 below: trust cues, local diagnostics log, pin from the Sort row, project and portable-app boundary, UI polish.
- **v0.2.2** (2026-10-20): the 300-row list-rebuild freeze fix (R5).

## 1.0 rewrite: Rust core + Tauri 2 + React UI (branch `v1-rust`)
Behaviour is fixed by `docs/CONTRACT.md` and proven by golden fixtures generated from the Python app. The Python app stays as the oracle and the shipping product until 1.0 is released. A beta is planned as v0.3.0 (the workspace version is already 0.3.0).

| | Milestone | Status |
|---|---|---|
| M0 | Contract and golden fixtures | Done: `docs/CONTRACT.md`, `tools/gen_golden.py`, `tests/golden/`. The Rust workflow runs them with `GOLDEN_STRICT=1` (every fixture must pass). |
| M1 | Rust core (`crates/declutter-core`) | Done. Not covered by fixtures, so still unverified on real machines: symlink and junction handling, the real Recycle Bin, OneDrive placeholders, USB and network drives, per-monitor DPI (see the end of `docs/CONTRACT.md`). |
| M2 | Tauri shell and design system (`crates/declutter-app`, `src-tauri/`, `ui/`) | Done. Never run end to end: the UI has not been run inside the Tauri shell (it is tested in a browser against the mock API). |
| M3 | Feature parity with v0.2.2 | Screens done. Exit test not run: the R0 hand checks (undo after an app restart, USB and network refusal, OneDrive online-only) on a copy of a messy folder. Not ported yet: the right-click "Tidy with Declutter" entry and opening on a folder given on the command line (the installer config has no entry, the shell ignores arguments other than `--remind`), and the diagnostics log. |
| M4 | New features | Built: scan ETA and progress, "space freed" tally, "where the space goes" breakdown, CSV export, Recycle Bin size and "Open Recycle Bin", sort presets, scan-only weekly reminder, similar pictures. Follow-ups below. |
| M5 | Performance gate (`docs/PERF.md`) | Not measured. The `criterion` benches (`walk`, `dupes`) compile in CI (clippy `--all-targets`) but have not been run, and the results table in `docs/PERF.md` is empty. |
| M6 | Packaging, trust, release | Not verified. The installer has never been built end to end, the `release-v1.yml` signing and attestation jobs have never run, the first signed build is untested on a machine with Smart App Control, and the UI has never been run inside the Tauri shell. The repository has no `SIGNING_PROVIDER` variable and no `release-signing` environment yet, so a tag build would be unsigned (`docs/DISTRIBUTION.md`). |

Decisions needed before the beta:
- `release-v1.yml` fires only on `v1.*` tags and the Python `release.yml` fires on `v0.*`. A `v0.3.0` tag would build the Python app, not the beta. Either widen the 1.0 workflow's tag filter (and keep the two disjoint) or name the beta `v1.0.0-beta.1`.
- Signing: **decided, SignPath Foundation** (`docs/DISTRIBUTION.md`, policy in `docs/CODE_SIGNING_POLICY.md`; `release-v1.yml` signs through SignPath's connector when `SIGNING_PROVIDER` is `signpath`). Still blocking a signed release: SignPath requires a project already released in the form to be signed, so the first beta ships unsigned with the README notice; then apply (unknown whether a single-maintainer project is accepted); then the one-time setup (the `release-signing` environment with its secret and variables, the SignPath project, artifact configuration and signing policy). Unconfirmed until tested on a machine with Smart App Control in enforcement mode: whether Smart App Control accepts SignPath Foundation's certificate (Microsoft names Azure Artifact Signing as the preferred route, which stays the fallback). Without a signature, a machine with Smart App Control refuses to start the app.
- Website and README install text change at release (the PR that added this section lists the website claims).

### Known follow-ups (1.0)
- `bin_info` sums sizes through the `trash` crate's listing; folders in the Bin count as items but add no bytes. Use `SHQueryRecycleBinW` behind `Platform` for exact totals (`crates/declutter-app/src/lib.rs`).
- `trash_paths` and `trash_empty_dirs` pass the caller's spelling of each path on (the scan's stored spelling is used for duplicates and similar groups) and do not check that size and modified time still match what the scan offered. The core re-checks the disk (root, links, left-alone files, emptiness), but a file replaced after the scan is not detected.
- `export_csv` takes any local-drive path ending in `.csv`; `docs/API.md` says the path comes from the native save dialog, but the shell does not enforce that.
- The "space freed" tally counts a removed folder as one file (the byte count adds 0 for folders), so "N files" is files plus folders.
- The `trash` crate deletes with `FOF_ALLOWUNDO | FOF_WANTNUKEWARNING` and no UI, and the code relies on it returning an error instead of destroying an item that cannot go to the Bin. That behaviour on a fixed drive with no Recycle Bin (for example Bin disabled for the drive) is assumed from the crate, not tested.
- The similar-pictures side-by-side preview is a placeholder: the screen says "Side-by-side previews are coming in a later version" and points to Explorer.
- Align versions: the Rust workspace is 0.3.0, `ui/package.json` says 1.0.0.

## R0. Blockers (no new features until these are done)
- [x] Merge #3 to #8 in order; retarget #8 to main after #7 merges.
- [ ] Run the three hand checks on a COPY of a messy folder: undo after a real app restart; removal refused on a USB stick and on a network share; OneDrive online-only files not downloaded. (Only the owner can do these.)
- [ ] Look at the new Leftovers and Re-downloads screens in both themes. `tests/gui_check.py` now writes screenshots to `.pytest_tmp/shots`, but nobody has looked at them yet.
- [x] Duplicates must not list .lnk, .url or .ini files (identical config files in different folders are not redundant). Test: a folder with two identical a.ini files and two identical b.txt files lists only b.txt.
- [x] Tag v0.2.0 (done) and v0.2.1 (see Releases).
- [ ] Install the exact v0.2.1 Release `Declutter-Setup.exe` on a clean Windows machine (Windows Sandbox or a VM). Not a local build. Nobody has launched the released files yet.

## R1. Trust cues (cheap, high value) - shipped in v0.2.1
- "Why flagged" pill next to each row in Large & old and Duplicates: Unpacked .zip (show the matching folder name), Old installer, Unfinished download, Numbered copy. For re-downloads show the real size difference ("2.0 MB vs 2.1 MB"), not a rounded percentage.
- Live "Selected to reclaim: X" next to the main action button in Duplicates and Large & old.
- core.peek_undo() returns folder, file count and age of the last sort. The Undo confirmation says: "Undo the sort of 14 files in Downloads (5 minutes ago)?" Test peek_undo headless.
- UI copy never says "safe to delete". Say "likely leftover".

## R2. Diagnostics (needed before real users) - shipped in v0.2.1
- Rotating log at %LOCALAPPDATA%\Declutter\logs\declutter.log: operation type, counts, and full tracebacks. No file contents. Nothing is sent anywhere.
- Settings > About: "Open log folder" and "Copy diagnostics" (app version, Windows version, last 200 log lines). The user reads it before sharing.
- Unhandled exceptions in the Tk callback and in worker threads are logged and shown in the themed dialog.
- Why: the app currently has no logging, so a crash on a friend's PC cannot be diagnosed.

## R3. Sort preview audit, then gaps only - shipped in v0.2.1
- Audit first: can the user already see every file that will move and untick or pin one from its row? M3 reported per-file sort as working, so do not rebuild it.
  - Audit result: every file that will move is listed under its destination folder, ticked, with Select all/none and a filter; at most 300 rows are shown and only shown rows move. Missing: pin from the row.
- Build only what is missing: pin ("Never touch") from the file row, then re-plan without rescanning.
  - Done: a "Never touch" button on each row greys that row out in place ("Pinned", unticked, not counted) and drops the file from the plan (no walk of the folder, no relayout: 22 ms at 300 rows; removing the row cost 225 ms). Other lists update on the next scan; their actions skip pinned files anyway.

## R4. Project and portable-app boundary (delta only) - shipped in v0.2.1
Decision made while building: the .exe+.dll rule applies to subfolders only. The folder the user picks is never refused for it (Downloads can hold a loose .exe and .dll).
Already in core.py: .git, node_modules, .venv, package.json, *.sln. Add: .svn, .hg, venv, pyproject.toml, Cargo.toml.
- Also treat a folder containing both an .exe and a .dll as a portable app: do not enter it. Failure direction is safe (less is offered), but it hides cleanup silently, so:
  - show "Skipped N app/project folders to keep them intact" with a "Show them" link;
  - tests: a project folder, a portable-app folder, and a plain folder with only an .exe.

## R5. UI backlog - shipped in v0.2.1
- Rebuilding a 300-row list blocked the window (about 0.7 s in `set_moves` on the Sort tab, after a sort, a rescan or a grace-period change; the cost is destroying the old widgets).
  - Fixed: the rows live in their own frame. A rebuild hides the old frame at once and destroys its widgets about 12 ms at a time; the new rows are built after that. Lists of 60 widgets or fewer are destroyed at once as before. Worst single event on a rebuild was about 215 ms in a hand measurement; `tests/gui_check.py` asserts under 250 ms.
  - Rules this depends on: destroy whole CTk widgets only (destroying a button's inner canvas on its own raises "invalid command name"); pause the clean-up while a background job runs (a worker's garbage collection must not free Tk variables, see `_run`).
  - Cost: after a rescan of a big list the new rows appear about a second later than before.
- Toast must not overlap the primary action button (anchor it left/centre of the status bar).
  - Done: it sits at the bottom left above the footer row; gui_check proves it clears the action button, the selection label and the status text on every page at 100/125/150% and at both window sizes.
- One themed alert() dialog replacing the remaining native messageboxes.
  - Done: the only native box left is the fallback if the themed dialog itself fails (a unit test pins this).
- Every new transition respects the Windows animation setting; focus rings at 100/125/150%.
  - Done: the setting is read each time (it was read once at start); focus rings are checked at all three scales in both themes.
- The list rebuild freeze: fixed after v0.2.1 (see the first item above); ships in the next release.

## Later, only if real users ask
- Year-Month sub-folders (opt-in). Risk: moves the mess one level deeper and adds undo cases.
- Code signing for v0.x (costs money; test first whether the SmartScreen warning actually stops your testers). For 1.0 it is a release gate, see `docs/DISTRIBUTION.md`.
- Folder watching, auto-update, .rar/.7z extraction check.
