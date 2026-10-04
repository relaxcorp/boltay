use crate::hotkey::Solo;
use crate::session::{Event, Key};
use crate::settings::HotkeyMode;

/// Turns raw presses of single-key hotkeys into hotkey events. Right Ctrl and Shift are
/// also halves of everyday shortcuts, so a press only counts if nothing else goes down
/// while the key is held.
#[derive(Debug)]
pub struct Watcher {
    bound: [Option<Solo>; 2],
    mode: HotkeyMode,
    /// The bound key held down right now, by hotkey index.
    held: Option<usize>,
    /// Another key went down while it was held: a shortcut like right Ctrl+C, not ours.
    chord: bool,
}

impl Watcher {
    pub fn new(bound: [Option<Solo>; 2], mode: HotkeyMode) -> Self {
        Self {
            bound,
            mode,
            held: None,
            chord: false,
        }
    }

    pub fn is_bound(&self, solo: Solo) -> bool {
        self.bound.contains(&Some(solo))
    }

    /// A key or a mouse button went down; `None` for one that is not bound.
    pub fn down(&mut self, solo: Option<Solo>) -> Vec<Event> {
        let index = self.index(solo);
        match (self.held, index) {
            (Some(held), Some(index)) if held == index => Vec::new(),
            (Some(held), _) => {
                if std::mem::replace(&mut self.chord, true) {
                    return Vec::new();
                }
                match self.mode {
                    // The recording started on the press: drop it, it was never meant.
                    HotkeyMode::Hold => vec![Event::Aborted(key_at(held))],
                    HotkeyMode::Toggle => Vec::new(),
                }
            }
            (None, Some(index)) => {
                self.held = Some(index);
                self.chord = false;
                match self.mode {
                    HotkeyMode::Hold => vec![Event::HotkeyPressed(key_at(index))],
                    // Fires on release, once it is clear the key was pressed alone.
                    HotkeyMode::Toggle => Vec::new(),
                }
            }
            (None, None) => Vec::new(),
        }
    }

    pub fn up(&mut self, solo: Option<Solo>) -> Vec<Event> {
        let Some(index) = self.index(solo).filter(|&i| self.held == Some(i)) else {
            return Vec::new();
        };
        self.held = None;
        let key = key_at(index);
        match self.mode {
            HotkeyMode::Hold => vec![Event::HotkeyReleased(key)],
            HotkeyMode::Toggle if self.chord => Vec::new(),
            HotkeyMode::Toggle => vec![Event::HotkeyPressed(key), Event::HotkeyReleased(key)],
        }
    }

    /// Forgets a key held right now: its release will not reach us, while the hotkeys are
    /// suspended or after they change.
    pub fn reset(&mut self) -> Vec<Event> {
        let Some(index) = self.held.take() else {
            return Vec::new();
        };
        let key = key_at(index);
        match self.mode {
            HotkeyMode::Hold if self.chord => vec![Event::HotkeyReleased(key)],
            HotkeyMode::Hold => vec![Event::Aborted(key), Event::HotkeyReleased(key)],
            HotkeyMode::Toggle => Vec::new(),
        }
    }

    fn index(&self, solo: Option<Solo>) -> Option<usize> {
        let solo = solo?;
        self.bound.iter().position(|b| *b == Some(solo))
    }
}

fn key_at(index: usize) -> Key {
    [Key::Dictate, Key::Translate][index]
}

/// The translator key opens the plate on release, and only if nothing else went down
/// while it was held: right Ctrl+C stays a copy.
#[derive(Debug)]
pub struct Tap {
    key: Option<Solo>,
    held: bool,
    chord: bool,
}

impl Tap {
    pub fn new(key: Option<Solo>) -> Self {
        Self {
            key,
            held: false,
            chord: false,
        }
    }

    pub fn is_bound(&self, solo: Solo) -> bool {
        self.key == Some(solo)
    }

    pub fn down(&mut self, solo: Option<Solo>) {
        if solo.is_some() && solo == self.key {
            if !self.held {
                self.held = true;
                self.chord = false;
            }
        } else if self.held {
            self.chord = true;
        }
    }

    /// True when the plate should open.
    pub fn up(&mut self, solo: Option<Solo>) -> bool {
        if solo.is_none() || solo != self.key || !self.held {
            return false;
        }
        self.held = false;
        !self.chord
    }

