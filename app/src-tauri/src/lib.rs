mod audio;
mod backup;
mod clipboard;
mod commands;
mod dictation;
mod downloads;
mod engine;
mod focus;
mod history;
#[cfg(windows)]
mod hooks;
mod hotkey;
mod i18n;
mod logging;
mod mic;
mod overlay;
mod paste;
mod permissions;
mod session;
mod settings;
#[cfg_attr(not(windows), allow(dead_code))]
mod solo;
mod sounds;
mod transcribe;
mod translator;
mod tray;
mod updates;
mod voicecheck;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, RwLock};

use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::dictation::Msg;
use crate::downloads::Downloads;
use crate::engine::Engines;
use crate::history::History;
use crate::hotkey::Hotkeys;
use crate::i18n::Text;
use crate::paste::Paster;
use crate::settings::Settings;
use crate::tray::Tray;

pub const SETTINGS_WINDOW: &str = "settings";

pub struct Paths {
    pub settings: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
}

/// State shared by the UI thread, the dictation thread and the commands.
pub struct Shared {
    settings: RwLock<Settings>,
    pub paths: Paths,
    pub history: History,
    pub engines: Engines,
    pub downloads: Downloads,
    pub paster: Paster,
    pub mic: mic::Monitor,
    pub updates: updates::Updates,
    pub voice_check: voicecheck::VoiceCheck,
    /// A loaded backup waiting for the user to confirm it.
    pub pending_import: Mutex<Option<Settings>>,
    pub hotkeys: Mutex<Hotkeys>,
    /// Why the saved hotkeys did not work at launch, for the settings window.
    pub hotkey_error: Mutex<Option<String>>,
    dictation: Mutex<Option<Sender<Msg>>>,
    tray: Mutex<Option<Tray>>,
    last: Mutex<Option<String>>,
    overlay_owner: AtomicU64,
}

impl Shared {
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    fn replace_settings(&self, settings: Settings) -> Settings {
        std::mem::replace(&mut self.settings.write().unwrap(), settings)
    }

    pub fn models_dir(&self, settings: &Settings) -> PathBuf {
        settings.models_dir(&self.paths.data)
    }

    pub fn send(&self, msg: Msg) {
        if let Some(tx) = self.dictation.lock().unwrap().as_ref() {
            let _ = tx.send(msg);
        }
    }

    /// Kept even with history off, for "copy last dictation" in the tray.
    pub fn remember(&self, text: &str) {
        *self.last.lock().unwrap() = Some(text.to_string());
    }

    /// Every recording takes the overlay over; a delayed hide from an earlier one then
    /// leaves it alone.
    pub fn claim_overlay(&self) {
        self.overlay_owner.fetch_add(1, Ordering::Relaxed);
    }

    pub fn overlay_generation(&self) -> u64 {
        self.overlay_owner.load(Ordering::Relaxed)
    }
}

/// The permission switches, for a dictation that ran into a missing one. A new window
/// checks them on its own.
pub fn show_permissions(app: &AppHandle) {
    open_window(app, None);
    let _ = app.emit_to(SETTINGS_WINDOW, "permissions", ());
}

/// Shows the settings window, optionally on a given section. The window is built on
/// demand and destroyed on close: a hidden webview would cost tens of megabytes all day.
pub fn open_window(app: &AppHandle, section: Option<&str>) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(section) = section {
            let _ = window.emit("navigate", section);
        }
        return;
    }
    let url = format!("index.html#{}", section.unwrap_or(""));
    let built =
        tauri::WebviewWindowBuilder::new(app, SETTINGS_WINDOW, tauri::WebviewUrl::App(url.into()))
            .title("Boltay")
            .inner_size(760.0, 540.0)
            .min_inner_size(640.0, 440.0)
            .center()
            .build();
    match built {
        // A closed window takes its level meter along.
        Ok(window) => {
            #[cfg(windows)]
            sharp_icons(&window);
            window.on_window_event({
                let app = app.clone();
                move |event| {
                    if let tauri::WindowEvent::Destroyed = event {
                        app.state::<Shared>().mic.stop();
                        // Closed while recording a new hotkey: the old ones were let go.
                        hotkey::suspend(&app, false);
                    }
                }
            })
        }
        Err(e) => log::error!("settings window: {e}"),
    }
}

