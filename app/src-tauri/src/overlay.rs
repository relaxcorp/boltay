use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
};

use crate::settings::OverlayPosition;
use crate::Shared;

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 300.0;
const HEIGHT: f64 = 64.0;
/// Room for the error details, opened on request.
const EXPANDED_HEIGHT: f64 = 210.0;
/// Distance from the bottom of the screen, clear of the taskbar and the Dock.
const BOTTOM_MARGIN: f64 = 96.0;
const TOP_MARGIN: f64 = 24.0;
/// Gap between the text cursor and the badge.
const CARET_GAP: f64 = 10.0;

/// The paste could not go through, but a Ctrl+V or Cmd+V from the user will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Copied {
    /// The window runs as administrator.
    Elevated,
    /// macOS has not let the app send keys: Accessibility is off.
    NoAccess,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum State {
    Listening {
        translate: bool,
    },
    /// The translation model is downloading on first use; the page shows the progress.
    Fetching {
        id: &'static str,
    },
    /// That download is over: ready to translate, or why not.
    Fetched {
        error: Option<String>,
    },
    Recognizing,
    /// The model is still loading: the first dictation after launch or a language switch.
    Loading,
    Done,
    /// Nothing but silence was recorded.
    Empty,
    /// The text is saved but may not have reached the app: offer to copy it.
    Unsent {
        text: String,
        reason: String,
        /// Why the text is already on the clipboard to be pasted by hand, if it is.
        copied: Option<Copied>,
    },
    Error {
        key: ErrorKind,
        detail: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
    Microphone,
    Model,
    Recognition,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Level {
    pub level: f32,
    pub elapsed_ms: u64,
}

/// What the overlay shows. A freshly created overlay page asks for it on load, since
/// events sent before its script runs are lost.
static CURRENT: Mutex<Option<State>> = Mutex::new(None);
static EXPANDED: AtomicBool = AtomicBool::new(false);
/// The text cursor the badge sits next to, in screen pixels: x, top, bottom.
static ANCHOR: Mutex<Option<Anchor>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct Anchor {
    x: f64,
    top: f64,
    bottom: f64,
}

pub fn current() -> Option<State> {
    CURRENT.lock().unwrap().clone()
}

/// A small badge over other windows. It never takes focus: the dictation is pasted into
/// whatever window is active, and an overlay that grabbed focus would receive it instead.
fn create(app: &AppHandle) -> Result<WebviewWindow> {
    let window =
        tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
            .title("Boltay")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .focusable(false)
            .focused(false)
            // macOS spends the first click on an inactive app's window on activating it:
            // "Copy" would need two.
            .accept_first_mouse(true)
            .visible(false)
            .build()?;
    click_through(&window, true);
    Ok(window)
}

pub fn show(app: &AppHandle, state: State) {
    *CURRENT.lock().unwrap() = Some(state.clone());
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => match create(app) {
            Ok(window) => window,
            Err(e) => {
                log::error!("overlay: {e}");
                return;
            }
        },
    };
    let _ = window.emit_to(LABEL, "overlay", &state);
    let position = app.state::<Shared>().settings().overlay_position;
    *ANCHOR.lock().unwrap() = (position == OverlayPosition::Cursor)
        .then(|| platform::caret().or_else(|| pointer(&window)))
        .flatten();
    if EXPANDED.swap(false, Ordering::Relaxed) {
        let _ = window.set_size(LogicalSize::new(WIDTH, HEIGHT));
    }
    place(&window, HEIGHT);
    platform::show(&window);
    click_through(&window, !interactive(&state));
}

/// Grows the overlay upwards to show why something failed.
pub fn expand(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        EXPANDED.store(true, Ordering::Relaxed);
        let _ = window.set_size(LogicalSize::new(WIDTH, EXPANDED_HEIGHT));
        place(&window, EXPANDED_HEIGHT);
    }
}