    pub fn reset(&mut self) {
        self.held = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Event::{Aborted, HotkeyPressed, HotkeyReleased};
    use Key::{Dictate, Translate};

    const CTRL: Option<Solo> = Some(Solo::RightCtrl);
    const CAPS: Option<Solo> = Some(Solo::CapsLock);
    const OTHER: Option<Solo> = None;

    enum Step {
        Down(Option<Solo>),
        Up(Option<Solo>),
        Reset,
    }
    use Step::{Down, Reset, Up};

    fn run(mode: HotkeyMode, steps: &[Step]) -> Vec<Event> {
        let mut watcher = Watcher::new([CTRL, CAPS], mode);
        steps
            .iter()
            .flat_map(|step| match step {
                Down(s) => watcher.down(*s),
                Up(s) => watcher.up(*s),
                Reset => watcher.reset(),
            })
            .collect()
    }

    #[test]
    fn hold_presses_and_releases_right_away() {
        let events = run(HotkeyMode::Hold, &[Down(CTRL), Down(CTRL), Up(CTRL)]);
        assert_eq!(events, [HotkeyPressed(Dictate), HotkeyReleased(Dictate)]);
    }

    #[test]
    fn hold_shortcut_with_the_key_is_aborted_once() {
        let events = run(
            HotkeyMode::Hold,
            &[Down(CTRL), Down(OTHER), Up(OTHER), Down(OTHER), Up(CTRL)],
        );
        assert_eq!(
            events,
            [
                HotkeyPressed(Dictate),
                Aborted(Dictate),
                HotkeyReleased(Dictate)
            ]
        );
    }

    #[test]
    fn second_bound_key_while_one_is_held_is_a_chord() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let events = run(mode, &[Down(CTRL), Down(CAPS), Up(CAPS), Up(CTRL)]);
            let expected: &[Event] = match mode {
                HotkeyMode::Hold => &[
                    HotkeyPressed(Dictate),
                    Aborted(Dictate),
                    HotkeyReleased(Dictate),
                ],
                HotkeyMode::Toggle => &[],
            };
            assert_eq!(events, expected, "{mode:?}");
        }
    }

    #[test]
    fn keys_work_again_after_a_chord() {
        let events = run(
            HotkeyMode::Hold,
            &[Down(CTRL), Down(CAPS), Up(CTRL), Up(CAPS), Down(CAPS)],
        );
        assert_eq!(events.last(), Some(&HotkeyPressed(Translate)));
    }

    #[test]
    fn toggle_fires_on_release_of_a_lone_press() {
        let events = run(HotkeyMode::Toggle, &[Down(CTRL), Down(CTRL), Up(CTRL)]);
        assert_eq!(events, [HotkeyPressed(Dictate), HotkeyReleased(Dictate)]);
    }

    #[test]
    fn toggle_shortcut_with_the_key_does_nothing() {
        let events = run(HotkeyMode::Toggle, &[Down(CTRL), Down(OTHER), Up(CTRL)]);
        assert_eq!(events, []);
    }

    #[test]
    fn other_keys_alone_are_ignored() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            assert_eq!(run(mode, &[Down(OTHER), Up(OTHER), Up(CTRL)]), []);
        }
    }

    #[test]
    fn reset_drops_a_held_key() {
        let events = run(HotkeyMode::Hold, &[Down(CTRL), Reset, Up(CTRL)]);
        assert_eq!(
            events,
            [
                HotkeyPressed(Dictate),
                Aborted(Dictate),
                HotkeyReleased(Dictate)
            ]
        );
        assert_eq!(run(HotkeyMode::Toggle, &[Down(CTRL), Reset, Up(CTRL)]), []);
        assert_eq!(run(HotkeyMode::Hold, &[Reset]), []);
    }

    #[test]
    fn reset_after_a_chord_only_releases() {
        let events = run(HotkeyMode::Hold, &[Down(CTRL), Down(OTHER), Reset]);
        assert_eq!(
            events,
            [
                HotkeyPressed(Dictate),
                Aborted(Dictate),
                HotkeyReleased(Dictate)
            ]
        );
    }

    #[test]
    fn tap_fires_on_a_lone_release() {
        let mut tap = Tap::new(CAPS);
        tap.down(CAPS);
        tap.down(CAPS);
        assert!(tap.up(CAPS));
        assert!(!tap.up(CAPS));
    }

    #[test]
    fn tap_with_another_key_is_a_shortcut() {
        let mut tap = Tap::new(CTRL);
        tap.down(CTRL);
        tap.down(OTHER);
        assert!(!tap.up(CTRL));
        tap.down(CTRL);
        assert!(tap.up(CTRL));
    }

    #[test]
    fn tap_ignores_other_keys_and_resets() {
        let mut tap = Tap::new(CTRL);
        tap.down(CAPS);
        assert!(!tap.up(CAPS));
        tap.down(CTRL);
        tap.reset();
        assert!(!tap.up(CTRL));
        let mut unbound = Tap::new(None);
        unbound.down(OTHER);
        assert!(!unbound.up(OTHER));
    }
}
