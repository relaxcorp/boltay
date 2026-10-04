use std::ptr::null;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::{GetLastError, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    VK_CAPITAL, VK_LCONTROL, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION,
    KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    WM_XBUTTONDOWN,
};

use crate::dictation::Msg;
use crate::hotkey::{Solo, TRANSLATOR};
use crate::session::Event;
use crate::settings::HotkeyMode;
use crate::solo::{Tap, Watcher};
use crate::Shared;

struct Watch {
    app: AppHandle,
    watcher: Watcher,
    translator: Tap,
}

static WATCH: Mutex<Option<Watch>> = Mutex::new(None);
static SUSPENDED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);

/// A lone right Ctrl, Alt, Shift or CapsLock cannot be a system hotkey, so they
/// are watched with low-level hooks on a thread of their own. The hooks are installed on
/// first use and stay: with nothing bound they only pass events on. If Windows refuses
/// them, the next call tries again.
pub fn start() -> Result<()> {
    if STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let (tx, rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("input hooks".into())
        .spawn(move || run(tx))
        .map_err(|e| anyhow!("cannot start the input hook thread: {e}"))?;
    let installed = rx
        .recv()
        .unwrap_or_else(|_| Err(anyhow!("the input hook thread is gone")));
    if installed.is_err() {
        STARTED.store(false, Ordering::SeqCst);
    }
    installed
}

pub fn watch(app: &AppHandle, bound: [Option<Solo>; 3], mode: HotkeyMode) {
    let mut guard = WATCH.lock().unwrap();
    // A key held through the change will not be released for the new watcher.
    if let Some(old) = guard.as_mut() {
        send(app, old.watcher.reset());
    }
    *guard = Some(Watch {
        app: app.clone(),
        watcher: Watcher::new([bound[0], bound[1]], mode),
        translator: Tap::new(bound[TRANSLATOR]),
    });
}

pub fn suspend(suspended: bool) {
    SUSPENDED.store(suspended, Ordering::Relaxed);
    if suspended {
        if let Some(w) = WATCH.lock().unwrap().as_mut() {
            send(&w.app, w.watcher.reset());
            w.translator.reset();
        }
    }
}

fn run(installed: mpsc::SyncSender<Result<()>>) {
    // Low-level hooks are called on the thread that installed them, through its message loop.
    unsafe {
        let module = GetModuleHandleW(null());
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_keyboard), module, 0);
        let keyboard_error = GetLastError();
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), module, 0);
        let mouse_error = GetLastError();
        if keyboard.is_null() || mouse.is_null() {
            for hook in [keyboard, mouse] {
                if !hook.is_null() {
                    UnhookWindowsHookEx(hook);
                }
            }
            let code = if keyboard.is_null() {
                keyboard_error
            } else {
                mouse_error
            };
            log::error!("cannot install input hooks, error {code}");
            let _ = installed.send(Err(anyhow!(
                "Windows did not let the app watch single keys and mouse buttons (error {code})"
            )));
            return;
        }
        let _ = installed.send(Ok(()));
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
    }
}

unsafe extern "system" fn on_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        // Keys we type ourselves when pasting are not the user's.
        if info.flags & LLKHF_INJECTED == 0 && key(info.vkCode, info.scanCode, down) {
            return 1;
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// Mouse buttons are never hotkeys, a click only tells that a held key is part of a
/// shortcut, as in right Ctrl + click on a link. Nothing is swallowed here.
unsafe extern "system" fn on_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = &*(lparam as *const MSLLHOOKSTRUCT);
        let pressed = matches!(
            wparam as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        );
        if pressed && info.flags & LLMHF_INJECTED == 0 {
            input(None, true);
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn solo_of(vk: u32) -> Option<Solo> {
    match vk as u16 {
        VK_RCONTROL => Some(Solo::RightCtrl),
        VK_RMENU => Some(Solo::RightAlt),
        VK_RSHIFT => Some(Solo::RightShift),
        VK_CAPITAL => Some(Solo::CapsLock),
        _ => None,
    }
}

/// Whether the key is swallowed: CapsLock bound to dictation must not also switch case.
/// Right Ctrl, Alt and Shift keep doing their job in shortcuts.
fn key(vk: u32, scan: u32, down: bool) -> bool {
    // AltGr on European layouts comes with a made-up left Ctrl, scan code 0x21D.
    if vk as u16 == VK_LCONTROL && scan == 0x21D {
        return false;
    }
    let solo = solo_of(vk);
    input(solo, down) && solo == Some(Solo::CapsLock)
}

/// Feeds the watcher; true if the key is bound.
fn input(solo: Option<Solo>, down: bool) -> bool {
    if SUSPENDED.load(Ordering::Relaxed) {
        return false;
    }
    let mut guard = WATCH.lock().unwrap();
    let Some(w) = guard.as_mut() else {
        return false;
    };
    let solo = solo.filter(|&s| w.watcher.is_bound(s) || w.translator.is_bound(s));
    // The translator key is just another key to the dictation watcher, and the other way
    // round: holding one while pressing the other is a chord for both.
    let dictation = solo.filter(|&s| w.watcher.is_bound(s));
    let events = if down {
        w.translator.down(solo);
        w.watcher.down(dictation)
    } else {
        if w.translator.up(solo) {
            crate::translator::toggle(&w.app);
        }
        w.watcher.up(dictation)
    };
    send(&w.app, events);
    solo.is_some()
}

fn send(app: &AppHandle, events: Vec<Event>) {
    let shared = app.state::<Shared>();
    for event in events {
        shared.send(Msg::Event(event));
    }
}