pub fn update(app: &AppHandle, state: State) {
    if let Some(window) = app.get_webview_window(LABEL) {
        set_state(&window, &state);
    }
}

fn set_state(window: &WebviewWindow, state: &State) {
    *CURRENT.lock().unwrap() = Some(state.clone());
    click_through(window, !interactive(state));
    let _ = window.emit_to(LABEL, "overlay", state);
}

/// Only the buttons of a failure need clicks, the rest of the time they go to the app below.
fn interactive(state: &State) -> bool {
    matches!(
        state,
        State::Unsent { .. } | State::Error { .. } | State::Fetched { error: Some(_) }
    )
}

fn click_through(window: &WebviewWindow, on: bool) {
    // GTK has no native window to reshape until the overlay is first shown, and tao
    // panics on a hidden one.
    if cfg!(target_os = "linux") && !window.is_visible().unwrap_or(false) {
        return;
    }
    let _ = window.set_ignore_cursor_events(on);
    platform::style(window, on);
}

pub fn level(app: &AppHandle, level: Level) {
    let _ = app.emit_to(LABEL, "overlay-level", level);
}

/// Drops a hidden overlay: its webview holds a hundred megabytes or more, too much to keep
/// between dictations that may be hours apart.
pub fn retire(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        if !window.is_visible().unwrap_or(true) {
            let _ = window.destroy();
        }
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        click_through(&window, true);
        platform::hide(&window);
    }
}

/// At the bottom or the top of the screen the mouse is on, or next to the text cursor: under
/// it when there is room, above it otherwise.
fn place(window: &WebviewWindow, height: f64) {
    let anchor = *ANCHOR.lock().unwrap();
    let position = window.state::<Shared>().settings().overlay_position;
    let point = anchor
        .map(|a| (a.x, a.top))
        .or_else(|| window.cursor_position().ok().map(|p| (p.x, p.y)));
    let monitor = point
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);
    let width = WIDTH * scale;
    let height = height * scale;
    let centre = left + (area.size.width as f64 - width) / 2.0;
    let (x, y) = match (position, anchor) {
        (OverlayPosition::Cursor, Some(a)) => {
            let gap = CARET_GAP * scale;
            let below = a.bottom + gap;
            let y = if below + height <= bottom {
                below
            } else {
                a.top - gap - height
            };
            (a.x - 16.0 * scale, y)
        }
        (OverlayPosition::Top, _) => (centre, top + TOP_MARGIN * scale),
        _ => (centre, bottom - height - BOTTOM_MARGIN * scale),
    };
    let x = x.clamp(left, (right - width).max(left));
    let y = y.clamp(top, (bottom - height).max(top));
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Shows a window on top without taking focus from the app the user is in.
pub fn show_inactive(window: &WebviewWindow) {
    platform::show(window);
}

/// Hides a window shown with [`show_inactive`]. Tauri did not see that show, so its own
/// `hide` takes the window for hidden already and does nothing.
pub fn hide_inactive(window: &WebviewWindow) {
    platform::hide(window);
}