/// Tauri gives a window the largest frame of the app icon, and Windows shrinks that
/// 256 px picture for the title bar and the taskbar into a blur. The exe carries frames
/// drawn for the small sizes: load the ones this screen asks for. Kept per screen scale,
/// the window comes and goes all day.
#[cfg(windows)]
fn sharp_icons(window: &tauri::WebviewWindow) {
    use tauri::utils::platform::WINDOWS_APP_ICON_RESOURCE_ID;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXICON,
        SM_CXSMICON, WM_SETICON,
    };

    static ICONS: Mutex<Vec<(u32, u32, isize)>> = Mutex::new(Vec::new());
    let Ok(hwnd) = window.hwnd() else { return };
    let hwnd = hwnd.0 as _;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let mut icons = ICONS.lock().unwrap();
    for (kind, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
        let cached = icons
            .iter()
            .find(|i| i.0 == dpi && i.1 == kind)
            .map(|i| i.2);
        let icon = cached.unwrap_or_else(|| unsafe {
            let size = GetSystemMetricsForDpi(metric, dpi);
            let module = GetModuleHandleW(std::ptr::null());
            let id = WINDOWS_APP_ICON_RESOURCE_ID as usize as *const u16;
            LoadImageW(module, id, IMAGE_ICON, size, size, LR_DEFAULTCOLOR) as isize
        });
        if icon == 0 {
            continue;
        }
        if cached.is_none() {
            icons.push((dpi, kind, icon));
        }
        unsafe { SendMessageW(hwnd, WM_SETICON, kind as usize, icon) };
    }
}

/// Explorer keeps the taskbar and Start icon of an exe in its own cache, by the path of the
/// file, and an update that replaces the exe leaves the old picture there: the pinned button
/// shows the previous icon while the window already has the new one.
#[cfg(windows)]
fn refresh_shell_icon() {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::UI::Shell::{SHUpdateImageW, Shell_GetCachedImageIndexW};

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let path: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let image = Shell_GetCachedImageIndexW(path.as_ptr(), 0, 0);
        if image >= 0 {
            SHUpdateImageW(path.as_ptr(), 0, 0, image);
        }
    }
}

pub fn open_logs(app: &AppHandle) -> anyhow::Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = &app.state::<Shared>().paths.logs;
    std::fs::create_dir_all(dir)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)?;
    Ok(())
}

pub fn copy_last(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let last = shared
        .last
        .lock()
        .unwrap()
        .clone()
        .or_else(|| shared.history.load().into_iter().next().map(|e| e.text));
    if let Some(text) = last {
        let _ = shared.paster.copy(&text);
    }
}

pub fn set_tray_status(app: &AppHandle, status: Text) {
    let shared = app.state::<Shared>();
    let settings = shared.settings();
    let lang = i18n::resolve(settings.ui_language);
    let tray = shared.tray.lock().unwrap();
    if let Some(tray) = tray.as_ref() {
        tray.set_status(lang, status, &settings.hotkey);
    }
}

pub fn refresh_tray(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let lang = i18n::resolve(shared.settings().ui_language);
    let update = shared.updates.status().available;
    if let Some(tray) = shared.tray.lock().unwrap().as_ref() {
        tray.set_language(lang);
        tray.set_update(lang, update.as_ref().map(|r| r.version.as_str()));
    };
    refresh_status(app);
    let _ = app.emit("settings-changed", ());
}

/// The tray line while nothing is being recorded.
pub fn refresh_status(app: &AppHandle) {
    let shared = app.state::<Shared>();
    if !shared.downloads.active().is_empty() {
        // The download thread keeps the line up to date with its percentage.
        return;
    }
    let status = if shared.engines.is_loading() {
        Text::Loading
    } else if models_ready(&shared) {
        Text::Ready
    } else {
        Text::NoModel
    };
    set_tray_status(app, status);
    let _ = app.emit("engine-changed", ());
}

pub fn show_download_in_tray(app: &AppHandle, done: u64, total: u64) {
    let shared = app.state::<Shared>();
    let lang = i18n::resolve(shared.settings().ui_language);
    let percent = done * 100 / total.max(1);
    if let Some(tray) = shared.tray.lock().unwrap().as_ref() {
        tray.set_status_text(format!("{} {percent}%", i18n::tr(lang, Text::Downloading)));
    };
}

fn models_ready(shared: &Shared) -> bool {
    let settings = shared.settings();
    engine::ready(&shared.models_dir(&settings), settings.speech)
}

/// Loads the model in the background as soon as it is on disk, so the first dictation
/// does not wait for it. The tray says so while it happens.
pub fn preload(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let settings = shared.settings();
    let root = shared.models_dir(&settings);
    if shared.engines.is_loading()
        || shared.engines.is_loaded(settings.speech, &root)
        || !engine::ready(&root, settings.speech)
    {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let shared = app.state::<Shared>();
        set_tray_status(&app, Text::Loading);
        let _ = app.emit("engine-changed", ());
        if let Err(e) = shared
            .engines
            .get(settings.speech, &root, settings.graph_cache)
        {
            log::error!("cannot load the {:?} model: {e:#}", settings.speech);
        }
        refresh_status(&app);
    });
}

