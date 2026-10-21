# Declutter behaviour contract (v0.2.2 -> 1.0 port)

Source of truth: `SPEC.md` safety rules 1-11 plus the observable behaviour of `src/declutter/core.py` at v0.2.2.
The Rust core must reproduce this behaviour. Golden fixtures in `tests/golden/` are generated from the Python
implementation by `tools/gen_golden.py`; the Rust tests consume the same JSON. Change this file and the fixtures
together, and only after asking about any safety rule.

## Fixture format
Each `tests/golden/<name>.json` holds `op`, `params`, a `tree` (dirs, files with `age_s`, `created_age_s`,
`attrs`, content spec) and `expected`. Paths are relative to the work root with `/` separators; `<root>` stands for
the absolute work root inside messages. `index.json` lists every fixture.

- File content: `text`, `segments` (`[[char, count], ...]`), or `size` (+ optional `fill`), or `zip` / `corrupt`.
- Times: `age_s` is seconds before "now"; `created_age_s` sets the creation time separately (default: same).
- Attributes: `hidden`, `offline` (cloud-only placeholder), applied with the Windows file attributes.
- Unordered results: group order beyond "reclaimable size, descending", equal-size ties in large/old and
  leftovers, and empty-folder ties at equal path length are unspecified in Python (they follow directory listing
  order). Fixtures avoid ties except `empty-dirs-basic` (`a` and `d`); the harness compares those as sets and checks
  the ordering rule. The Rust API should break ties by path so output is stable.
- Where Python sorts by path (sort plan, re-downloads, duplicate-group tie-break) it compares the absolute
  Windows path string: backslash separators, code-point order, case-sensitive. Rust must compare the same string.

## Rules by function (what the port must do)

**Roots (`check_root`).** Refuse: not a folder; a drive root; the home folder; any parent of home; any path part named
(case-insensitive) windows, program files, program files (x86), programdata, $recycle.bin, system volume information,
.git, node_modules, appdata; a folder directly holding a project marker (.git .svn .hg node_modules .venv venv
package.json pyproject.toml cargo.toml, or any *.sln). Reason strings are user-visible and are part of the fixture.

**Walk (`walk_files`).** One pass, directory stats only. Never follow symlinks or junctions (reparse tags
0xA000000C, 0xA0000003). Skip: dot-names; Windows hidden attribute; protected dir names; pinned paths (exact match
or inside); project folders and (below the chosen root only) portable-app folders (an .exe and a .dll side by side);
project/app folders skipped are reported. Leave alone: active downloads (.crdownload .part .tmp .download
.opdownload .!qb) except .crdownload/.part untouched for more than 3 days; files modified in the last 5 minutes;
online-only placeholders (attribute bits 0x1000, 0x40000, 0x400000). Honour cancel once per directory.

**Sort (`plan_sort`, `apply_moves`).** Only loose files directly in the root. Categories by lower-cased extension,
else Other. `.lnk .url .ini` and dead downloads (.crdownload/.part) are never moved. Images get a sub-bucket
(Screenshots: name starts with screenshot|snip|capture; Chat: IMG-########-WA#... or WhatsApp Image...), matched
case-insensitively at the start of the stem only. Name clashes (existing files and earlier planned moves, compared
case-insensitively) get ` (1)`, ` (2)`... Plan is sorted by source path. `recent_hours` (0..72, 0 = off) leaves
files whose later of modified and created time is newer; they are counted. Apply re-checks each file is still a
regular file, not left-alone, not recent, inside the root; moves never overwrite; one failure never aborts; the
undo log is written only for moves that happened.

**Undo.** Log at `%LOCALAPPDATA%\Declutter\undo.json`: a JSON list of `{"time": "YYYY-MM-DD HH:MM:SS",
"moves": [{"src", "dst"}]}`, newest last, capped at 20 batches, written atomically (temp + replace). An old
`%APPDATA%\Declutter\undo.json` is migrated once. Undo reverses the newest batch in reverse order: refuses paths
that are not absolute or contain a protected part, skips missing or linked destinations, restores to a free name
(` (1)`), then removes the now-empty category folder and its parent if that is empty too (`Images` after
`Images/Screenshots|Chat`, a preset's `Work` after `Work/2024`), never above the folder the file went back to. A damaged
entry is reported as `Skipped a damaged log entry.` A missing or non-list log is treated as empty.

**Duplicates.** Candidates: size >= `min_size` (default 1), not online-only, not `.lnk .url .ini` or dead
downloads. Group by size; then SHA-256 of first + last 64 KiB (the whole file when <= 128 KiB); then full SHA-256 in
1 MiB chunks for larger files. Within a group files sort by (modified time, path); index 0 is the suggested keeper.
Groups sort by size x (copies - 1), descending. A cancel returns no groups. A selection that contains every copy of
a group is refused (`check_dup_selection`).

**Removals (`to_recycle_bin` and the trash_* wrappers).** Everything goes to the Windows Recycle Bin; nothing is
deleted. Refused with a collected message (batch continues): outside the chosen root or the root itself; a link;
not on a local fixed drive (GetDriveTypeW == 3); a file that is a left-alone file. A missing path is silently
skipped. `trash_duplicates` removes only where an intact kept copy of the same size still exists now.
`trash_redownloads` removes a copy only if its original exists now and is not itself being removed.
`trash_empty_dirs` re-checks emptiness (junk files `desktop.ini`, `Thumbs.db` excepted) and refuses a folder that
now holds files or is a project folder; deepest first.

**Re-downloads.** `name (N).ext` or `name - Copy[ (N)].ext` beside `name.ext` in the same folder with the same
extension, both non-empty, size within 2 % of the original; excludes `.lnk .url .ini` and any path in `exclude`.
Sorted by copy path.

**Large and old.** `size >= min_mb MiB` AND `mtime older than older_days`; 0 disables a filter; dead downloads
excluded; sorted by size, descending.

**Leftovers.** `.crdownload`/`.part` older than 3 days -> "Unfinished download"; `.exe .msi .iso` older than 7 days
inside the Downloads known folder -> "Old installer"; a `.zip` whose same-named sibling folder holds every file of
the archive at its stored size (no entry may escape the folder, no links, corrupt/empty zips never match) ->
"Unpacked .zip". Sorted by size, descending. The Downloads folder comes from `SHGetKnownFolderPath`
(`{374DE290-123F-4565-9164-39C4925E467B}`) and must be injectable for tests.

**Empty folders.** Folders with no files at any depth, root excluded. `desktop.ini` and `Thumbs.db` do not count.
Hidden, protected, linked, pinned, project and app folders are never reported and count as content, so their
parents are not reported either. Sorted deepest (longest path) first.

## Not covered by fixtures (needs a Windows-only or manual test)
Symlink and junction handling (needs privilege), real Recycle Bin behaviour, real OneDrive placeholders, USB or
network drives (the drive type is injected in fixtures), per-monitor DPI, and the installer.