/// A floating window that must also show over a full-screen app, as the translator does.
pub fn everywhere(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    platform::everywhere(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// The text cursor of the focused app, or the mouse pointer when the app does not show
/// its caret: x, top and bottom in screen pixels.
pub fn caret_or_pointer(window: &WebviewWindow) -> Option<(f64, f64, f64)> {
    platform::caret()
        .or_else(|| pointer(window))
        .map(|a| (a.x, a.top, a.bottom))
}

fn pointer(window: &WebviewWindow) -> Option<Anchor> {
    let p = window.cursor_position().ok()?;
    Some(Anchor {
        x: p.x,
        top: p.y,
        bottom: p.y + 18.0,
    })
}

#[cfg(windows)]
mod platform {
    use tauri::WebviewWindow;
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW, GetWindowThreadProcessId,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, GUITHREADINFO, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    /// The caret of the focused window, if that app keeps a system caret. Browsers and
    /// Electron apps draw their own and leave this empty.
    pub fn caret() -> Option<super::Anchor> {
        unsafe {
            let front = GetForegroundWindow();
            if front.is_null() {
                return None;
            }
            let thread = GetWindowThreadProcessId(front, std::ptr::null_mut());
            let mut info: GUITHREADINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread, &mut info) == 0 || info.hwndCaret.is_null() {
                return None;
            }
            let rect = info.rcCaret;
            if rect.bottom <= rect.top {
                return None;
            }
            let mut point = POINT {
                x: rect.left,
                y: rect.top,
            };
            if ClientToScreen(info.hwndCaret, &mut point) == 0 {
                return None;
            }
            Some(super::Anchor {
                x: point.x as f64,
                top: point.y as f64,
                bottom: (point.y + rect.bottom - rect.top) as f64,
            })
        }
    }

    fn hwnd(window: &WebviewWindow) -> Option<windows_sys::Win32::Foundation::HWND> {
        window.hwnd().ok().map(|h| h.0 as _)
    }

    /// Tauri recomputes the extended style whenever a window flag changes, so the bits are
    /// set again after every change: never activated, no taskbar button, and clicks pass
    /// through unless the copy button is up.
    pub fn style(window: &WebviewWindow, click_through: bool) {
        let Some(hwnd) = hwnd(window) else { return };
        unsafe {
            let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            style |= (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED) as isize;
            if click_through {
                style |= WS_EX_TRANSPARENT as isize;
            } else {
                style &= !(WS_EX_TRANSPARENT as isize);
            }
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        }
    }

    /// `ShowWindow(SW_SHOW)`, which Tauri uses, activates the window even with
    /// `WS_EX_NOACTIVATE`. Showing it ourselves keeps the focus where it is.
    pub fn show(window: &WebviewWindow) {
        let Some(hwnd) = hwnd(window) else { return };
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn hide(window: &WebviewWindow) {
        if let Some(hwnd) = hwnd(window) {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    use tauri::WebviewWindow;

    pub fn caret() -> Option<super::Anchor> {
        None
    }

    /// Clicks passing through are Tauri's.
    pub fn style(window: &WebviewWindow, _click_through: bool) {
        everywhere(window);
    }

    /// On every desktop and over full-screen apps, out of the Cmd+` window cycle.
    pub fn everywhere(window: &WebviewWindow) {
        with_ns_window(window, |w| {
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            )
        });
    }

    /// `orderFrontRegardless` puts the window on top without making it key and without
    /// activating the app, so the frontmost app keeps its text cursor.
    pub fn show(window: &WebviewWindow) {
        with_ns_window(window, |w| w.orderFrontRegardless());
    }

    pub fn hide(window: &WebviewWindow) {
        with_ns_window(window, |w| w.orderOut(None));
    }

    fn with_ns_window(window: &WebviewWindow, f: fn(&NSWindow)) {
        let Ok(ptr) = window.ns_window() else { return };
        let ptr = ptr as usize;
        let _ = window.run_on_main_thread(move || {
            // SAFETY: Tauri hands out a valid NSWindow pointer for the window's lifetime,
            // and AppKit is only touched on the main thread.
            let ns_window = unsafe { &*(ptr as *const NSWindow) };
            f(ns_window);
        });
    }
}

/// X11 window managers honour "do not accept focus", Wayland compositors decide on their
/// own, and on some of them the overlay may still take focus.
#[cfg(all(not(windows), not(target_os = "macos")))]
mod platform {
    use tauri::WebviewWindow;

    pub fn caret() -> Option<super::Anchor> {
        None
    }

    pub fn style(_window: &WebviewWindow, _click_through: bool) {}

    pub fn show(window: &WebviewWindow) {
        let _ = window.show();
    }

    pub fn hide(window: &WebviewWindow) {
        let _ = window.hide();
    }
}
