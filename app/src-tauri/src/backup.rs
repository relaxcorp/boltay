use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::settings::Settings;

/// Marks a file as Boltay settings: any other JSON would load as "everything default".
const FORMAT: &str = "boltay-settings";

/// Settings that belong to this computer, not to the person: a backup carries over neither
/// the microphone nor the models folder nor autostart, and a reset keeps the models folder.
const MACHINE: &[&str] = &["microphone", "models_dir", "autostart"];

/// The settings as a backup file. History lives elsewhere and is never in it.
pub fn export(settings: &Settings) -> Result<String> {
    let mut value = serde_json::to_value(settings)?;
    let object = value
        .as_object_mut()
        .context("settings are not an object")?;
    for key in MACHINE {
        object.remove(*key);
    }
    let mut out = Map::new();
    out.insert("format".into(), FORMAT.into());
    out.extend(std::mem::take(object));
    Ok(serde_json::to_string_pretty(&out)?)
}

/// Reads a backup over the current settings: what it lacks stays at defaults, what belongs
/// to this computer stays as it is now.
pub fn import(text: &str, current: &Settings) -> Result<Settings> {
    let mut value: Value = serde_json::from_str(text).context("not a settings file")?;
    let object = value.as_object_mut().context("not a settings file")?;
    if object.remove("format").as_ref().and_then(Value::as_str) != Some(FORMAT) {
        bail!("not a Boltay settings file");
    }
    let mut settings: Settings = serde_json::from_value(value).context("not a settings file")?;
    settings.catch_up();
    keep_machine(&mut settings, current);
    Ok(settings.sanitized())
}

/// Back to how it was after installing, on this computer.
pub fn reset(current: &Settings) -> Settings {
    let mut settings = Settings::default();
    settings.models_dir.clone_from(&current.models_dir);
    settings
}

fn keep_machine(settings: &mut Settings, current: &Settings) {
    settings.microphone.clone_from(&current.microphone);
    settings.models_dir.clone_from(&current.models_dir);
    settings.autostart = current.autostart;
}

/// What loading a backup would change, for the question before it is applied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub file: String,
    /// Last change of the file, in milliseconds since the epoch.
    pub modified: Option<u64>,
    /// Settings with a different value, the lists below not counted.
    pub changes: usize,
    pub dictionary: Delta,
    pub snippets: Delta,
    pub brands: Delta,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Delta {
    pub added: usize,
    pub removed: usize,
}

pub fn preview(path: &Path, from: &Settings, to: &Settings) -> Result<Preview> {
    let modified = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    let (old, new) = (serde_json::to_value(from)?, serde_json::to_value(to)?);
    let lists = [
        "/text/dictionary/entries",
        "/text/snippets/entries",
        "/text/brands/entries",
        "/text/brands/seen",
        "/text/brands/seen_variants",
        "/text/ambiguous/seen",
        "/text/ambiguous/seen_variants",
    ];
    let mut changes = 0;
    count_changes(&old, &new, String::new(), &lists, &mut changes);
    let delta = |pointer: &str| {
        let items = |v: &Value| -> BTreeSet<String> {
            v.pointer(pointer)
                .and_then(Value::as_array)
                .map(|a| a.iter().map(Value::to_string).collect())
                .unwrap_or_default()
        };
        let (before, after) = (items(&old), items(&new));
        Delta {
            added: after.difference(&before).count(),
            removed: before.difference(&after).count(),
        }
    };
    Ok(Preview {
        file: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        modified,
        changes,
        dictionary: delta(lists[0]),
        snippets: delta(lists[1]),
        brands: delta(lists[2]),
    })
}

