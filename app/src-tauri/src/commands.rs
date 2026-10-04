use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::dictation::{now_ms, Msg};
use crate::engine::{self, ModelStatus};
use crate::history::Entry;
use crate::i18n;
use crate::paste::DisplayServer;
use crate::permissions::{self, Permissions};
use crate::settings::{default_models_dir, Settings, Speech, UiLanguage};
use crate::sounds::{self, SoundSet};
use crate::{audio, backup, hotkey, overlay, transcribe, translator, updates, voicecheck, Shared};

type Result<T> = std::result::Result<T, String>;

/// Written once the first-run window has been closed.
pub const ONBOARDED: &str = "onboarded";

/// Named .txt in the packages: every system has something to open that with.
const NOTICES: &str = "THIRD_PARTY_NOTICES.txt";

fn message(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
pub fn get_settings(shared: State<Shared>) -> Settings {
    shared.settings()
}

/// Applies and stores new settings. The hotkey goes first: if it cannot be registered
/// nothing is saved and the window shows why.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<()> {
    apply(&app, settings).map_err(message)
}

/// Opened from the disk image, or from Downloads through App Translocation, the app runs
/// from a path that is gone after a restart: a login item there would never start.
pub fn temporary_location() -> bool {
    cfg!(target_os = "macos")
        && std::env::current_exe().is_ok_and(|exe| {
            let path = exe.to_string_lossy();
            path.contains("/AppTranslocation/") || path.starts_with("/Volumes/")
        })
}

pub fn apply(app: &AppHandle, settings: Settings) -> anyhow::Result<()> {
    let shared = app.state::<Shared>();
    let settings = settings.sanitized();
    hotkey::register(
        app,
        &settings.hotkey,
        &settings.translate_hotkey,
        &settings.translator_hotkey,
        settings.mode,
    )?;
    *shared.hotkey_error.lock().unwrap() = None;

    if settings.autostart && !shared.settings().autostart && temporary_location() {
        anyhow::bail!(
            "move Boltay to Applications first: from where it runs now it would not start at login"
        );
    }
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) != settings.autostart {
        let changed = if settings.autostart {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        changed.map_err(|e| anyhow::anyhow!("autostart: {e}"))?;
    }

    settings.save(&shared.paths.settings)?;
    let previous = shared.replace_settings(settings.clone());
    if previous.speech != settings.speech
        || previous.models_dir != settings.models_dir
        || previous.graph_cache != settings.graph_cache
    {
        // Off this thread: unloading waits for a model still loading, and with the graph
        // cache the first load takes long enough for the window to stop responding.
        let app = app.clone();
        std::thread::spawn(move || {
            app.state::<Shared>().engines.unload();
            crate::preload(&app);
        });
    }
    if settings.check_updates && !previous.check_updates {
        let app = app.clone();
        std::thread::spawn(move || crate::updates::check(&app));
    }
    shared.send(Msg::Reload);
    crate::refresh_tray(app);
    Ok(())
}

#[tauri::command]
pub fn ui_language(shared: State<Shared>) -> UiLanguage {
    i18n::resolve(shared.settings().ui_language)
}

#[tauri::command]
pub fn suspend_hotkey(app: AppHandle, suspended: bool) {
    hotkey::suspend(&app, suspended);
}

#[tauri::command]
pub fn hotkey_error(shared: State<Shared>) -> Option<String> {
    shared.hotkey_error.lock().unwrap().clone()
}

#[tauri::command]
pub fn microphones() -> Vec<String> {
    audio::microphones()
}

#[derive(Serialize)]
pub struct Models {
    dir: PathBuf,
    default_dir: PathBuf,
    items: Vec<ModelStatus>,
    downloading: Vec<&'static str>,
    /// The model for the chosen language is in memory.
    loaded: bool,
    loading: bool,
}

#[tauri::command]
pub fn models(shared: State<Shared>) -> Models {
    let settings = shared.settings();
    let dir = shared.models_dir(&settings);
    Models {
        items: engine::status(&dir),
        default_dir: default_models_dir(&shared.paths.data),
        downloading: shared.downloads.active(),
        loaded: shared.engines.is_loaded(settings.speech, &dir),
        loading: shared.engines.is_loading(),
        dir,
    }
}

#[tauri::command]
pub fn download_model(app: AppHandle, id: String) -> Result<()> {
    crate::downloads::start(&app, &id).map_err(message)
}

#[tauri::command]
pub fn cancel_download(shared: State<Shared>, id: String) {
    shared.downloads.cancel(&id);
}

