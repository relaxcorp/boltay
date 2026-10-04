use std::time::{Duration, Instant};

use crate::settings::HotkeyMode;

/// A tap shorter than this is a slip of the finger, not a dictation.
pub const MIN_RECORDING: Duration = Duration::from_millis(250);
/// With the latch on, a second press this soon after a tap records hands-free.
pub const DOUBLE_PRESS: Duration = Duration::from_millis(450);

/// Which hotkey: plain dictation or dictation translated to English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Dictate,
    Translate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// The recording belongs to the key that started it: only that key stops it.
    Recording {
        since: Instant,
        key: Key,
        /// Started by a double press in hold mode: goes on after the key is let go and
        /// stops at the next press.
        latched: bool,
    },
    Transcribing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    HotkeyPressed(Key),
    HotkeyReleased(Key),
    /// Esc during recording.
    Cancel,
    /// A system shortcut ran the app with a command: there is no release to wait for, the
    /// next one stops what this one started.
    Toggle(Key),
    /// The press that started a recording was part of a shortcut after all.
    Aborted(Key),
    /// Recording hit the length limit.
    Limit,
    /// Recognition and pasting are over, successfully or not.
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start(Key),
    Stop,
    Discard,
    None,
}

/// What the hotkey means right now. Pure, so every press sequence can be tested without
/// a keyboard or a microphone.
#[derive(Debug)]
pub struct Session {
    mode: HotkeyMode,
    latch: bool,
    phase: Phase,
    /// The OS repeats `pressed` while a key is held; only the first one counts.
    held: [bool; 2],
    /// The last tap too short to be a dictation: the first half of a double press.
    tap: Option<(Key, Instant)>,
}

impl Session {
    pub fn new(mode: HotkeyMode) -> Self {
        Self {
            mode,
            latch: false,
            phase: Phase::Idle,
            held: [false; 2],
            tap: None,
        }
    }

    pub fn set_latch(&mut self, latch: bool) {
        self.latch = latch;
    }

    #[cfg(test)]
    fn phase(&self) -> Phase {
        self.phase
    }

    /// Takes effect from the next recording.
    pub fn set_mode(&mut self, mode: HotkeyMode) {
        if self.phase == Phase::Idle {
            self.mode = mode;
        }
    }

    pub fn handle(&mut self, event: Event, now: Instant) -> Action {
        match event {
            Event::HotkeyPressed(key) => {
                let repeat = std::mem::replace(&mut self.held[key as usize], true);
                if repeat {
                    return Action::None;
                }
                match (self.phase, self.mode) {
                    (Phase::Idle, _) => {
                        let double = self.latch
                            && self.mode == HotkeyMode::Hold
                            && self.tap.take().is_some_and(|(k, at)| {
                                k == key && now.duration_since(at) <= DOUBLE_PRESS
                            });
                        self.start(key, now, double)
                    }
                    (
                        Phase::Recording {
                            since,
                            key: owner,
                            latched,
                        },
                        mode,
                    ) if owner == key && (latched || mode == HotkeyMode::Toggle) => {
                        self.stop(since, now)
                    }
                    _ => Action::None,
                }
            }
            Event::HotkeyReleased(key) => {
                self.held[key as usize] = false;
                match (self.phase, self.mode) {
                    (
                        Phase::Recording {
                            since,
                            key: owner,
                            latched: false,
                        },
                        HotkeyMode::Hold,
                    ) if owner == key => {
                        let action = self.stop(since, now);
                        if action == Action::Discard {
                            self.tap = Some((key, now));
                        }
                        action
                    }
                    _ => Action::None,
                }
            }
            Event::Toggle(key) => match self.phase {
                Phase::Idle => self.start(key, now, true),
                Phase::Recording {
                    since, key: owner, ..
                } if owner == key => self.stop(since, now),
                _ => Action::None,
            },
            Event::Cancel => match self.phase {
                Phase::Recording { .. } => {
                    self.phase = Phase::Idle;
                    Action::Discard
                }
                _ => Action::None,
            },
            Event::Aborted(key) => match self.phase {
                Phase::Recording {
                    key: owner,
                    latched: false,
                    ..
                } if owner == key => {
                    self.phase = Phase::Idle;
                    Action::Discard
                }
                _ => Action::None,
            },
            Event::Limit => match self.phase {
                Phase::Recording { since, .. } => self.stop(since, now),
                _ => Action::None,
            },
            Event::Finished => {
                if self.phase == Phase::Transcribing {
                    self.phase = Phase::Idle;
                }
                Action::None
            }
        }
    }

    fn start(&mut self, key: Key, now: Instant, latched: bool) -> Action {
        self.phase = Phase::Recording {
            since: now,
            key,
            latched,
        };
        Action::Start(key)
    }

    fn stop(&mut self, since: Instant, now: Instant) -> Action {
        if now.duration_since(since) < MIN_RECORDING {
            self.phase = Phase::Idle;
            Action::Discard
        } else {
            self.phase = Phase::Transcribing;
            Action::Stop
        }
    }
}

