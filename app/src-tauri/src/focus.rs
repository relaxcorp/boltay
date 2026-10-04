use std::thread;
use std::time::{Duration, Instant};

/// The window that had focus when the recording started: the dictation goes there. WebView2
/// can take focus while the overlay is being created, even though that window never
/// activates, and a Ctrl+V sent then lands in the overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target(platform::Handle);

pub fn current() -> Option<Target> {
    platform::foreground().map(Target)
}

impl Target {
    /// One of the app's own windows.
    pub fn is_ours(self) -> bool {
        platform::is_ours(self.0)
    }
}

/// Gives focus back to the target if one of our own windows took it. A switch to another
/// app is the user's doing and stays.
pub fn reclaim(target: Target) {
    if let Some(now) = platform::foreground() {
        if now != target.0 && platform::is_ours(now) {
            log::warn!("focus moved to our window, giving it back");
            restore(target);
        }
    }
}

/// Brings the target to the front. False if it is gone or would not take focus.
pub fn restore(target: Target) -> bool {
    if platform::foreground() == Some(target.0) {
        return true;
    }
    if !platform::activate(target.0) {
        return false;
    }
    // Activation is asynchronous everywhere.
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if platform::foreground() == Some(target.0) {
            // Let the window settle before keys arrive, some apps drop the first ones.
            thread::sleep(Duration::from_millis(50));
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

/// The target runs with more rights than we do: Windows drops the keys we send it (UIPI).
pub fn is_above_us(target: Target) -> bool {
    platform::is_above_us(target.0)
}

/// Integrity levels decide when both can be read. An anti-cheat keeps a game's process
/// closed to everyone, so a closed process says nothing: then a message posted to the
/// window tells, UIPI refuses it only from below.
#[cfg_attr(not(windows), allow(dead_code))]
fn above(ours: Option<u32>, theirs: Option<u32>, probe_refused: impl FnOnce() -> bool) -> bool {
    match (ours, theirs) {
        (Some(ours), Some(theirs)) => theirs > ours,
        _ => probe_refused(),
    }
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, HANDLE, HWND,
    };
    use windows_sys::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcess,
        OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
        IsWindowVisible, PostMessageW, RegisterWindowMessageW, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    /// An HWND, kept as a number so it can cross threads.
    pub type Handle = isize;

    pub fn foreground() -> Option<Handle> {
        let hwnd = unsafe { GetForegroundWindow() };
        (!hwnd.is_null()).then_some(hwnd as Handle)
    }

    pub fn is_ours(handle: Handle) -> bool {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(handle as HWND, &mut pid) };
        pid == unsafe { GetCurrentProcessId() }
    }

    pub fn is_above_us(handle: Handle) -> bool {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(handle as HWND, &mut pid) };
        let ours = integrity(unsafe { GetCurrentProcess() });
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        let theirs = (!process.is_null()).then(|| integrity(process)).flatten();
        if !process.is_null() {
            unsafe { CloseHandle(process) };
        }
        super::above(ours, theirs, || probe_refused(handle))
    }

    /// Posts a message of our own that no app acts on.
    fn probe_refused(handle: Handle) -> bool {
        let name: Vec<u16> = "BoltayProbe".encode_utf16().chain([0]).collect();
        unsafe {
            let message = RegisterWindowMessageW(name.as_ptr());
            message != 0
                && PostMessageW(handle as HWND, message, 0, 0) == 0
                && GetLastError() == ERROR_ACCESS_DENIED
        }
    }

    /// The mandatory integrity level of a process: what UIPI compares.
    fn integrity(process: HANDLE) -> Option<u32> {
        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
                return None;
            }
            let mut buffer = [0u64; 16];
            let mut size = 0;
            let ok = GetTokenInformation(
                token,
                TokenIntegrityLevel,
                buffer.as_mut_ptr().cast(),
                std::mem::size_of_val(&buffer) as u32,
                &mut size,
            );
            CloseHandle(token);
            if ok == 0 {
                return None;
            }
            let label = &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL);
            let sid = label.Label.Sid;
            let count = *GetSidSubAuthorityCount(sid);
            Some(*GetSidSubAuthority(sid, u32::from(count).checked_sub(1)?))
        }
    }

    /// Windows lets a process take the foreground only from the one that has it, so the
    /// call runs with our input attached to the current foreground thread.
    pub fn activate(handle: Handle) -> bool {
        let hwnd = handle as HWND;
        unsafe {
            // Telegram and the new Notepad hide their window on close instead of destroying
            // it: a hidden window is as gone as a closed one.
            if IsWindow(hwnd) == 0 || IsWindowVisible(hwnd) == 0 {
                return false;
            }
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            let me = GetCurrentThreadId();
            let front = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let attached = front != 0 && front != me && AttachThreadInput(me, front, 1) != 0;
            BringWindowToTop(hwnd);
            let done = SetForegroundWindow(hwnd) != 0;
            if attached {
                AttachThreadInput(me, front, 0);
            }
            done
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

    /// A process id: macOS focuses apps, and an app brings back its own key window.
    pub type Handle = i32;

    pub fn foreground() -> Option<Handle> {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
            .filter(|&pid| pid > 0)
    }

    pub fn is_ours(handle: Handle) -> bool {
        handle == std::process::id() as i32
    }

    pub fn is_above_us(_: Handle) -> bool {
        false
    }

    /// macOS 13 needs "ignoring other apps" to take the focus from us. From 14 on the
    /// flag does nothing and only the active app can hand the focus over: from one of our
    /// windows that works, from another app's it fails and the text is offered to copy.
    #[allow(deprecated)]
    pub fn activate(handle: Handle) -> bool {
        NSRunningApplication::runningApplicationWithProcessIdentifier(handle).is_some_and(|app| {
            app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
        })
    }
}

/// X11 through xdotool, when it is installed. Wayland does not let one client focus
/// another, there the compositor keeps focus where the user left it.
#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use std::process::Command;

    /// An X11 window id.
    pub type Handle = u64;

    fn xdotool(args: &[&str]) -> Option<String> {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return None;
        }
        let out = Command::new("xdotool").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn foreground() -> Option<Handle> {
        xdotool(&["getactivewindow"])?.parse().ok()
    }

    pub fn is_ours(handle: Handle) -> bool {
        xdotool(&["getwindowpid", &handle.to_string()]).and_then(|pid| pid.parse::<u32>().ok())
            == Some(std::process::id())
    }

    pub fn activate(handle: Handle) -> bool {
        xdotool(&["windowactivate", &handle.to_string()]).is_some()
    }

    pub fn is_above_us(_: Handle) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEDIUM: u32 = 0x2000;
    const HIGH: u32 = 0x3000;

    #[test]
    fn known_levels_decide_without_a_probe() {
        let probe = || panic!("no probe when both levels are known");
        assert!(above(Some(MEDIUM), Some(HIGH), probe));
        assert!(!above(Some(MEDIUM), Some(MEDIUM), probe));
        assert!(!above(Some(HIGH), Some(MEDIUM), probe));
    }

    #[test]
    fn closed_process_goes_by_the_probe() {
        // A game under an anti-cheat: the process is closed, the window takes messages.
        assert!(!above(Some(MEDIUM), None, || false));
        // An elevated window: closed and refusing.
        assert!(above(Some(MEDIUM), None, || true));
        assert!(above(None, None, || true));
    }
}