pub fn history_changed(app: &AppHandle) {
    let _ = app.emit_to(SETTINGS_WINDOW, "history-changed", ());
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    #[cfg(windows)]
    std::thread::spawn(refresh_shell_icon);

    let handle = app.handle().clone();
    let config_dir = app.path().app_config_dir()?;
    let data_dir = app.path().app_data_dir()?;
    // Everything said lives in the history there: on a shared Linux machine with homes
    // open to all, only the owner may look in.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(&data_dir)?;
        std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let logs = app.path().local_data_dir()?.join("Boltay").join("logs");
    logging::init(&handle, &logs)?;
    let paths = Paths {
        settings: config_dir.join("settings.json"),
        data: data_dir.clone(),
        logs,
    };
    let settings = Settings::load(&paths.settings).sanitized();
    // The login item names the path the app ran from, and the app may have been moved to
    // Applications since.
    #[cfg(target_os = "macos")]
    if settings.autostart && !commands::temporary_location() {
        use tauri_plugin_autostart::ManagerExt;
        let autolaunch = app.autolaunch();
        let _ = autolaunch.disable().and_then(|()| autolaunch.enable());
    }
    let history = History::new(data_dir.join("history.jsonl"));
    if let Some(days) = settings.history.retention_days {
        let _ = history.prune(days, dictation::now_ms());
    }
    let first_run = !data_dir.join(commands::ONBOARDED).exists();

    app.manage(Shared {
        settings: RwLock::new(settings.clone()),
        paths,
        history,
        engines: Engines::default(),
        downloads: Downloads::default(),
        paster: Paster::new(),
        mic: mic::Monitor::default(),
        updates: updates::Updates::default(),
        voice_check: voicecheck::VoiceCheck::default(),
        pending_import: Mutex::new(None),
        hotkey_error: Mutex::new(None),
        hotkeys: Mutex::new(Hotkeys::default()),
        dictation: Mutex::new(None),
        tray: Mutex::new(None),
        last: Mutex::new(None),
        overlay_owner: AtomicU64::new(0),
    });
    let shared = app.state::<Shared>();

    let tray = Tray::build(&handle, i18n::resolve(settings.ui_language))?;
    *shared.tray.lock().unwrap() = Some(tray);
    *shared.dictation.lock().unwrap() = Some(dictation::spawn(handle.clone()));

    let registered = hotkey::register_at_launch(
        &handle,
        [
            &settings.hotkey,
            &settings.translate_hotkey,
            &settings.translator_hotkey,
        ],
        settings.mode,
    );
    let hotkey_failed = registered.is_err();
    if let Err(e) = registered {
        log::error!("hotkey: {e:#}");
        *shared.hotkey_error.lock().unwrap() = Some(format!("{e:#}"));
    }
    refresh_status(&handle);
    preload(&handle);
    updates::watch(&handle);

    // Settings that cannot work as saved deserve a look; everyone else starts in the tray.
    let needs_attention = hotkey_failed || !models_ready(&shared) || permissions::missing();
    if first_run || needs_attention {
        open_window(&handle, None);
    }

    Ok(())
}

/// `boltay-app --dictate` and the like, for a shortcut set in the system: on Wayland the
/// app's own hotkeys only fire while an X11 window has focus. Each run toggles.
fn run_command(app: &AppHandle, args: &[String]) -> bool {
    let key = match args.iter().skip(1).map(String::as_str).next() {
        Some("--dictate") => session::Key::Dictate,
        Some("--translate") => session::Key::Translate,
        Some("--translator") => {
            translator::toggle(app);
            return true;
        }
        _ => return false,
    };
    app.state::<Shared>()
        .send(Msg::Event(session::Event::Toggle(key)));
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !run_command(app, &args) {
                open_window(app, None);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkey::handle)
                .build(),
        )
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::ui_language,
            commands::suspend_hotkey,
            commands::hotkey_error,
            commands::microphones,
            commands::models,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
            commands::history,
            commands::delete_history,
            commands::clear_history,
            commands::copy_text,
            commands::copy_and_dismiss,
            commands::dismiss_overlay,
            commands::overlay_state,
            commands::app_info,
            commands::permissions,
            commands::request_microphone,
            commands::request_accessibility,
            commands::open_permission_settings,
            commands::open_link,
            commands::open_folder,
            commands::open_logs,
            commands::open_notices,
            commands::expand_overlay,
            commands::preview_text,
            commands::close_onboarding,
            commands::preview_sound,
            commands::mic_monitor,
            commands::mic_volume,
            commands::raise_mic_volume,
            commands::update_status,
            commands::check_updates_now,
            commands::export_settings,
            commands::import_settings,
            commands::apply_import,
            commands::cancel_import,
            commands::reset_settings,
            commands::voice_check_start,
            commands::voice_check_stop,
            commands::voice_check_cancel,
            commands::translator_opened,
            commands::translate_text,
            commands::translator_fit,
            commands::translator_keyboard,
            commands::translator_close,
            commands::translator_moved,
            commands::translator_insert,
            commands::translator_fetch,
            commands::transcribe_job,
            commands::transcribe_pick,
            commands::transcribe_path,
            commands::transcribe_cancel,
        ]);
    // Started at login, the app must not take the keyboard from what the user types in.
    #[cfg(target_os = "macos")]
    let builder = builder.activate_ignoring_other_apps(false);
    let app = builder
        .build(tauri::generate_context!())
        .expect("cannot start Boltay");

    app.run(|app, event| {
        // A tray app lives on with every window closed; only "Quit" ends it.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
            return;
        }
        // Started again from Finder or Spotlight: macOS tells the running app instead of
        // starting a second one.
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            open_window(app, None);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = app;
    });
}
