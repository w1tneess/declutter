//! Declutter's Tauri 2 shell. Owner: shell. Thin command wrappers over `declutter_app` (docs/API.md), the window
//! (Mica on Windows 11, a solid theme colour otherwise), single-instance, dialogs and the `--remind` notification.
//! No logic lives here: validation and safety checks are in `declutter_app` and the core, where CI tests them.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Duration;

use declutter_app::{App, Core};
use declutter_core::model::{
    ActionResult, AppInfo, BinInfo, DupRemoval, ExportPayload, FolderCheck, Move, Preset,
    QuickFolders, Redownload, ReminderStatus, ScanEvent, ScanOptions, Settings, SimilarRemoval,
    SpaceFreed, UndoInfo,
};
use declutter_core::platform::SystemPlatform;
use tauri::ipc::Channel;
use tauri::window::Color;
use tauri::{AppHandle, Manager, State, Theme, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

type AppState<'a> = State<'a, Arc<App>>;
type Res<T> = Result<T, String>;

/// Runs app work on the blocking pool: disk walks and Recycle Bin calls must not stall the async runtime.
async fn blocking<T: Send + 'static>(
    app: AppState<'_>,
    f: impl FnOnce(&App) -> Res<T> + Send + 'static,
) -> Res<T> {
    let app = app.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|_| "Something went wrong. Please try again.".to_string())?
}

#[tauri::command]
async fn app_info(app: AppState<'_>) -> Res<AppInfo> {
    Ok(app.app_info())
}

#[tauri::command]
async fn quick_folders(app: AppState<'_>) -> Res<QuickFolders> {
    blocking(app, |a| Ok(a.quick_folders())).await
}

#[tauri::command]
async fn pick_folder(window: WebviewWindow) -> Res<Option<String>> {
    let picked = window
        .dialog()
        .file()
        .set_parent(&window)
        .blocking_pick_folder();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn check_folder(app: AppState<'_>, path: String) -> Res<FolderCheck> {
    blocking(app, move |a| Ok(a.check_folder(&path))).await
}

#[tauri::command]
async fn start_scan(
    app: AppState<'_>,
    path: String,
    options: ScanOptions,
    on_event: Channel<ScanEvent>,
) -> Res<u64> {
    // A closed window drops the channel; the scan then just finishes unheard.
    let emit = Arc::new(move |ev: ScanEvent| {
        let _ = on_event.send(ev);
    });
    Ok(app.start_scan(path, options, emit))
}

#[tauri::command]
async fn cancel_scan(app: AppState<'_>, id: u64) -> Res<()> {
    app.cancel_scan(id);
    Ok(())
}

#[tauri::command]
async fn apply_sort(
    app: AppState<'_>,
    root: String,
    moves: Vec<Move>,
    recent_hours: u32,
) -> Res<ActionResult> {
    blocking(app, move |a| a.apply_sort(&root, &moves, recent_hours)).await
}

#[tauri::command]
async fn trash_paths(app: AppState<'_>, root: String, paths: Vec<String>) -> Res<ActionResult> {
    blocking(app, move |a| a.trash_paths(&root, &paths)).await
}

#[tauri::command]
async fn trash_duplicates(
    app: AppState<'_>,
    root: String,
    removals: Vec<DupRemoval>,
) -> Res<ActionResult> {
    blocking(app, move |a| a.trash_duplicates(&root, &removals)).await
}

#[tauri::command]
async fn trash_redownloads(
    app: AppState<'_>,
    root: String,
    pairs: Vec<Redownload>,
    removing: Vec<String>,
) -> Res<ActionResult> {
    blocking(app, move |a| a.trash_redownloads(&root, &pairs, &removing)).await
}

#[tauri::command]
async fn trash_similar(
    app: AppState<'_>,
    root: String,
    removals: Vec<SimilarRemoval>,
) -> Res<ActionResult> {
    blocking(app, move |a| a.trash_similar(&root, &removals)).await
}

#[tauri::command]
async fn trash_empty_dirs(
    app: AppState<'_>,
    root: String,
    paths: Vec<String>,
) -> Res<ActionResult> {
    blocking(app, move |a| a.trash_empty_dirs(&root, &paths)).await
}

#[tauri::command]
async fn peek_undo(app: AppState<'_>) -> Res<Option<UndoInfo>> {
    blocking(app, |a| Ok(a.peek_undo())).await
}

#[tauri::command]
async fn undo_last_sort(app: AppState<'_>) -> Res<ActionResult> {
    blocking(app, |a| Ok(a.undo_last_sort())).await
}

#[tauri::command]
async fn get_settings(app: AppState<'_>) -> Res<Settings> {
    blocking(app, |a| Ok(a.get_settings())).await
}

#[tauri::command]
async fn save_settings(app: AppState<'_>, settings: Settings) -> Res<Settings> {
    blocking(app, move |a| a.save_settings(&settings)).await
}

#[tauri::command]
async fn add_pin(app: AppState<'_>, path: String) -> Res<Settings> {
    blocking(app, move |a| a.add_pin(&path)).await
}

#[tauri::command]
async fn remove_pin(app: AppState<'_>, path: String) -> Res<Settings> {
    blocking(app, move |a| a.remove_pin(&path)).await
}

#[tauri::command]
async fn list_presets(app: AppState<'_>) -> Res<Vec<Preset>> {
    blocking(app, |a| Ok(a.list_presets())).await
}

#[tauri::command]
async fn save_preset(app: AppState<'_>, preset: Preset) -> Res<Vec<Preset>> {
    blocking(app, move |a| a.save_preset(&preset)).await
}

#[tauri::command]
async fn delete_preset(app: AppState<'_>, id: String) -> Res<Vec<Preset>> {
    blocking(app, move |a| a.delete_preset(&id)).await
}

#[tauri::command]
async fn export_csv(app: AppState<'_>, path: String, payload: ExportPayload) -> Res<()> {
    blocking(app, move |a| a.export_csv(&path, &payload)).await
}

#[tauri::command]
async fn space_freed(app: AppState<'_>) -> Res<SpaceFreed> {
    blocking(app, |a| Ok(a.space_freed())).await
}

#[tauri::command]
async fn reset_space_freed(app: AppState<'_>) -> Res<SpaceFreed> {
    blocking(app, |a| a.reset_space_freed()).await
}

#[tauri::command]
async fn bin_info(app: AppState<'_>) -> Res<BinInfo> {
    blocking(app, |a| a.bin_info()).await
}

#[tauri::command]
async fn open_recycle_bin(app: AppState<'_>) -> Res<()> {
    app.open_recycle_bin()
}

#[tauri::command]
async fn reveal_in_explorer(app: AppState<'_>, path: String) -> Res<()> {
    app.reveal_in_explorer(&path)
}

#[tauri::command]
async fn reminder_status(app: AppState<'_>) -> Res<ReminderStatus> {
    blocking(app, |a| Ok(a.reminder_status())).await
}

#[tauri::command]
async fn reminder_enable(app: AppState<'_>) -> Res<ReminderStatus> {
    blocking(app, |a| a.reminder_enable()).await
}

#[tauri::command]
async fn reminder_disable(app: AppState<'_>) -> Res<ReminderStatus> {
    blocking(app, |a| a.reminder_disable()).await
}

/// Without Mica (Windows 10) the transparent window needs a solid colour that matches the theme.
fn solid_background(window: &WebviewWindow, theme: Theme) {
    let color = if theme == Theme::Dark {
        Color(32, 32, 32, 255)
    } else {
        Color(243, 243, 243, 255)
    };
    let _ = window.set_background_color(Some(color));
}

/// The window is declared with `create: false` so `--remind` can run without one.
fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .first()
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;
    let window = WebviewWindowBuilder::from_config(app, &config)?.build()?;
    // `None` follows the system light/dark setting, like the rest of the window.
    if window_vibrancy::apply_mica(&window, None).is_err() {
        solid_background(&window, window.theme().unwrap_or(Theme::Light));
        let w = window.clone();
        window.on_window_event(move |e| {
            if let WindowEvent::ThemeChanged(theme) = e {
                solid_background(&w, *theme);
            }
        });
    }
    Ok(())
}

