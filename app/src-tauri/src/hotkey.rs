use anyhow::{anyhow, bail, Context, Result};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::dictation::Msg;
use crate::session::{Event, Key};
use crate::settings::HotkeyMode;
use crate::Shared;

/// What a hotkey is bound to. Combinations and lone F keys go through the system hotkey
/// API. A lone right modifier or CapsLock cannot be registered that way: it is watched
/// with low-level hooks, Windows only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Combo(Shortcut),
    Solo(Solo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solo {
    RightCtrl,
    RightAlt,
    RightShift,
    CapsLock,
}

fn escape() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn parse(hotkey: &str) -> Result<Binding> {
    let solo = match hotkey {
        "ControlRight" => Some(Solo::RightCtrl),
        "AltRight" => Some(Solo::RightAlt),
        "ShiftRight" => Some(Solo::RightShift),
        "CapsLock" => Some(Solo::CapsLock),
        // Half of every Ctrl+C: a recording would start on each shortcut.
        "ControlLeft" | "AltLeft" | "ShiftLeft" | "Control" | "Ctrl" | "Alt" | "Shift" => {
            bail!("a left Ctrl, Alt or Shift alone is part of every shortcut, use the right one")
        }
        _ => None,
    };
    if let Some(solo) = solo {
        return Ok(Binding::Solo(solo));
    }
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|e| anyhow!("cannot read the hotkey {hotkey:?}: {e}"))?;
    if shortcut.key == Code::Escape {
        bail!("Esc is reserved for cancelling a recording");
    }
    Ok(Binding::Combo(shortcut))
}

/// The registered hotkeys: dictation, dictation into English, the translator.
#[derive(Default)]
pub struct Hotkeys([Option<Binding>; 3]);

/// Index of the translator hotkey in [`Hotkeys`].
pub const TRANSLATOR: usize = 2;

impl Hotkeys {
    fn combos(&self) -> impl Iterator<Item = Shortcut> + '_ {
        self.0.iter().flatten().filter_map(|b| match b {
            Binding::Combo(s) => Some(*s),
            Binding::Solo(_) => None,
        })
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn solos(&self) -> [Option<Solo>; 3] {
        self.0.map(|b| match b {
            Some(Binding::Solo(solo)) => Some(solo),
            _ => None,
        })
    }

    /// A single key starts recording on a press that may yet turn out to be a shortcut.
    pub fn is_solo(&self, key: Key) -> bool {
        matches!(self.0[key as usize], Some(Binding::Solo(_)))
    }

    fn key_of(&self, shortcut: &Shortcut) -> Option<Key> {
        [Key::Dictate, Key::Translate]
            .into_iter()
            .find(|&k| self.0[k as usize] == Some(Binding::Combo(*shortcut)))
    }

    fn is_translator(&self, shortcut: &Shortcut) -> bool {
        self.0[TRANSLATOR] == Some(Binding::Combo(*shortcut))
    }
}

/// Swaps the hotkeys. If a new one cannot be registered (another app owns it), the old
/// ones are put back so the app never ends up without a hotkey.
pub fn register(
    app: &AppHandle,
    dictate: &str,
    translate: &str,
    translator: &str,
    mode: HotkeyMode,
) -> Result<()> {
    let new = [parse(dictate)?, parse(translate)?, parse(translator)?];
    if new[0] == new[1] || new[0] == new[2] || new[1] == new[2] {
        bail!("every hotkey needs a combination of its own");
    }
    if new.iter().any(|b| matches!(b, Binding::Solo(_))) {
        #[cfg(windows)]
        crate::hooks::start()?;
        #[cfg(not(windows))]
        bail!("a single key works on Windows only for now");
    }
    let shared = app.state::<Shared>();
    let mut current = shared.hotkeys.lock().unwrap();
    let shortcuts = app.global_shortcut();
    let combos: Vec<(Shortcut, &str)> = new
        .iter()
        .zip([dictate, translate, translator])
        .filter_map(|(b, name)| match b {
            Binding::Combo(s) => Some((*s, name)),
            Binding::Solo(_) => None,
        })
        .collect();
    let unchanged =
        current.0 == new.map(Some) && combos.iter().all(|&(s, _)| shortcuts.is_registered(s));
    if !unchanged {
        for old in current.combos() {
            let _ = shortcuts.unregister(old);
        }
        for (i, &(shortcut, name)) in combos.iter().enumerate() {
            if let Err(e) = shortcuts.register(shortcut) {
                for &(done, _) in &combos[..i] {
                    let _ = shortcuts.unregister(done);
                }
                for old in current.combos() {
                    let _ = shortcuts.register(old);
                }
                return Err(e).context(format!("{name} is taken by another app or the system"));
            }
        }
        current.0 = new.map(Some);
    }
    #[cfg(windows)]
    crate::hooks::watch(app, current.solos(), mode);
    #[cfg(not(windows))]
    let _ = mode;
    Ok(())
}

/// At launch there are no old hotkeys to fall back to, and a combination another app holds
/// must not cost the other two: each that can be registered is, the first failure is
/// returned.
pub fn register_at_launch(app: &AppHandle, hotkeys: [&str; 3], mode: HotkeyMode) -> Result<()> {
    let Err(error) = register(app, hotkeys[0], hotkeys[1], hotkeys[2], mode) else {
        return Ok(());
    };
    let shared = app.state::<Shared>();
    let mut current = shared.hotkeys.lock().unwrap();
    let shortcuts = app.global_shortcut();
    for (i, hotkey) in hotkeys.iter().enumerate() {
        let Ok(binding) = parse(hotkey) else { continue };
        if current.0.contains(&Some(binding)) {
            continue;
        }
        let registered = match binding {
            Binding::Combo(shortcut) => shortcuts.register(shortcut).is_ok(),
            #[cfg(windows)]
            Binding::Solo(_) => crate::hooks::start().is_ok(),
            #[cfg(not(windows))]
            Binding::Solo(_) => false,
        };
        if registered {
            current.0[i] = Some(binding);
        }
    }
    #[cfg(windows)]
    crate::hooks::watch(app, current.solos(), mode);
    #[cfg(not(windows))]
    let _ = mode;
    Err(error)
}

