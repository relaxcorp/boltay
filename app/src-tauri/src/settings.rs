use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::sounds::SoundSet;

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
/// The dictation hotkey plus Alt: easy to reach from it, and free on all three systems.
pub const DEFAULT_TRANSLATE_HOTKEY: &str = "Ctrl+Shift+Alt+Space";
/// Opens the translator on the selected text.
/// Not Ctrl+Shift+T: browsers reopen a closed tab with it.
#[cfg(windows)]
pub const DEFAULT_TRANSLATOR_HOTKEY: &str = "Ctrl+Alt+KeyT";
/// Ctrl+Alt+T opens a terminal on GNOME, KDE and Xfce.
#[cfg(not(windows))]
pub const DEFAULT_TRANSLATOR_HOTKEY: &str = "Ctrl+Shift+Alt+KeyT";
/// Where a hotkey put back to its default goes when another action already has that one.
const SPARE_HOTKEYS: [&str; 3] = ["Ctrl+Shift+F9", "Ctrl+Shift+F10", "Ctrl+Shift+F11"];

/// Everything the user can change. Stored as JSON in the config dir; every level fills
/// missing fields with defaults, so an older file keeps loading after an update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey: String,
    /// Same recording, but the result is translated from Russian to English.
    pub translate_hotkey: String,
    /// Translates the selected text, or opens the translator with a field to type into.
    pub translator_hotkey: String,
    /// The big translation models, one per direction, fetched on first use: better on
    /// work chat and business text, 585 MB each.
    pub quality_translation: bool,
    pub mode: HotkeyMode,
    /// In hold mode, a double press keeps recording hands-free until the next press.
    pub latch: bool,
    pub speech: Speech,
    /// Input device name, `None` for the system default.
    pub microphone: Option<String>,
    /// Software gain on the microphone, for quiet voices and quiet microphones.
    pub mic_gain_db: f32,
    /// Speech probability above which the voice detector hears speech. Lower catches a
    /// whisper, higher ignores a noisy room.
    pub vad_threshold: f32,
    /// Where the recording badge appears.
    pub overlay_position: OverlayPosition,
    pub sounds: bool,
    pub sound_set: SoundSet,
    pub autostart: bool,
    /// `None` follows the system language.
    pub ui_language: Option<UiLanguage>,
    pub paste: PasteMethod,
    /// Put back what was on the clipboard before the dictation was pasted through it.
    pub restore_clipboard: bool,
    /// Every paragraph goes out as its own message: Enter between them, none after the
    /// last. Off, line breaks never send anything.
    pub paragraph_messages: bool,
    /// Ask GitHub once a day whether a newer version is out.
    pub check_updates: bool,
    /// A pause in speech this long starts a new paragraph. `None` never does.
    pub paragraph_pause_secs: Option<f32>,
    pub max_recording_secs: u32,
    /// Free the models after this many idle minutes. `None` keeps them loaded.
    pub unload_after_minutes: Option<u32>,
    /// Keep optimized copies of the model graphs on disk: loading twice as fast for about
    /// 600 MB of disk space.
    pub graph_cache: bool,
    /// `None` uses the default location, see [`default_models_dir`].
    pub models_dir: Option<PathBuf>,
    pub history: History,
    pub text: boltay_text::Config,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.into(),
            translate_hotkey: DEFAULT_TRANSLATE_HOTKEY.into(),
            translator_hotkey: DEFAULT_TRANSLATOR_HOTKEY.into(),
            quality_translation: false,
            mode: HotkeyMode::Hold,
            latch: false,
            speech: Speech::Russian,
            microphone: None,
            mic_gain_db: 0.0,
            vad_threshold: boltay_core::Vad::DEFAULT_THRESHOLD,
            overlay_position: OverlayPosition::Bottom,
            sounds: true,
            sound_set: SoundSet::Soft,
            autostart: false,
            ui_language: None,
            paste: PasteMethod::Clipboard,
            restore_clipboard: true,
            paragraph_messages: false,
            check_updates: true,
            paragraph_pause_secs: Some(2.0),
            max_recording_secs: 300,
            unload_after_minutes: None,
            graph_cache: false,
            models_dir: None,
            history: History::default(),
            text: boltay_text::Config::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    Bottom,
    Top,
    /// Next to the text cursor, or the mouse pointer when the app does not show its caret.
    Cursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// Push to talk: records while the hotkey is held.
    Hold,
    /// First press starts, second press stops.
    Toggle,
}