/// A second launch brings the open window forward instead of starting another copy.
fn focus_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// `--remind` (started by the scheduled task): no window, one notification if files are waiting, then exit.
fn remind(handle: AppHandle, app: Arc<App>) {
    std::thread::spawn(move || {
        // A panic in the scan must still reach `exit`, or a windowless process lingers.
        if let Ok(Ok(Some(text))) = catch_unwind(AssertUnwindSafe(|| app.remind())) {
            let _ = handle
                .notification()
                .builder()
                .title("Declutter")
                .body(text)
                .show();
            // shortcut: `show` hands the toast to the async runtime, so wait a moment before exiting; switch to a
            // completion signal if the plugin ever offers one.
            std::thread::sleep(Duration::from_secs(3));
        }
        handle.exit(0);
    });
}

fn main() {
    let remind_mode = std::env::args().skip(1).any(|a| a == "--remind");
    let app = App::new(Arc::new(SystemPlatform), Arc::new(Core));
    let mut builder = tauri::Builder::default();
    if !remind_mode {
        // Registered first, as the plugin requires. The reminder may run beside an open window, so it skips this.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app)
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app.clone())
        .invoke_handler(tauri::generate_handler![
            app_info,
            quick_folders,
            pick_folder,
            check_folder,
            start_scan,
            cancel_scan,
            apply_sort,
            trash_paths,
            trash_duplicates,
            trash_redownloads,
            trash_similar,
            trash_empty_dirs,
            peek_undo,
            undo_last_sort,
            get_settings,
            save_settings,
            add_pin,
            remove_pin,
            list_presets,
            save_preset,
            delete_preset,
            export_csv,
            space_freed,
            reset_space_freed,
            bin_info,
            open_recycle_bin,
            reveal_in_explorer,
            reminder_status,
            reminder_enable,
            reminder_disable,
        ])
        .setup(move |tauri_app| {
            if remind_mode {
                remind(tauri_app.handle().clone(), app);
            } else {
                open_main_window(tauri_app.handle())?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Declutter could not start the window system");
}