#[tauri::command]
pub fn delete_model(app: AppHandle, shared: State<Shared>, id: String) -> Result<()> {
    let model = boltay_core::Model::by_id(&id).ok_or_else(|| format!("unknown model {id}"))?;
    shared.downloads.cancel(model.id);
    // A loaded model keeps its files mapped on Windows; let it go first.
    shared.engines.unload();
    engine::delete(&shared.models_dir(&shared.settings()), model).map_err(message)?;
    crate::refresh_status(&app);
    crate::preload(&app);
    Ok(())
}

#[tauri::command]
pub fn history(shared: State<Shared>) -> Vec<Entry> {
    shared.history.load()
}

#[tauri::command]
pub fn delete_history(app: AppHandle, shared: State<Shared>, at: u64) -> Result<()> {
    shared.history.delete(at).map_err(message)?;
    crate::history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, shared: State<Shared>) -> Result<()> {
    shared.history.clear().map_err(message)?;
    crate::history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn copy_text(shared: State<Shared>, text: String) -> Result<()> {
    shared.paster.copy(&text).map_err(message)
}

/// The copy button on the overlay: copies and gets out of the way.
#[tauri::command]
pub fn copy_and_dismiss(app: AppHandle, shared: State<Shared>, text: String) -> Result<()> {
    shared.paster.copy(&text).map_err(message)?;
    overlay::hide(&app);
    Ok(())
}

#[tauri::command]
pub fn overlay_state() -> Option<overlay::State> {
    overlay::current()
}

/// Frees the overlay a minute later unless a dictation reuses it: after "Details" nothing
/// else would, as that stops the timer that normally does.
#[tauri::command]
pub fn dismiss_overlay(app: AppHandle) {
    std::thread::spawn(move || crate::dictation::hide_after(&app, std::time::Duration::ZERO));
}

/// "Details" keeps the overlay up until it is closed.
#[tauri::command]
pub fn expand_overlay(app: AppHandle, shared: State<Shared>) {
    shared.claim_overlay();
    overlay::expand(&app);
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    os: &'static str,
    display: DisplayServer,
    soft_fillers: &'static [&'static str],
    /// For "Restore the built-in list".
    default_brands: Vec<boltay_text::Term>,
    config_dir: PathBuf,
    data_dir: PathBuf,
    first_run: bool,
}

#[tauri::command]
pub fn app_info(shared: State<Shared>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        display: shared.paster.display(),
        soft_fillers: boltay_text::SOFT_FILLERS,
        default_brands: boltay_text::default_brands(),
        config_dir: shared
            .paths
            .settings
            .parent()
            .map(Into::into)
            .unwrap_or_default(),
        data_dir: shared.paths.data.clone(),
        first_run: !shared.paths.data.join(ONBOARDED).exists(),
    }
}

#[tauri::command]
pub fn permissions() -> Permissions {
    permissions::check()
}

#[tauri::command]
pub fn request_microphone() {
    permissions::request_microphone();
}

#[tauri::command]
pub fn request_accessibility() {
    permissions::request_accessibility();
}