/// While the settings window records a new hotkey, the current ones must not fire.
pub fn suspend(app: &AppHandle, suspended: bool) {
    let shortcuts = app.global_shortcut();
    for shortcut in app.state::<Shared>().hotkeys.lock().unwrap().combos() {
        let _ = if suspended {
            shortcuts.unregister(shortcut)
        } else {
            shortcuts.register(shortcut)
        };
    }
    #[cfg(windows)]
    crate::hooks::suspend(suspended);
}

/// Esc is taken from other apps only while a recording runs. In hold mode the user's
/// fingers are still on the hotkey modifiers, so Esc with those counts too. Some combos
/// belong to the system (Ctrl+Shift+Esc opens the Task Manager) and cannot be taken.
pub fn grab_escape(app: &AppHandle, grab: bool) {
    let shortcuts = app.global_shortcut();
    for shortcut in escapes(app) {
        let _ = if grab {
            shortcuts.register(shortcut)
        } else {
            shortcuts.unregister(shortcut)
        };
    }
}

fn escapes(app: &AppHandle) -> Vec<Shortcut> {
    let mut all = vec![escape()];
    for hotkey in app.state::<Shared>().hotkeys.lock().unwrap().combos() {
        let esc = Shortcut::new(Some(hotkey.mods), Code::Escape);
        if !hotkey.mods.is_empty() && !all.contains(&esc) {
            all.push(esc);
        }
    }
    all
}

pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let pressed = event.state() == ShortcutState::Pressed;
    let shared = app.state::<Shared>();
    if shared.hotkeys.lock().unwrap().is_translator(shortcut) {
        // On release: a key still held would repeat into the app once its modifiers are
        // let go for the copy, typing over the selection.
        if !pressed {
            crate::translator::toggle(app);
        }
        return;
    }
    let event = if shortcut.key == Code::Escape {
        if !pressed || crate::translator::escape(app) {
            return;
        }
        Event::Cancel
    } else {
        let Some(key) = shared.hotkeys.lock().unwrap().key_of(shortcut) else {
            return;
        };
        if pressed {
            Event::HotkeyPressed(key)
        } else {
            Event::HotkeyReleased(key)
        }
    };
    shared.send(Msg::Event(event));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    fn combo(hotkey: &str) -> Shortcut {
        match parse(hotkey).unwrap() {
            Binding::Combo(s) => s,
            other => panic!("{hotkey}: {other:?}"),
        }
    }

    #[test]
    fn default_hotkey_parses() {
        let shortcut = combo(crate::settings::DEFAULT_HOTKEY);
        assert_eq!(shortcut.key, Code::Space);
        assert_eq!(shortcut.mods, Modifiers::CONTROL | Modifiers::SHIFT);
    }

    #[test]
    fn accepts_what_the_settings_window_records() {
        for hotkey in [
            "Alt+KeyD",
            "Ctrl+Alt+F9",
            "Super+Space",
            "Shift+Digit1",
            "F8",
            "Control+Backquote",
        ] {
            assert!(parse(hotkey).is_ok(), "{hotkey}");
        }
    }

    #[test]
    fn single_keys() {
        assert_eq!(
            parse("ControlRight").unwrap(),
            Binding::Solo(Solo::RightCtrl)
        );
        assert_eq!(parse("AltRight").unwrap(), Binding::Solo(Solo::RightAlt));
        assert_eq!(
            parse("ShiftRight").unwrap(),
            Binding::Solo(Solo::RightShift)
        );
        assert_eq!(parse("CapsLock").unwrap(), Binding::Solo(Solo::CapsLock));
        // F keys alone stay system hotkeys: they work everywhere.
        assert_eq!(combo("F9").key, Code::F9);
    }

    #[test]
    fn left_modifiers_alone_are_refused() {
        for hotkey in ["ControlLeft", "AltLeft", "ShiftLeft"] {
            let err = parse(hotkey).unwrap_err().to_string();
            assert!(err.contains("right one"), "{hotkey}: {err}");
        }
        for mouse in ["Mouse1", "Mouse2", "Mouse3", "Mouse4"] {
            assert!(parse(mouse).is_err(), "{mouse}");
        }
    }

    #[test]
    fn default_translate_hotkey_parses() {
        let shortcut = combo(crate::settings::DEFAULT_TRANSLATE_HOTKEY);
        assert_eq!(shortcut.key, Code::Space);
        assert_eq!(
            shortcut.mods,
            Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT
        );
        assert_ne!(shortcut, combo(crate::settings::DEFAULT_HOTKEY));
    }

    #[test]
    fn default_translator_hotkey_parses() {
        let shortcut = combo(crate::settings::DEFAULT_TRANSLATOR_HOTKEY);
        assert_eq!(shortcut.key, Code::KeyT);
        let mods = if cfg!(windows) {
            Modifiers::CONTROL | Modifiers::ALT
        } else {
            Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT
        };
        assert_eq!(shortcut.mods, mods);
        assert_ne!(shortcut, combo(crate::settings::DEFAULT_TRANSLATE_HOTKEY));
    }

    #[test]
    fn rejects_garbage_and_escape() {
        for hotkey in ["", "Ctrl+", "Ctrl+Shift+Nothing", "Escape", "Ctrl+Esc"] {
            assert!(parse(hotkey).is_err(), "{hotkey}");
        }
    }
}
