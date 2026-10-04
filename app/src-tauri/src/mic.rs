use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::Capture;
use crate::Shared;

/// Feeds the level meter next to the microphone choice. Opens the microphone only while the
/// settings window shows the meter.
#[derive(Default)]
pub struct Monitor(Mutex<Option<Arc<AtomicBool>>>);

#[derive(Clone, Serialize)]
struct Level {
    /// Loudness after the software gain, 0..=1.
    rms: f32,
}

impl Monitor {
    /// Restarts on the current microphone and gain.
    pub fn start(&self, app: &AppHandle) {
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.0.lock().unwrap().replace(stop.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        let app = app.clone();
        thread::spawn(move || {
            let settings = app.state::<Shared>().settings();
            // Nothing is kept: one second of buffer is the least a capture takes.
            let capture = match Capture::start(settings.microphone.as_deref(), 1) {
                Ok(capture) => capture,
                Err(e) => {
                    log::warn!("level meter: {e:#}");
                    return;
                }
            };
            let gain = 10f32.powf(settings.mic_gain_db / 20.0);
            while !stop.load(Ordering::Relaxed) {
                let level = Level {
                    rms: (capture.rms() * gain).min(1.0),
                };
                let _ = app.emit_to(crate::SETTINGS_WINDOW, "mic-level", level);
                thread::sleep(Duration::from_millis(60));
            }
        });
    }

    pub fn stop(&self) {
        if let Some(stop) = self.0.lock().unwrap().take() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// The recording level Windows applies to the microphone, 0..=1. A level left at 30 % in
/// the sound settings makes speech too quiet to recognize, no software gain fixes that well.
#[cfg(windows)]
pub mod volume {
    use windows::core::Result;
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
        STGM_READ,
    };

    pub fn get(device: Option<String>) -> Option<f32> {
        on_com_thread(move || unsafe { endpoint(device.as_deref())?.GetMasterVolumeLevelScalar() })
            .ok()
    }

    pub fn set(device: Option<String>, level: f32) -> Result<()> {
        on_com_thread(move || unsafe {
            endpoint(device.as_deref())?.SetMasterVolumeLevelScalar(level, std::ptr::null())
        })
    }

    /// COM on a thread of its own: the main thread is already set up by the webview.
    fn on_com_thread<T: Send + 'static>(
        f: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        std::thread::spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            f()
        })
        .join()
        .unwrap_or_else(|_| Err(windows::core::Error::empty()))
    }

    /// The microphone chosen in settings by name, as the capture code finds it, or the
    /// system default.
    unsafe fn endpoint(name: Option<&str>) -> Result<IAudioEndpointVolume> {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let mut device = None;
        if let Some(name) = name {
            let all = devices.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
            for i in 0..all.GetCount()? {
                let candidate = all.Item(i)?;
                if friendly_name(&candidate).as_deref() == Some(name) {
                    device = Some(candidate);
                    break;
                }
            }
        }
        let device = match device {
            Some(device) => device,
            None => devices.GetDefaultAudioEndpoint(eCapture, eConsole)?,
        };
        device.Activate(CLSCTX_ALL, None)
    }

    unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let text = PropVariantToStringAlloc(&value).ok()?;
        let name = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const _));
        name
    }
}