/// The dictation language. Russian goes to GigaAM, everything else to Parakeet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speech {
    Russian,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    En,
    Ru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PasteMethod {
    /// Through the clipboard and Ctrl+V, restoring the clipboard afterwards.
    Clipboard,
    /// Key by key, for fields that block pasting.
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub enabled: bool,
    /// Entries older than this are dropped on start. `None` keeps everything.
    pub retention_days: Option<u32>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: Some(30),
        }
    }
}

impl Settings {
    pub fn paragraph_pause(&self) -> Option<Duration> {
        self.paragraph_pause_secs.map(Duration::from_secs_f32)
    }

    /// A missing file gives defaults. A broken one is kept aside as `settings.json.bad`
    /// and replaced by defaults, so a typo made by hand never stops the app from starting.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut settings) => {
                settings.catch_up();
                settings
            }
            Err(_) => {
                let _ = fs::rename(path, path.with_extension("json.bad"));
                Self::default()
            }
        }
    }

    /// Settings written by an older version get what this one ships: brands added since
    /// then, and the current list of ambiguous words.
    pub fn catch_up(&mut self) {
        self.text.brands.add_new(boltay_text::default_brands());
        // The app has no editor for this list, so it is always the one this version ships.
        self.text.ambiguous.entries = boltay_text::default_ambiguous();
    }

    /// Writes through a temporary file, so a crash mid-write leaves the old settings.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        let mut file =
            fs::File::create(&tmp).with_context(|| format!("cannot write {}", tmp.display()))?;
        file.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))?;
        Ok(())
    }

    pub fn models_dir(&self, data_dir: &Path) -> PathBuf {
        self.models_dir
            .clone()
            .unwrap_or_else(|| default_models_dir(data_dir))
    }

    /// Clamps values a hand-edited file could set to something that breaks recording.
    pub fn sanitized(mut self) -> Self {
        self.max_recording_secs = self.max_recording_secs.clamp(5, 3600);
        self.unload_after_minutes = self.unload_after_minutes.filter(|&m| m > 0);
        self.mic_gain_db = finite_or(self.mic_gain_db, 0.0).clamp(0.0, MAX_GAIN_DB);
        self.vad_threshold =
            finite_or(self.vad_threshold, boltay_core::Vad::DEFAULT_THRESHOLD).clamp(0.1, 0.9);
        self.paragraph_pause_secs = self
            .paragraph_pause_secs
            .filter(|s| s.is_finite() && *s > 0.0)
            .map(|s| s.clamp(0.5, 30.0));
        let defaults = [
            DEFAULT_HOTKEY,
            DEFAULT_TRANSLATE_HOTKEY,
            DEFAULT_TRANSLATOR_HOTKEY,
        ];
        let hotkeys = [
            &mut self.hotkey,
            &mut self.translate_hotkey,
            &mut self.translator_hotkey,
        ];
        for i in 0..hotkeys.len() {
            // A settings file brought over from another system must not leave this one
            // without a hotkey: a key on its own needs the Windows input hooks. Mouse buttons
            // used to be allowed, and a bound side button broke clicks.
            let usable = match crate::hotkey::parse(hotkeys[i]) {
                Ok(crate::hotkey::Binding::Solo(_)) => cfg!(windows),
                Ok(crate::hotkey::Binding::Combo(_)) => true,
                Err(_) => false,
            };
            if usable {
                continue;
            }
            // The user may have moved the default to another action: then the next free one.
            let taken = |hotkeys: &[&mut String; 3], candidate: &str| {
                (0..hotkeys.len()).any(|j| j != i && same_hotkey(hotkeys[j], candidate))
            };
            let free = std::iter::once(defaults[i])
                .chain(defaults)
                .chain(SPARE_HOTKEYS)
                .find(|candidate| !taken(&hotkeys, candidate));
            *hotkeys[i] = free.unwrap_or(defaults[i]).into();
        }
        self
    }
}

pub const MAX_GAIN_DB: f32 = 24.0;

