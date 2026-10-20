# UI <-> shell API (Tauri 2 commands and events)

The UI never touches the disk. It calls these commands (`invoke`) and listens to a scan's event channel. Types are the
DTOs in `crates/declutter-core/src/model.rs`, generated to `ui/src/api/generated/*.ts` (never hand-edit). All commands
are async and return `Result<T, String>`; the string is plain-language and safe to show. The UI wraps `invoke` in
`ui/src/api/client.ts` behind an `Api` interface, with a mock implementation (`mock.ts`) for tests and the browser
preview; the real one is selected when `window.__TAURI_INTERNALS__` exists.

## Commands
| Command | Args | Returns | Notes |
|---|---|---|---|
| `app_info` | | `AppInfo` | version, data dir |
| `quick_folders` | | `QuickFolders` | shell-resolved Downloads/Desktop/Documents |
| `pick_folder` | | `string \| null` | native folder dialog |
| `check_folder` | `path` | `FolderCheck` | same rules and reasons as `check_root` |
| `start_scan` | `path`, `options: ScanOptions`, `onEvent: Channel<ScanEvent>` | `number` (scan id) | returns at once; work runs on a worker thread |
| `cancel_scan` | `id` | | partial results already sent stay valid; `Done{cancelled:true}` follows |
| `apply_sort` | `root`, `moves: Move[]`, `recentHours` | `ActionResult` | writes the undo log |
| `trash_paths` | `root`, `paths: string[]` | `ActionResult` | Large & Old, Leftovers |
| `trash_duplicates` | `root`, `removals: DupRemoval[]` | `ActionResult` | |
| `trash_redownloads` | `root`, `pairs: Redownload[]`, `removing: string[]` | `ActionResult` | |
| `trash_similar` | `root`, `removals: SimilarRemoval[]` | `ActionResult` | keeps at least one per group |
| `trash_empty_dirs` | `root`, `paths: string[]` | `ActionResult` | |
| `peek_undo` | | `UndoInfo \| null` | |
| `undo_last_sort` | | `ActionResult` | |
| `get_settings` / `save_settings` | `Settings` | `Settings` | |
| `add_pin` / `remove_pin` | `path` | `Settings` | Never Touch list |
| `list_presets` | | `Preset[]` | built-ins first |
| `save_preset` / `delete_preset` | `Preset` / `id` | `Preset[]` | validated server-side |
| `export_csv` | `path`, `payload: ExportPayload` | | `path` comes from the native save dialog |
| `space_freed` / `reset_space_freed` | | `SpaceFreed` | local tally, Recycle Bin removals only |
| `bin_info` | | `BinInfo` | size of the Recycle Bin |
| `open_recycle_bin` | | | opens Explorer's Recycle Bin |
| `reveal_in_explorer` | `path` | | `explorer /select,` only; never executes the file |
| `reminder_status` | | `ReminderStatus` | |
| `reminder_enable` / `reminder_disable` | `schedule` / | `ReminderStatus` | scan-only task, off by default |

Every action command that sends something to the Recycle Bin calls `settings::record_freed` for what actually went.

## Scan events (`ScanEvent`, in this order, each phase as soon as it finishes)
`progress` (many) -> `skipped` -> `sort` -> `largeOld` -> `leftovers` -> `breakdown` -> `emptyDirs` -> `redownloads`
(after `duplicates` so exact copies are excluded) -> `duplicates` -> `similar` (only if requested) -> `done`.
`progress.total` is null while walking, then a number so the bar becomes determinate; `etaSecs` appears once hashing
has a rate. `error` ends a scan that cannot continue (for example a refused folder). The UI must render each
result as it arrives and stay responsive (no frame over 50 ms).

## Security rules for the shell
- Validate every path with `require_root` and confirm it lies inside the scanned root before acting; re-check the disk
  inside the core (already required by the contract), never trust the UI's copy of a result.
- Minimal Tauri capabilities (dialog open/save, notification, window effects, shell none). Strict CSP, no remote content,
  no `eval`, no `dangerouslySetInnerHTML`. `reveal_in_explorer` and `open_recycle_bin` use fixed arguments.
- No telemetry, no network access of any kind in the app.