fn count_changes(old: &Value, new: &Value, path: String, skip: &[&str], count: &mut usize) {
    if skip.contains(&path.as_str()) {
        return;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                count_changes(
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    format!("{path}/{key}"),
                    skip,
                    count,
                );
            }
        }
        (a, b) if a != b => *count += 1,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use boltay_text::{Replacement, Snippet};

    use super::*;
    use crate::settings::HotkeyMode;

    fn mine() -> Settings {
        Settings {
            microphone: Some("USB Mic".into()),
            models_dir: Some("D:/models".into()),
            autostart: true,
            ..Settings::default()
        }
    }

    #[test]
    fn export_leaves_out_this_computer() {
        let file = export(&mine()).unwrap();
        assert!(file.contains("\"format\": \"boltay-settings\""));
        for key in MACHINE {
            assert!(!file.contains(&format!("\"{key}\"")), "{key} in {file}");
        }
    }

    #[test]
    fn import_keeps_this_computer_and_takes_the_rest() {
        let mut theirs = Settings {
            mode: HotkeyMode::Toggle,
            microphone: Some("Their mic".into()),
            ..Settings::default()
        };
        theirs.text.dictionary.entries = vec![Replacement::new("релакс", "Relax")];
        let file = export(&theirs).unwrap();
        let loaded = import(&file, &mine()).unwrap();
        assert_eq!(loaded.mode, HotkeyMode::Toggle);
        assert_eq!(loaded.text.dictionary.entries.len(), 1);
        assert_eq!(loaded.microphone.as_deref(), Some("USB Mic"));
        assert_eq!(loaded.models_dir, mine().models_dir);
        assert!(loaded.autostart);
    }

    #[test]
    fn import_refuses_other_files() {
        assert!(import("{}", &mine()).is_err());
        assert!(import(r#"{"mode": "toggle"}"#, &mine()).is_err());
        assert!(import("not json", &mine()).is_err());
        assert!(import(
            r#"{"format": "boltay-settings", "mode": "toggle"}"#,
            &mine()
        )
        .is_ok());
    }

    #[test]
    fn backup_from_an_older_version_catches_up() {
        let old = r#"{"format": "boltay-settings", "text": {
            "brands": {"enabled": true, "entries": [], "seen": ["Spotify"]},
            "ambiguous": {"enabled": true, "entries": [{"name": "TON", "variants": ["тон"]}]}
        }}"#;
        let current = Settings::default();
        let loaded = import(old, &current).unwrap();
        let names: Vec<&str> = loaded
            .text
            .brands
            .entries
            .iter()
            .map(|b| b.name.as_str())
            .collect();
        assert!(names.contains(&"Telegram"));
        assert!(!names.contains(&"Spotify"), "deleted by the user");
        assert_eq!(
            loaded.text.ambiguous.entries,
            boltay_text::default_ambiguous()
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.json");
        fs::write(&path, old).unwrap();
        let preview = preview(&path, &current, &loaded).unwrap();
        assert_eq!(preview.brands.removed, 1, "only Spotify");
    }

    #[test]
    fn reset_keeps_the_models_folder_only() {
        let reset = reset(&mine());
        assert_eq!(reset.models_dir, mine().models_dir);
        assert_eq!(reset.microphone, None);
        assert!(!reset.autostart);
    }

    #[test]
    fn preview_counts_settings_and_list_entries() {
        let from = mine();
        let mut to = mine();
        to.mode = HotkeyMode::Toggle;
        to.sounds = false;
        to.text.dictionary.entries = vec![Replacement::new("а", "A"), Replacement::new("б", "B")];
        to.text.snippets.entries = vec![Snippet::new("подпись", "Иван")];
        to.text.brands.entries.pop();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("boltay-settings.json");
        fs::write(&path, "{}").unwrap();
        let preview = preview(&path, &from, &to).unwrap();
        assert_eq!(preview.file, "boltay-settings.json");
        assert!(preview.modified.is_some());
        assert_eq!(preview.changes, 2);
        assert_eq!(
            preview.dictionary,
            Delta {
                added: 2,
                removed: 0
            }
        );
        assert_eq!(
            preview.snippets,
            Delta {
                added: 1,
                removed: 0
            }
        );
        assert_eq!(
            preview.brands,
            Delta {
                added: 0,
                removed: 1
            }
        );
    }

    #[test]
    fn nothing_to_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("same.json");
        fs::write(&path, "{}").unwrap();
        let preview = preview(&path, &mine(), &mine()).unwrap();
        assert_eq!(preview.changes, 0);
        assert_eq!(preview.dictionary, Delta::default());
    }
}
