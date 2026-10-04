use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use enigo::{Direction, Enigo, Key, Keyboard};
use serde::Serialize;

use crate::clipboard::Contents;
use crate::settings::PasteMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayServer {
    X11,
    Wayland,
    /// Windows and macOS.
    Native,
}

impl DisplayServer {
    pub fn detect() -> Self {
        Self::from_env(|name| std::env::var(name).ok())
    }

    fn from_env(var: impl Fn(&str) -> Option<String>) -> Self {
        if !cfg!(target_os = "linux") {
            return Self::Native;
        }
        let session = var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();
        match session.as_str() {
            "wayland" => Self::Wayland,
            "x11" => Self::X11,
            _ if var("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty()) => Self::Wayland,
            _ => Self::X11,
        }
    }
}

/// How long the dictation stays on the clipboard after Ctrl+V. The target app reads it
/// asynchronously, and on a loaded machine a quarter of a second was not always enough.
const RESTORE_AFTER: Duration = Duration::from_millis(800);

/// V by its physical key, not by the character: with a Russian layout there is no `v` to
/// type, and Windows would send the letter as text instead of the shortcut. On X11 the
/// keysym is looked up in the first layout, where Latin letters live.
#[cfg(windows)]
const V: Key = Key::V;
#[cfg(target_os = "macos")]
const V: Key = Key::Other(9); // kVK_ANSI_V
#[cfg(all(unix, not(target_os = "macos")))]
const V: Key = Key::Unicode('v');
#[cfg(windows)]
const C: Key = Key::C;
#[cfg(target_os = "macos")]
const C: Key = Key::Other(8); // kVK_ANSI_C
#[cfg(all(unix, not(target_os = "macos")))]
const C: Key = Key::Unicode('c');

/// How long the app gets to put a selection on the clipboard after Ctrl+C.
const COPY_WAIT: Duration = Duration::from_millis(400);

/// Puts text into whatever field has focus in another app.
pub struct Paster {
    board: Arc<Mutex<Board>>,
    display: DisplayServer,
}

#[derive(Default)]
struct Board {
    /// Kept alive for the whole run: on Linux the clipboard contents die with the owner.
    clipboard: Option<arboard::Clipboard>,
    /// What the user had copied, while a paste waits to put it back.
    saved: Option<Contents>,
    pastes: u64,
}

impl Paster {
    pub fn new() -> Self {
        Self {
            board: Arc::new(Mutex::new(Board {
                clipboard: arboard::Clipboard::new().ok(),
                ..Board::default()
            })),
            display: DisplayServer::detect(),
        }
    }

    pub fn display(&self) -> DisplayServer {
        self.display
    }

    /// `hotkey` tells which modifiers the user may still be holding. `restore` puts the
    /// previous clipboard contents back a moment after a paste through the clipboard.
    /// `messages` sends every paragraph on its own, see [`plan`].
    pub fn paste(
        &self,
        text: &str,
        method: PasteMethod,
        hotkey: &str,
        restore: bool,
        messages: bool,
    ) -> Result<()> {
        let held = held_modifiers(hotkey);
        let clipboard = method == PasteMethod::Clipboard;
        let steps = plan(text, messages, clipboard);
        for (i, step) in steps.iter().enumerate() {
            match step {
                Step::Text(text) if clipboard => {
                    self.via_clipboard(text, &held, restore)?;
                    // The app reads the clipboard on its own time: Enter right after Ctrl+V
                    // can send the message before the text is in it.
                    if steps.get(i + 1) == Some(&Step::Send) {
                        thread::sleep(Duration::from_millis(150));
                    }
                }
                Step::Text(text) => self.type_text(text, &held)?,
                Step::LineBreak => self.keys(true)?,
                Step::Send => {
                    self.keys(false)?;
                    // Let the message go before the next one starts.
                    thread::sleep(Duration::from_millis(150));
                }
            }
        }
        Ok(())
    }

    /// The text on the clipboard now, for the `{буфер}` placeholder in snippets.
    pub fn read(&self) -> Option<String> {
        let mut board = self.board.lock().unwrap();
        clipboard(&mut board.clipboard).ok()?.get_text().ok()
    }

    pub fn copy(&self, text: &str) -> Result<()> {
        let mut board = self.board.lock().unwrap();
        board.saved = None;
        clipboard(&mut board.clipboard)?
            .set_text(text)
            .context("cannot write to the clipboard")
    }

