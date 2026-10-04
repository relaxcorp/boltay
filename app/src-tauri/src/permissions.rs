use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub enum Access {
    Granted,
    Denied,
    /// Not asked yet: the system shows its prompt on first use.
    Unknown,
}

/// What dictation needs from the system. Only macOS gates the microphone and synthetic
/// keystrokes behind user consent; elsewhere both are always there.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Permissions {
    pub needed: bool,
    pub microphone: Access,
    pub accessibility: Access,
}

#[cfg(not(target_os = "macos"))]
pub fn check() -> Permissions {
    Permissions {
        needed: false,
        microphone: Access::Granted,
        accessibility: Access::Granted,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone() {}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility() {}

#[cfg(target_os = "macos")]
pub use macos::{check, request_accessibility, request_microphone};

/// Something dictation needs is not granted yet.
pub fn missing() -> bool {
    let permissions = check();
    permissions.needed
        && (permissions.microphone != Access::Granted
            || permissions.accessibility != Access::Granted)
}

/// macOS records silence from a microphone the app may not use. Not asked yet: the system
/// asks now, and the next dictation has the answer.
pub fn ready_to_record() -> Result<(), String> {
    match check().microphone {
        Access::Granted => Ok(()),
        Access::Unknown => {
            request_microphone();
            Err("waiting for the microphone permission".into())
        }
        Access::Denied => Err("the microphone is off for Boltay in System Settings".into()),
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    use super::{Access, Permissions};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: &NSDictionary<NSString, NSNumber>) -> bool;
        static kAXTrustedCheckOptionPrompt: &'static NSString;
    }

    /// Shows the system prompt that leads to the Accessibility list, where the app is then
    /// already listed: opening the list alone makes the user find the app and add it.
    pub fn request_accessibility() {
        // SAFETY: a dictionary with one constant key, passed for the duration of the call.
        unsafe {
            let options = NSDictionary::from_slices(
                &[kAXTrustedCheckOptionPrompt],
                &[&*NSNumber::new_bool(true)],
            );
            AXIsProcessTrustedWithOptions(&options);
        }
    }

    pub fn check() -> Permissions {
        Permissions {
            needed: true,
            microphone: microphone(),
            // SAFETY: a plain query without arguments.
            accessibility: if unsafe { AXIsProcessTrusted() } {
                Access::Granted
            } else {
                Access::Denied
            },
        }
    }

    fn microphone() -> Access {
        // SAFETY: AVMediaTypeAudio is a constant string provided by AVFoundation.
        let Some(media) = (unsafe { AVMediaTypeAudio }) else {
            return Access::Unknown;
        };
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) };
        match status {
            AVAuthorizationStatus::Authorized => Access::Granted,
            AVAuthorizationStatus::NotDetermined => Access::Unknown,
            _ => Access::Denied,
        }
    }

    /// Shows the system prompt if the user has not decided yet. After a refusal only
    /// System Settings can change it.
    pub fn request_microphone() {
        let Some(media) = (unsafe { AVMediaTypeAudio }) else {
            return;
        };
        let done = RcBlock::new(|_granted: Bool| {});
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &done) };
    }
}

/// Without Accessibility on macOS the synthetic Cmd+V is silently dropped, so the text
/// would look pasted while going nowhere.
pub fn ready_to_paste() -> Result<(), String> {
    match check().accessibility {
        Access::Denied => Err("Accessibility permission is not granted".into()),
        _ => Ok(()),
    }
}

/// System Settings pages the onboarding links to.
pub const MICROPHONE_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
pub const ACCESSIBILITY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
