use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::i18n::{tr, Text};
use crate::settings::UiLanguage;

pub const ID: &str = "main";
/// The app icon is a lit egg on a warm tile, mush at 16 px. The tray gets a flat egg with a
/// bolder crack, drawn in `icons/tray.svg`.
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray.png");

/// Menu items whose text changes while the app runs.
pub struct Tray {
    menu: Menu<Wry>,
    status: MenuItem<Wry>,
    items: [(MenuItem<Wry>, Text); 7],
    /// "Version X is out", on top of the menu only while there is one.
    update: MenuItem<Wry>,
    update_shown: AtomicBool,
}

impl Tray {
    pub fn build(app: &AppHandle, lang: UiLanguage) -> tauri::Result<Self> {
        let status = MenuItem::with_id(app, "status", tr(lang, Text::Ready), false, None::<&str>)?;
        let settings = MenuItem::with_id(
            app,
            "settings",
            tr(lang, Text::Settings),
            true,
            None::<&str>,
        )?;
        let history =
            MenuItem::with_id(app, "history", tr(lang, Text::History), true, None::<&str>)?;
        let copy = MenuItem::with_id(
            app,
            "copy-last",
            tr(lang, Text::CopyLast),
            true,
            None::<&str>,
        )?;
        let translator = MenuItem::with_id(
            app,
            "translator",
            tr(lang, Text::Translator),
            true,
            None::<&str>,
        )?;
        let transcribe = MenuItem::with_id(
            app,
            "transcribe",
            tr(lang, Text::Transcribe),
            true,
            None::<&str>,
        )?;
        let logs = MenuItem::with_id(app, "logs", tr(lang, Text::Logs), true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", tr(lang, Text::Quit), true, None::<&str>)?;
        let update = MenuItem::with_id(app, "update", "", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &status,
                &PredefinedMenuItem::separator(app)?,
                &settings,
                &history,
                &copy,
                &PredefinedMenuItem::separator(app)?,
                &translator,
                &transcribe,
                &PredefinedMenuItem::separator(app)?,
                &logs,
                &quit,
            ],
        )?;
        TrayIconBuilder::with_id(ID)
            .icon(Image::from_bytes(TRAY_ICON)?)
            .tooltip("Boltay")
            .menu(&menu)
            // A menu bar icon opens its menu on a click; a tray icon opens the window.
            .show_menu_on_left_click(cfg!(target_os = "macos"))
            .on_menu_event(on_menu)
            .on_tray_icon_event(|tray, event| {
                if cfg!(target_os = "macos") {
                    return;
                }
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    crate::open_window(tray.app_handle(), None);
                }
            })
            .build(app)?;
        Ok(Self {
            menu,
            update,
            update_shown: AtomicBool::new(false),
            status,
            items: [
                (settings, Text::Settings),
                (history, Text::History),
                (copy, Text::CopyLast),
                (translator, Text::Translator),
                (transcribe, Text::Transcribe),
                (logs, Text::Logs),
                (quit, Text::Quit),
            ],
        })
    }

    /// `hotkey` is shown next to "Ready" so the user always sees what to press.
    pub fn set_status(&self, lang: UiLanguage, status: Text, hotkey: &str) {
        let text = match status {
            Text::Ready => format!("{} — {}", tr(lang, status), pretty(hotkey, lang)),
            _ => tr(lang, status).to_string(),
        };
        let _ = self.status.set_text(text);
    }

    pub fn set_status_text(&self, text: String) {
        let _ = self.status.set_text(text);
    }

    pub fn set_update(&self, lang: UiLanguage, version: Option<&str>) {
        let shown = self.update_shown.load(Ordering::Relaxed);
        let Some(version) = version else {
            if shown && self.menu.remove(&self.update).is_ok() {
                self.update_shown.store(false, Ordering::Relaxed);
            }
            return;
        };
        let _ = self
            .update
            .set_text(tr(lang, Text::Update).replace("{v}", version));
        if !shown && self.menu.insert(&self.update, 0).is_ok() {
            self.update_shown.store(true, Ordering::Relaxed);
        }
    }

    pub fn set_language(&self, lang: UiLanguage) {
        for (item, text) in &self.items {
            let _ = item.set_text(tr(lang, *text));
        }
    }
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "settings" => crate::open_window(app, None),
        "history" => crate::open_window(app, Some("history")),
        "copy-last" => crate::copy_last(app),
        "translator" => crate::translator::open_empty(app),
        "transcribe" => {
            let app = app.clone();
            std::thread::spawn(move || match crate::transcribe::pick(&app) {
                Ok(true) => crate::open_window(&app, Some("transcribe")),
                Ok(false) => {}
                Err(e) => log::error!("transcription: {e:#}"),
            });
        }
        "logs" => {
            if let Err(e) = crate::open_logs(app) {
                log::error!("logs folder: {e:#}");
            }
        }
        "update" => {
            let status = app.state::<crate::Shared>().updates.status();
            if let Some(release) = status.available {
                let _ = app.opener().open_url(release.url, None::<&str>);
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

/// As the settings window shows a hotkey: `Super+Shift+KeyD` is `Cmd + Shift + D` on macOS.
fn pretty(hotkey: &str, lang: UiLanguage) -> String {
    let mac = cfg!(target_os = "macos");
    let ru = lang == UiLanguage::Ru;
    hotkey
        .split('+')
        .map(|part| match part {
            "Super" if mac => "Cmd",
            "Super" => "Win",
            "Alt" if mac => "Option",
            "ControlRight" if ru => "Правый Ctrl",
            "ControlRight" => "Right Ctrl",
            "AltRight" if ru => "Правый Alt",
            "AltRight" => "Right Alt",
            "ShiftRight" if ru => "Правый Shift",
            "ShiftRight" => "Right Shift",
            _ => part
                .strip_prefix("Key")
                .or_else(|| part.strip_prefix("Digit"))
                .filter(|key| key.len() == 1)
                .unwrap_or(part),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_read_as_in_the_window() {
        assert_eq!(
            pretty("Ctrl+Shift+KeyD", UiLanguage::En),
            "Ctrl + Shift + D"
        );
        assert!(pretty("Ctrl+Alt+Digit1", UiLanguage::En).ends_with(" + 1"));
        assert_eq!(
            pretty("Ctrl+Shift+Space", UiLanguage::En),
            "Ctrl + Shift + Space"
        );
        assert_eq!(pretty("ControlRight", UiLanguage::Ru), "Правый Ctrl");
        assert_eq!(pretty("F9", UiLanguage::En), "F9");
    }
}