/// Ctrl+Shift+Space and Shift+Ctrl+Space are one hotkey.
fn same_hotkey(a: &str, b: &str) -> bool {
    match (crate::hotkey::parse(a), crate::hotkey::parse(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a.eq_ignore_ascii_case(b),
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// Where the CLI keeps models too, so one download serves both. Debug builds prefer
/// `models/` at the repo root when `scripts/fetch-models.sh` has filled it.
pub fn default_models_dir(data_dir: &Path) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
    if cfg!(debug_assertions) && repo.is_dir() {
        return repo;
    }
    boltay_core::Store::default_root().unwrap_or_else(|| data_dir.join("models"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Settings::load(&dir.path().join("settings.json")),
            Settings::default()
        );
    }

    #[test]
    fn single_keys_fall_back_outside_windows() {
        let settings = Settings {
            hotkey: "ControlRight".into(),
            translate_hotkey: "CapsLock".into(),
            translator_hotkey: "Alt+F9".into(),
            ..Settings::default()
        }
        .sanitized();
        if cfg!(windows) {
            assert_eq!(settings.hotkey, "ControlRight");
            assert_eq!(settings.translate_hotkey, "CapsLock");
        } else {
            assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
            assert_eq!(settings.translate_hotkey, DEFAULT_TRANSLATE_HOTKEY);
        }
        assert_eq!(settings.translator_hotkey, "Alt+F9");
    }

    #[test]
    fn a_fallback_takes_a_free_hotkey() {
        // Right Ctrl for dictation, its old combination moved to the translation.
        let settings = Settings {
            hotkey: "ControlRight".into(),
            translate_hotkey: "Shift+Ctrl+Space".into(),
            translator_hotkey: "Mouse4".into(),
            ..Settings::default()
        }
        .sanitized();
        let all = [
            &settings.hotkey,
            &settings.translate_hotkey,
            &settings.translator_hotkey,
        ];
        assert_eq!(settings.translate_hotkey, "Shift+Ctrl+Space");
        assert_eq!(settings.translator_hotkey, DEFAULT_TRANSLATOR_HOTKEY);
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert!(!same_hotkey(a, b), "{a} and {b}");
            }
        }
        if cfg!(windows) {
            assert_eq!(settings.hotkey, "ControlRight");
        } else {
            assert_eq!(settings.hotkey, DEFAULT_TRANSLATE_HOTKEY);
        }
    }

    #[test]
    fn unreadable_hotkeys_fall_back() {
        let settings = Settings {
            hotkey: "Mouse4".into(),
            translate_hotkey: "Ctrl+Shift+Nothing".into(),
            translator_hotkey: "".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.translate_hotkey, DEFAULT_TRANSLATE_HOTKEY);
        assert_eq!(settings.translator_hotkey, DEFAULT_TRANSLATOR_HOTKEY);
    }

    #[test]
    fn spare_hotkeys_parse() {
        for hotkey in SPARE_HOTKEYS {
            assert!(crate::hotkey::parse(hotkey).is_ok(), "{hotkey}");
        }
        assert!(same_hotkey("Ctrl+Shift+Space", "Shift+Ctrl+Space"));
        assert!(!same_hotkey("Ctrl+Shift+Space", "Ctrl+Shift+Alt+Space"));
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let mut settings = Settings {
            hotkey: "Alt+F9".into(),
            mode: HotkeyMode::Toggle,
            speech: Speech::Other,
            microphone: Some("USB Mic".into()),
            ui_language: Some(UiLanguage::Ru),
            paste: PasteMethod::Type,
            models_dir: Some("/opt/models".into()),
            ..Settings::default()
        };
        settings.text.profanity = boltay_text::Profanity::Soften;
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"mode": "toggle", "history": {"enabled": false}, "text": {"profanity": "mask"}}"#,
        )
        .unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.mode, HotkeyMode::Toggle);
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert!(!settings.history.enabled);
        assert_eq!(settings.history.retention_days, Some(30));
        assert_eq!(settings.text.profanity, boltay_text::Profanity::Mask);
        assert!(settings.text.fillers.hard);
        // Added later: a file from an older version gets them on.
        assert!(settings.restore_clipboard);
        assert_eq!(settings.paragraph_pause(), Some(Duration::from_secs(2)));
    }

    #[test]
    fn brand_lists_catch_up_with_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let old = r#"{"text": {
            "brands": {"enabled": true, "entries": [{"name": "GitHub", "variants": ["гитхап"]}],
                       "seen": ["GitHub", "Spotify"]},
            "ambiguous": {"enabled": true, "entries": [{"name": "TON", "variants": ["тон"]}]}
        }}"#;
        fs::write(&path, old).unwrap();
        let text = Settings::load(&path).text;
        let names: Vec<&str> = text
            .brands
            .entries
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names.last(), Some(&"GitHub"));
        assert!(names.contains(&"Telegram"));
        assert!(!names.contains(&"Spotify"), "deleted by the user");
        assert!(text.ambiguous.enabled);
        assert_eq!(text.ambiguous.entries, boltay_text::default_ambiguous());
    }

    #[test]
    fn kept_brand_rows_get_new_variants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let old = r#"{"text": {"brands": {"enabled": true,
            "entries": [{"name": "Jira", "variants": ["джира"]}], "seen": ["Jira"]}}}"#;
        fs::write(&path, old).unwrap();
        let jira = |settings: &Settings| {
            settings
                .text
                .brands
                .entries
                .last()
                .unwrap()
                .variants
                .clone()
        };
        let mut settings = Settings::load(&path);
        assert!(jira(&settings).iter().any(|v| v == "джайро"));
        // Saved again, a variant the user then deletes stays deleted.
        settings.text.brands.entries.last_mut().unwrap().variants = vec!["джира".into()];
        settings.save(&path).unwrap();
        assert_eq!(jira(&Settings::load(&path)), ["джира"]);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"sounds": false, "from_the_future": 1}"#).unwrap();
        assert!(!Settings::load(&path).sounds);
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.bad")).unwrap(),
            "{ not json"
        );
        assert!(!path.exists());
    }

    #[test]
    fn wrong_enum_value_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"mode": "sometimes"}"#).unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn sanitizes_hand_edited_values() {
        let settings = Settings {
            hotkey: "  ".into(),
            max_recording_secs: 0,
            unload_after_minutes: Some(0),
            paragraph_pause_secs: Some(0.01),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(settings.paragraph_pause_secs, Some(0.5));
        let off = Settings {
            paragraph_pause_secs: Some(f32::NAN),
            ..Settings::default()
        };
        assert_eq!(off.sanitized().paragraph_pause_secs, None);
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.max_recording_secs, 5);
        assert_eq!(settings.unload_after_minutes, None);

        let loud = Settings {
            mic_gain_db: 100.0,
            vad_threshold: f32::NAN,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(loud.mic_gain_db, MAX_GAIN_DB);
        assert_eq!(loud.vad_threshold, boltay_core::Vad::DEFAULT_THRESHOLD);
    }

    #[test]
    fn models_dir_prefers_the_setting() {
        let data = Path::new("/data");
        let custom = Settings {
            models_dir: Some("/custom".into()),
            ..Settings::default()
        };
        assert_eq!(custom.models_dir(data), Path::new("/custom"));
        assert_eq!(
            Settings::default().models_dir(data),
            default_models_dir(data)
        );
    }

    #[test]
    fn default_values() {
        let s = Settings::default();
        assert_eq!(s.hotkey, "Ctrl+Shift+Space");
        assert_eq!(s.translate_hotkey, "Ctrl+Shift+Alt+Space");
        assert!(!s.graph_cache);
        assert!(!s.quality_translation);
        assert_eq!(s.mode, HotkeyMode::Hold);
        assert_eq!(s.max_recording_secs, 300);
        assert_eq!(s.paste, PasteMethod::Clipboard);
        assert!(s.sounds);
        assert!(!s.autostart);
        assert_eq!(s.text.profanity, boltay_text::Profanity::Keep);
        assert!(!s.latch);
        assert!(!s.paragraph_messages);
        assert!(s.check_updates);
        assert_eq!(s.sound_set, SoundSet::Soft);
        assert_eq!(s.overlay_position, OverlayPosition::Bottom);
        assert_eq!(s.mic_gain_db, 0.0);
    }
}
