# Declutter roadmap (after PR #8)

Keep CLAUDE.md short and stable (commands, layout, rules). This file holds the plan. Update it when items finish.

Rules for every item: tests first, core.py stays free of GUI imports, no hex colours outside theme.py, UI pause under 250 ms in tests/gui_check.py, one PR per item.

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
- Code signing (costs money; test first whether the SmartScreen warning actually stops your testers).
- Folder watching, auto-update, .rar/.7z extraction check.