    /// What is selected in the focused app, through Ctrl+C, with the clipboard put back
    /// as it was right after. `None` when nothing is selected.
    pub fn copy_selection(&self, hotkey: &str) -> Result<Option<String>> {
        let held = held_modifiers(hotkey);
        let mut guard = self.board.lock().unwrap();
        let board = &mut *guard;
        let current = clipboard(&mut board.clipboard)?;
        // A paste may still be waiting to put the user's copy back: that is the one to keep.
        let previous = board
            .saved
            .take()
            .or_else(|| crate::clipboard::take(current));
        // Empty, so an unchanged clipboard means there was nothing to copy.
        let _ = current.clear();
        let copied = self.chord(&held, C);
        let mut selected = None;
        let started = Instant::now();
        while copied.is_ok() && started.elapsed() < COPY_WAIT {
            thread::sleep(Duration::from_millis(25));
            if let Ok(text) = current.get_text() {
                if !text.trim().is_empty() {
                    selected = Some(text);
                    break;
                }
            }
        }
        let restored = match previous {
            Some(previous) => crate::clipboard::put(current, previous),
            None => current.clear().context("cannot clear the clipboard"),
        };
        if let Err(e) = restored {
            log::warn!("{e:#}");
        }
        copied?;
        Ok(selected)
    }

    /// Clipboard, Ctrl+V, then what the user had copied back.
    fn via_clipboard(&self, text: &str, held: &[Key], restore: bool) -> Result<()> {
        let mut board = self.board.lock().unwrap();
        // Right after another dictation the clipboard holds that one, the user's own copy
        // is still waiting to be put back. Nothing to keep either if nothing goes back.
        let fresh = restore && board.saved.is_none();
        let current = clipboard(&mut board.clipboard)?;
        let previous = if fresh {
            crate::clipboard::take(current)
        } else {
            None
        };
        if restore {
            crate::clipboard::set_passing(current, text)?;
        } else {
            // The text stays with the user, as if they had copied it.
            current
                .set_text(text)
                .context("cannot write to the clipboard")?;
        }
        // Clipboard managers and the target app pick up the change asynchronously.
        thread::sleep(Duration::from_millis(40));
        let previous = board.saved.take().or(previous);
        self.chord(held, V)?;
        let Some(previous) = previous.filter(|_| restore) else {
            return Ok(());
        };
        board.saved = Some(previous);
        board.pastes += 1;
        let paste = board.pastes;
        let board = self.board.clone();
        let text = text.to_string();
        thread::spawn(move || {
            thread::sleep(RESTORE_AFTER);
            let mut board = board.lock().unwrap();
            if board.pastes != paste {
                return;
            }
            let Some(previous) = board.saved.take() else {
                return;
            };
            if let Ok(clipboard) = clipboard(&mut board.clipboard) {
                // Something copied in the meantime is newer than both and stays.
                if clipboard.get_text().ok().as_deref() == Some(text.as_str()) {
                    if let Err(e) = crate::clipboard::put(clipboard, previous) {
                        log::warn!("{e:#}");
                    }
                }
            }
        });
        Ok(())
    }

    /// Ctrl (Cmd on macOS) with V to paste or C to copy.
    fn chord(&self, held: &[Key], key: Key) -> Result<()> {
        if self.display == DisplayServer::Wayland {
            let (letter, code) = if key == C { ("c", "46") } else { ("v", "47") };
            let (down, up) = (format!("{code}:1"), format!("{code}:0"));
            return wayland_tool(&[
                &["wtype", "-M", "ctrl", letter, "-m", "ctrl"],
                // Linux input event codes: 29 is left Ctrl, 47 is V, 46 is C.
                &["ydotool", "key", "29:1", &down, &up, "29:0"],
            ]);
        }
        let mut enigo = enigo()?;
        release(&mut enigo, held);
        let modifier = if cfg!(target_os = "macos") {
            Key::Meta
        } else {
            Key::Control
        };
        enigo.key(modifier, Direction::Press)?;
        let clicked = enigo.key(key, Direction::Click);
        enigo.key(modifier, Direction::Release)?;
        clicked?;
        Ok(())
    }

    /// One line of text, no line breaks: those are keys of their own.
    fn type_text(&self, text: &str, held: &[Key]) -> Result<()> {
        if self.display == DisplayServer::Wayland {
            return wayland_tool(&[&["wtype", "--", text], &["ydotool", "type", "--", text]]);
        }
        let mut enigo = enigo()?;
        release(&mut enigo, held);
        enigo.text(text)?;
        Ok(())
    }

