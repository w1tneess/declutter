# Declutter roadmap (after PR #8)

Keep CLAUDE.md short and stable (commands, layout, rules). This file holds the plan. Update it when items finish.

Rules for every item: tests first, core.py stays free of GUI imports, no hex colours outside theme.py, UI pause under 250 ms in tests/gui_check.py, one PR per item.

## R0. Blockers (no new features until these are done)
- [ ] Merge #3 to #8 in order; retarget #8 to main after #7 merges.
- [ ] Run the three hand checks on a COPY of a messy folder: undo after a real app restart; removal refused on a USB stick and on a network share; OneDrive online-only files not downloaded.
- [ ] Look at the new Leftovers and Re-downloads screens in both themes (the last automated screenshots were black).
- [ ] Duplicates must not list .lnk, .url or .ini files (identical config files in different folders are not redundant). Test: a folder with two identical a.ini files and two identical b.txt files lists only b.txt.
- [ ] Tag v0.2.0, then install that exact Release file on a clean Windows machine (Windows Sandbox or a VM). Not a local build.

## R1. Trust cues (cheap, high value) - built, awaiting review
- "Why flagged" pill next to each row in Large & old and Duplicates: Unpacked .zip (show the matching folder name), Old installer, Unfinished download, Numbered copy. For re-downloads show the real size difference ("2.0 MB vs 2.1 MB"), not a rounded percentage.
- Live "Selected to reclaim: X" next to the main action button in Duplicates and Large & old.
- core.peek_undo() returns folder, file count and age of the last sort. The Undo confirmation says: "Undo the sort of 14 files in Downloads (5 minutes ago)?" Test peek_undo headless.
- UI copy never says "safe to delete". Say "likely leftover".

## R2. Diagnostics (needed before real users)
- Rotating log at %LOCALAPPDATA%\Declutter\logs\declutter.log: operation type, counts, and full tracebacks. No file contents. Nothing is sent anywhere.
- Settings > About: "Open log folder" and "Copy diagnostics" (app version, Windows version, last 200 log lines). The user reads it before sharing.
- Unhandled exceptions in the Tk callback and in worker threads are logged and shown in the themed dialog.
- Why: the app currently has no logging, so a crash on a friend's PC cannot be diagnosed.

## R3. Sort preview audit, then gaps only - built, awaiting review
- Audit first: can the user already see every file that will move and untick or pin one from its row? M3 reported per-file sort as working, so do not rebuild it.
  - Audit result: every file that will move is listed under its destination folder, ticked, with Select all/none and a filter; at most 300 rows are shown and only shown rows move. Missing: pin from the row.
- Build only what is missing: pin ("Never touch") from the file row, then re-plan without rescanning.
  - Done: a "Never touch" button on each row removes that row in place and keeps the rest of the plan (no walk of the folder). Other lists update on the next scan; their actions skip pinned files anyway.

## R4. Project and portable-app boundary (delta only)
Already in core.py: .git, node_modules, .venv, package.json, *.sln. Add: .svn, .hg, venv, pyproject.toml, Cargo.toml.
- Also treat a folder containing both an .exe and a .dll as a portable app: do not enter it. Failure direction is safe (less is offered), but it hides cleanup silently, so:
  - show "Skipped N app/project folders to keep them intact" with a "Show them" link;
  - tests: a project folder, a portable-app folder, and a plain folder with only an .exe.

## R5. UI backlog
- Rebuilding a 300-row list blocks the window (measured: about 0.4 s in the real event loop on main, Sort tab, after a sort, a rescan or a grace-period change). CustomTkinter makes and destroys about three widgets per row. Tried chunked destroying: worse (about 1 s). Options: fewer widgets per row, a lower row cap, or a canvas-drawn list.
- Toast must not overlap the primary action button (anchor it left/centre of the status bar).
- One themed alert() dialog replacing the remaining native messageboxes.
- Every new transition respects the Windows animation setting; focus rings at 100/125/150%.

## Later, only if real users ask
- Year-Month sub-folders (opt-in). Risk: moves the mess one level deeper and adds undo cases.
- Code signing (costs money; test first whether the SmartScreen warning actually stops your testers).
- Folder watching, auto-update, .rar/.7z extraction check.