#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests {
    use super::*;
    use Action::{Discard, None, Stop};
    use Event::{Aborted, Cancel, Finished, HotkeyPressed, HotkeyReleased, Limit, Toggle};

    const Pressed: Event = HotkeyPressed(Key::Dictate);
    const Released: Event = HotkeyReleased(Key::Dictate);
    const TPressed: Event = HotkeyPressed(Key::Translate);
    const TReleased: Event = HotkeyReleased(Key::Translate);
    const Start: Action = Action::Start(Key::Dictate);

    struct Clock(Instant);

    impl Clock {
        fn new() -> Self {
            Self(Instant::now())
        }

        fn after(&mut self, ms: u64) -> Instant {
            self.0 += Duration::from_millis(ms);
            self.0
        }
    }

    fn run(mode: HotkeyMode, steps: &[(u64, Event)]) -> Vec<Action> {
        run_with(mode, false, steps)
    }

    fn run_with(mode: HotkeyMode, latch: bool, steps: &[(u64, Event)]) -> Vec<Action> {
        let mut session = Session::new(mode);
        session.set_latch(latch);
        let mut clock = Clock::new();
        steps
            .iter()
            .map(|&(ms, event)| session.handle(event, clock.after(ms)))
            .collect()
    }

    #[test]
    fn a_command_toggles_in_either_mode() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let actions = run(
                mode,
                &[(0, Toggle(Key::Dictate)), (2000, Toggle(Key::Dictate))],
            );
            assert_eq!(actions, [Start, Stop], "{mode:?}");
        }
    }

    #[test]
    fn a_command_recording_outlives_a_hotkey_release() {
        let actions = run(
            HotkeyMode::Hold,
            &[(0, Toggle(Key::Dictate)), (500, Released), (2000, Pressed)],
        );
        assert_eq!(actions, [Start, None, Stop]);
    }

    #[test]
    fn a_command_for_the_other_key_waits() {
        let actions = run(
            HotkeyMode::Hold,
            &[
                (0, Toggle(Key::Dictate)),
                (500, Toggle(Key::Translate)),
                (2000, Toggle(Key::Dictate)),
            ],
        );
        assert_eq!(actions, [Start, None, Stop]);
    }

    #[test]
    fn hold_records_while_held() {
        let actions = run(HotkeyMode::Hold, &[(0, Pressed), (2000, Released)]);
        assert_eq!(actions, [Start, Stop]);
    }

    #[test]
    fn hold_ignores_key_repeat() {
        let actions = run(
            HotkeyMode::Hold,
            &[
                (0, Pressed),
                (500, Pressed),
                (30, Pressed),
                (1000, Released),
            ],
        );
        assert_eq!(actions, [Start, None, None, Stop]);
    }

    #[test]
    fn hold_short_tap_is_discarded() {
        let actions = run(HotkeyMode::Hold, &[(0, Pressed), (100, Released)]);
        assert_eq!(actions, [Start, Discard]);
    }

    #[test]
    fn toggle_press_starts_second_press_stops() {
        let actions = run(
            HotkeyMode::Toggle,
            &[
                (0, Pressed),
                (100, Released),
                (3000, Pressed),
                (100, Released),
            ],
        );
        assert_eq!(actions, [Start, None, Stop, None]);
    }

    #[test]
    fn toggle_key_repeat_does_not_stop() {
        let actions = run(
            HotkeyMode::Toggle,
            &[(0, Pressed), (500, Pressed), (500, Pressed)],
        );
        assert_eq!(actions, [Start, None, None]);
    }

    #[test]
    fn toggle_quick_double_press_is_discarded() {
        let actions = run(
            HotkeyMode::Toggle,
            &[(0, Pressed), (50, Released), (50, Pressed)],
        );
        assert_eq!(actions, [Start, None, Discard]);
    }

    #[test]
    fn escape_cancels_in_both_modes() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let actions = run(mode, &[(0, Pressed), (1000, Cancel), (100, Released)]);
            assert_eq!(actions, [Start, Discard, None], "{mode:?}");
        }
    }

    #[test]
    fn escape_when_idle_does_nothing() {
        assert_eq!(run(HotkeyMode::Hold, &[(0, Cancel)]), [None]);
    }

    #[test]
    fn limit_stops_recording() {
        let actions = run(
            HotkeyMode::Hold,
            &[(0, Pressed), (300_000, Limit), (10, Released)],
        );
        assert_eq!(actions, [Start, Stop, None]);
    }

    #[test]
    fn busy_while_transcribing() {
        let mut session = Session::new(HotkeyMode::Toggle);
        let mut clock = Clock::new();
        session.handle(Pressed, clock.after(0));
        session.handle(Released, clock.after(10));
        assert_eq!(session.handle(Pressed, clock.after(1000)), Stop);
        session.handle(Released, clock.after(10));
        assert_eq!(session.phase(), Phase::Transcribing);
        assert_eq!(session.handle(Pressed, clock.after(10)), None);
        assert_eq!(session.handle(Cancel, clock.after(10)), None);
        session.handle(Released, clock.after(10));
        session.handle(Finished, clock.after(10));
        assert_eq!(session.phase(), Phase::Idle);
        assert_eq!(session.handle(Pressed, clock.after(10)), Start);
    }

    #[test]
    fn finished_when_idle_is_harmless() {
        let mut session = Session::new(HotkeyMode::Hold);
        session.handle(Finished, Instant::now());
        assert_eq!(session.phase(), Phase::Idle);
    }

    #[test]
    fn mode_changes_only_between_recordings() {
        let mut session = Session::new(HotkeyMode::Hold);
        let mut clock = Clock::new();
        session.handle(Pressed, clock.after(0));
        session.set_mode(HotkeyMode::Toggle);
        assert_eq!(session.handle(Released, clock.after(1000)), Stop);
        session.handle(Finished, clock.after(10));
        session.set_mode(HotkeyMode::Toggle);
        session.handle(Pressed, clock.after(10));
        assert_eq!(session.handle(Released, clock.after(1000)), None);
        assert!(matches!(session.phase(), Phase::Recording { .. }));
    }

    #[test]
    fn translate_key_starts_its_own_recording() {
        let actions = run(HotkeyMode::Hold, &[(0, TPressed), (2000, TReleased)]);
        assert_eq!(actions, [Action::Start(Key::Translate), Stop]);
    }

    #[test]
    fn the_other_key_does_not_stop_a_recording() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let actions = run(
                mode,
                &[
                    (0, TPressed),
                    (500, Pressed),
                    (100, Released),
                    (100, TReleased),
                ],
            );
            let expected = match mode {
                HotkeyMode::Hold => [Action::Start(Key::Translate), None, None, Stop],
                HotkeyMode::Toggle => [Action::Start(Key::Translate), None, None, None],
            };
            assert_eq!(actions, expected, "{mode:?}");
        }
    }

    #[test]
    fn keys_repeat_independently() {
        let actions = run(
            HotkeyMode::Toggle,
            &[
                (0, Pressed),
                (100, TPressed),
                (100, TPressed),
                (500, Pressed),
            ],
        );
        assert_eq!(actions, [Start, None, None, None]);
    }

    #[test]
    fn latch_double_press_records_hands_free() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (80, Released),
                (150, Pressed),
                (100, Released),
                (4000, Pressed),
                (100, Released),
            ],
        );
        assert_eq!(actions, [Start, Discard, Start, None, Stop, None]);
    }

    #[test]
    fn latch_off_double_press_is_two_taps() {
        let actions = run_with(
            HotkeyMode::Hold,
            false,
            &[
                (0, Pressed),
                (80, Released),
                (150, Pressed),
                (100, Released),
            ],
        );
        assert_eq!(actions, [Start, Discard, Start, Discard]);
    }

    #[test]
    fn latch_needs_a_quick_second_press() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (80, Released),
                (900, Pressed),
                (2000, Released),
            ],
        );
        assert_eq!(actions, [Start, Discard, Start, Stop]);
    }

    #[test]
    fn latch_keeps_plain_hold_working() {
        let actions = run_with(HotkeyMode::Hold, true, &[(0, Pressed), (2000, Released)]);
        assert_eq!(actions, [Start, Stop]);
    }

    #[test]
    fn latched_recording_is_cancelled_by_escape() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (80, Released),
                (150, Pressed),
                (100, Released),
                (2000, Cancel),
            ],
        );
        assert_eq!(actions, [Start, Discard, Start, None, Discard]);
    }

    #[test]
    fn latch_is_for_the_same_key() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (80, Released),
                (150, TPressed),
                (2000, TReleased),
            ],
        );
        assert_eq!(
            actions,
            [Start, Discard, Action::Start(Key::Translate), Stop]
        );
    }

    #[test]
    fn aborted_press_drops_only_its_own_recording() {
        let actions = run(
            HotkeyMode::Hold,
            &[(0, Pressed), (80, Aborted(Key::Dictate)), (100, Released)],
        );
        assert_eq!(actions, [Start, Discard, None]);
        let actions = run(
            HotkeyMode::Hold,
            &[
                (0, TPressed),
                (500, Aborted(Key::Dictate)),
                (900, TReleased),
            ],
        );
        assert_eq!(actions, [Action::Start(Key::Translate), None, Stop]);
    }

    #[test]
    fn aborted_press_leaves_a_latched_recording() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (80, Released),
                (150, Pressed),
                (100, Released),
                (2000, Aborted(Key::Dictate)),
            ],
        );
        assert_eq!(actions, [Start, Discard, Start, None, None]);
    }

    #[test]
    fn aborted_press_is_not_half_of_a_double_press() {
        let actions = run_with(
            HotkeyMode::Hold,
            true,
            &[
                (0, Pressed),
                (60, Aborted(Key::Dictate)),
                (20, Released),
                (150, Pressed),
                (2000, Released),
            ],
        );
        assert_eq!(actions, [Start, Discard, None, Start, Stop]);
    }

    #[test]
    fn release_without_press_does_nothing() {
        assert_eq!(run(HotkeyMode::Hold, &[(0, Released)]), [None]);
    }
}