    /// Shift+Enter for a line break inside a message, plain Enter to send it.
    fn keys(&self, shift: bool) -> Result<()> {
        if self.display == DisplayServer::Wayland {
            // Linux input event codes: 42 is left Shift, 28 is Enter.
            return if shift {
                wayland_tool(&[
                    &["wtype", "-M", "shift", "-k", "Return", "-m", "shift"],
                    &["ydotool", "key", "42:1", "28:1", "28:0", "42:0"],
                ])
            } else {
                wayland_tool(&[
                    &["wtype", "-k", "Return"],
                    &["ydotool", "key", "28:1", "28:0"],
                ])
            };
        }
        let mut enigo = enigo()?;
        if shift {
            enigo.key(Key::Shift, Direction::Press)?;
        }
        let clicked = enigo.key(Key::Return, Direction::Click);
        if shift {
            enigo.key(Key::Shift, Direction::Release)?;
        }
        clicked?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step<'a> {
    Text(&'a str),
    /// Shift+Enter: a new line in the same message. Enter would send it in a messenger.
    LineBreak,
    /// Enter: sends what is in the field.
    Send,
}

/// How a text goes into the field. Through the clipboard a paragraph keeps its line breaks,
/// typed they become Shift+Enter. With `messages`, paragraphs (split at empty lines) go
/// one by one with Enter between them and none after the last, which stays in the field.
pub fn plan(text: &str, messages: bool, clipboard: bool) -> Vec<Step<'_>> {
    let paragraphs: Vec<&str> = if messages {
        split_paragraphs(text)
    } else {
        Vec::new()
    };
    // A dictated "абзац" alone has no paragraph to send: it goes in as line breaks.
    let paragraphs = if paragraphs.is_empty() {
        vec![text]
    } else {
        paragraphs
    };
    let mut steps = Vec::new();
    for (i, paragraph) in paragraphs.into_iter().enumerate() {
        if i > 0 {
            steps.push(Step::Send);
        }
        if clipboard {
            steps.push(Step::Text(paragraph));
            continue;
        }
        for (j, line) in paragraph.split('\n').enumerate() {
            if j > 0 {
                steps.push(Step::LineBreak);
            }
            if !line.is_empty() {
                steps.push(Step::Text(line));
            }
        }
    }
    steps
}

fn split_paragraphs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = find_blank_line(rest) {
        out.push(&rest[..at.0]);
        rest = &rest[at.1..];
    }
    out.push(rest);
    out.into_iter()
        .map(|p| p.trim_matches('\n'))
        .filter(|p| !p.trim().is_empty())
        .collect()
}

/// Start and end of the first run of line breaks with an empty line in it.
fn find_blank_line(text: &str) -> Option<(usize, usize)> {
    let start = text.find('\n')?;
    let mut end = start + 1;
    let bytes = text.as_bytes();
    while end < bytes.len() && (bytes[end] == b'\n' || bytes[end] == b' ') {
        end += 1;
    }
    if text[start..end].matches('\n').count() >= 2 {
        Some((start, end))
    } else {
        find_blank_line(&text[end..]).map(|(s, e)| (s + end, e + end))
    }
}

fn clipboard(slot: &mut Option<arboard::Clipboard>) -> Result<&mut arboard::Clipboard> {
    if slot.is_none() {
        *slot = Some(arboard::Clipboard::new().context("the clipboard is unavailable")?);
    }
    Ok(slot.as_mut().unwrap())
}

fn enigo() -> Result<Enigo> {
    Enigo::new(&enigo::Settings::default()).map_err(|e| anyhow!("cannot simulate keys: {e}"))
}

/// In hold mode the user may still be pressing Shift from the hotkey when the text is
/// ready, and Ctrl+Shift+V means something else in many apps. A synthetic release of a
/// key that is not down does nothing. The Windows key is never touched: a stray release
/// of it can open the Start menu.
fn release(enigo: &mut Enigo, keys: &[Key]) {
    for &key in keys {
        let _ = enigo.key(key, Direction::Release);
    }
}

fn held_modifiers(hotkey: &str) -> Vec<Key> {
    let mut keys = Vec::new();
    for part in hotkey.split('+').map(|p| p.trim().to_lowercase()) {
        let key = match part.as_str() {
            "shift" => Key::Shift,
            "ctrl" | "control" => Key::Control,
            "alt" | "option" => Key::Alt,
            "cmd" | "command" | "super" | "meta" if cfg!(target_os = "macos") => Key::Meta,
            _ => continue,
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// Wayland has no common way to fake input. `wtype` works on wlroots compositors (Sway,
/// Hyprland), `ydotool` anywhere but needs its daemon running.
fn wayland_tool(commands: &[&[&str]]) -> Result<()> {
    for command in commands {
        match Command::new(command[0]).args(&command[1..]).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).context(format!("cannot run {}", command[0])),
        }
    }
    bail!("pasting on Wayland needs wtype or ydotool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use Step::{LineBreak, Send, Text};

    #[test]
    fn typed_lines_break_with_shift_enter() {
        assert_eq!(
            plan("Раз.\nДва.", false, false),
            [Text("Раз."), LineBreak, Text("Два.")]
        );
        assert_eq!(
            plan("Раз.\n\nДва.", false, false),
            [Text("Раз."), LineBreak, LineBreak, Text("Два.")]
        );
    }

    #[test]
    fn clipboard_pastes_everything_at_once() {
        assert_eq!(plan("Раз.\n\nДва.", false, true), [Text("Раз.\n\nДва.")]);
    }

    #[test]
    fn paragraphs_as_messages() {
        assert_eq!(
            plan("Привет.\n\nКак дела?\nЖду.\n\nПока.", true, true),
            [
                Text("Привет."),
                Send,
                Text("Как дела?\nЖду."),
                Send,
                Text("Пока.")
            ]
        );
        assert_eq!(
            plan("Привет.\n\nКак дела?\nЖду.", true, false),
            [
                Text("Привет."),
                Send,
                Text("Как дела?"),
                LineBreak,
                Text("Жду.")
            ]
        );
    }

    #[test]
    fn no_enter_after_the_last_message() {
        assert_eq!(
            plan("Раз.\n\nДва. ", true, true),
            [Text("Раз."), Send, Text("Два. ")]
        );
        assert_eq!(plan("Один абзац.", true, true), [Text("Один абзац.")]);
        assert_eq!(plan("Раз.\n\n", true, true), [Text("Раз.")]);
    }

    #[test]
    fn spaces_on_the_empty_line_still_split() {
        assert_eq!(
            plan("Раз.\n \nДва.", true, true),
            [Text("Раз."), Send, Text("Два.")]
        );
    }

    #[test]
    fn a_lone_paragraph_break_is_not_a_send() {
        assert_eq!(plan("\n\n", true, false), [LineBreak, LineBreak]);
        assert_eq!(plan("\n\n", true, true), [Text("\n\n")]);
    }

    fn detect(vars: &[(&str, &str)]) -> DisplayServer {
        DisplayServer::from_env(|name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn modifiers_of_the_hotkey() {
        assert_eq!(
            held_modifiers("Ctrl+Shift+Space"),
            [Key::Control, Key::Shift]
        );
        assert_eq!(held_modifiers("alt + F9"), [Key::Alt]);
        assert_eq!(held_modifiers("Control+ctrl+K"), [Key::Control]);
        assert!(held_modifiers("F8").is_empty());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn windows_key_is_never_released() {
        assert!(held_modifiers("Super+Space").is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn detects_linux_sessions() {
        assert_eq!(
            detect(&[("XDG_SESSION_TYPE", "wayland")]),
            DisplayServer::Wayland
        );
        assert_eq!(detect(&[("XDG_SESSION_TYPE", "x11")]), DisplayServer::X11);
        assert_eq!(detect(&[("XDG_SESSION_TYPE", "X11")]), DisplayServer::X11);
        assert_eq!(
            detect(&[("WAYLAND_DISPLAY", "wayland-0")]),
            DisplayServer::Wayland
        );
        assert_eq!(detect(&[("WAYLAND_DISPLAY", "")]), DisplayServer::X11);
        assert_eq!(detect(&[]), DisplayServer::X11);
        assert_eq!(
            detect(&[
                ("XDG_SESSION_TYPE", "x11"),
                ("WAYLAND_DISPLAY", "wayland-0")
            ]),
            DisplayServer::X11
        );
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn other_systems_are_native() {
        assert_eq!(
            detect(&[("XDG_SESSION_TYPE", "wayland")]),
            DisplayServer::Native
        );
    }
}