#[tauri::command]
pub fn open_permission_settings(app: AppHandle, kind: String) -> Result<()> {
    let url = match kind.as_str() {
        "microphone" => permissions::MICROPHONE_SETTINGS,
        "accessibility" => permissions::ACCESSIBILITY_SETTINGS,
        _ => return Err(format!("unknown permission {kind}")),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Only https: the About section links to the site and the model licenses.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<()> {
    if !url.starts_with("https://") {
        return Err("only https links".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_folder(app: AppHandle, path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_logs(app: AppHandle) -> Result<()> {
    crate::open_logs(&app).map_err(message)
}

/// The licenses of everything inside the app, the copy that ships with it.
#[tauri::command]
pub fn open_notices(app: AppHandle) -> Result<()> {
    let dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.join(NOTICES).to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Lets the settings window try the text pipeline on typed text.
#[tauri::command]
pub fn preview_text(shared: State<Shared>, config: boltay_text::Config, text: String) -> String {
    // As in dictation: the brand lists fix how the Russian model spells names, in other
    // languages they would only catch plain words.
    let config = if shared.settings().speech == Speech::Other {
        boltay_text::Config {
            brands: boltay_text::Terms::default(),
            ambiguous: boltay_text::Terms::default(),
            ..config
        }
    } else {
        config
    };
    boltay_text::process(&config, &text)
}

#[tauri::command]
pub fn close_onboarding(app: AppHandle, shared: State<Shared>) -> Result<()> {
    let marker = shared.paths.data.join(ONBOARDED);
    std::fs::create_dir_all(&shared.paths.data).map_err(|e| e.to_string())?;
    std::fs::write(marker, now_ms().to_string()).map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn preview_sound(set: SoundSet) {
    sounds::preview(set);
}

/// The level meter in the settings window: on while it is shown.
#[tauri::command]
pub fn mic_monitor(app: AppHandle, shared: State<Shared>, on: bool) {
    if on {
        shared.mic.start(&app);
    } else {
        shared.mic.stop();
    }
}

/// The recording level Windows sets for the chosen microphone, in percent.
#[tauri::command]
pub fn mic_volume(shared: State<Shared>) -> Option<u32> {
    #[cfg(windows)]
    return crate::mic::volume::get(shared.settings().microphone)
        .map(|level| (level * 100.0).round() as u32);
    #[cfg(not(windows))]
    {
        let _ = shared;
        None
    }
}

/// Raises a microphone left too low in the Windows sound settings.
#[tauri::command]
pub fn raise_mic_volume(shared: State<Shared>) -> Result<()> {
    #[cfg(windows)]
    return crate::mic::volume::set(shared.settings().microphone, 0.9).map_err(|e| e.to_string());
    #[cfg(not(windows))]
    {
        let _ = shared;
        Err("only on Windows".into())
    }
}

#[tauri::command]
pub fn update_status(shared: State<Shared>) -> updates::Status {
    shared.updates.status()
}

#[tauri::command]
pub async fn check_updates_now(app: AppHandle) -> updates::Status {
    updates::check(&app)
}

/// Saves the settings to a file the user picks. `None` if the dialog was cancelled.
#[tauri::command]
pub async fn export_settings(app: AppHandle) -> Result<Option<String>> {
    let text = backup::export(&app.state::<Shared>().settings()).map_err(message)?;
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name("boltay-settings.json")
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// Reads a backup the user picks and says what it would change. Nothing is applied yet.
#[tauri::command]
pub async fn import_settings(app: AppHandle) -> Result<Option<backup::Preview>> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let shared = app.state::<Shared>();
    let current = shared.settings();
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let loaded = backup::import(&text, &current).map_err(message)?;
    let preview = backup::preview(&path, &current, &loaded).map_err(message)?;
    *shared.pending_import.lock().unwrap() = Some(loaded);
    Ok(Some(preview))
}

#[tauri::command]
pub fn apply_import(app: AppHandle, shared: State<Shared>) -> Result<()> {
    let settings = shared
        .pending_import
        .lock()
        .unwrap()
        .take()
        .ok_or("nothing to apply")?;
    apply(&app, settings).map_err(message)
}

#[tauri::command]
pub fn cancel_import(shared: State<Shared>) {
    shared.pending_import.lock().unwrap().take();
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, shared: State<Shared>) -> Result<()> {
    apply(&app, backup::reset(&shared.settings())).map_err(message)
}

#[tauri::command]
pub fn voice_check_start(app: AppHandle) -> Result<()> {
    voicecheck::start(&app).map_err(message)
}

#[tauri::command]
pub async fn voice_check_stop(app: AppHandle) -> Result<voicecheck::Heard> {
    voicecheck::stop(&app).map_err(message)
}

#[tauri::command]
pub fn voice_check_cancel(app: AppHandle) {
    voicecheck::cancel(&app);
}

#[tauri::command]
pub fn translator_opened() -> Option<translator::Opened> {
    translator::opened()
}

/// Translation takes from a tenth of a second to a few seconds with a cold model: off the
/// main thread, so the windows stay responsive.
#[tauri::command]
pub async fn translate_text(
    app: AppHandle,
    text: String,
    direction: Option<boltay_core::Direction>,
) -> Result<translator::Outcome> {
    tauri::async_runtime::spawn_blocking(move || translator::translate(&app, &text, direction))
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)
}

#[tauri::command]
pub fn translator_fit(app: AppHandle, height: f64) {
    translator::fit(&app, height);
}

#[tauri::command]
pub fn translator_keyboard(app: AppHandle) {
    translator::take_keyboard(&app);
}

#[tauri::command]
pub fn translator_close(app: AppHandle) {
    translator::hide(&app);
}

#[tauri::command]
pub fn translator_moved() {
    translator::moved();
}

#[tauri::command]
pub async fn translator_insert(app: AppHandle, text: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || translator::insert(&app, &text))
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)
}

#[tauri::command]
pub fn translator_fetch(
    app: AppHandle,
    direction: boltay_core::Direction,
    quality: bool,
) -> Result<()> {
    translator::fetch(&app, direction, quality).map_err(message)
}

#[tauri::command]
pub fn transcribe_job() -> transcribe::Job {
    transcribe::job()
}

#[tauri::command]
pub async fn transcribe_pick(app: AppHandle) -> Result<bool> {
    transcribe::pick(&app).map_err(message)
}

/// A file dropped on the window.
#[tauri::command]
pub fn transcribe_path(app: AppHandle, path: PathBuf) -> Result<()> {
    transcribe::start(&app, path).map_err(message)
}

#[tauri::command]
pub fn transcribe_cancel() {
    transcribe::cancel();
}
