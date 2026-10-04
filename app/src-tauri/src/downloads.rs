use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use boltay_core::{Model, Source, Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{overlay, Shared};

/// Downloads in flight, by model id. Each can be cancelled on its own.
#[derive(Default)]
pub struct Downloads {
    active: Mutex<HashMap<&'static str, Arc<AtomicBool>>>,
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    id: &'static str,
    done: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
struct FinishedEvent {
    id: &'static str,
    /// `None` when the model is ready or the user cancelled.
    error: Option<String>,
    cancelled: bool,
}

impl Downloads {
    pub fn active(&self) -> Vec<&'static str> {
        self.active.lock().unwrap().keys().copied().collect()
    }

    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.active.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn cancel_all(&self) {
        for flag in self.active.lock().unwrap().values() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// Starts downloading into the current models folder. A second request for a model that
/// is already downloading is ignored.
pub fn start(app: &AppHandle, id: &str) -> Result<()> {
    let model = Model::by_id(id).ok_or_else(|| anyhow!("unknown model {id}"))?;
    let shared = app.state::<Shared>();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = shared.downloads.active.lock().unwrap();
        if active.contains_key(model.id) {
            return Ok(());
        }
        active.insert(model.id, cancel.clone());
    }
    let root = shared.models_dir(&shared.settings());
    let app = app.clone();
    thread::Builder::new()
        .name(format!("download {}", model.id))
        .spawn(move || run(app, model, root, cancel))?;
    Ok(())
}

fn run(app: AppHandle, model: &'static Model, root: std::path::PathBuf, cancel: Arc<AtomicBool>) {
    log::info!("downloading {}", model.id);
    crate::refresh_status(&app);
    let mut last = Instant::now() - Duration::from_secs(1);
    let result = Store::new(&root).fetch(
        model,
        &Source::defaults(),
        |p| {
            // A progress bar does not need 1000 updates a second.
            if last.elapsed() >= Duration::from_millis(150) || p.done == p.total {
                last = Instant::now();
                let _ = app.emit(
                    "model-progress",
                    ProgressEvent {
                        id: model.id,
                        done: p.done,
                        total: p.total,
                    },
                );
                crate::show_download_in_tray(&app, p.done, p.total);
            }
        },
        &cancel,
    );
    let shared = app.state::<Shared>();
    shared.downloads.active.lock().unwrap().remove(model.id);
    let cancelled = cancel.load(Ordering::Relaxed);
    let error = match result {
        Err(_) if cancelled => None,
        Err(e) => Some(e.to_string()),
        Ok(()) => None,
    };
    match &error {
        Some(e) => log::error!("download of {} failed: {e}", model.id),
        None if cancelled => log::info!("download of {} cancelled", model.id),
        None => log::info!("{} downloaded", model.id),
    }
    let _ = app.emit(
        "model-done",
        FinishedEvent {
            id: model.id,
            error: error.clone(),
            cancelled,
        },
    );
    // The overlay only knows about the download it was opened for: the first translation.
    if matches!(overlay::current(), Some(overlay::State::Fetching { id }) if id == model.id) {
        let failed = error.is_some() || cancelled;
        overlay::update(
            &app,
            overlay::State::Fetched {
                error: error.clone(),
            },
        );
        let linger = if failed { 6 } else { 3 };
        let app = app.clone();
        thread::spawn(move || crate::dictation::hide_after(&app, Duration::from_secs(linger)));
    }
    crate::refresh_status(&app);
    crate::preload(&app);
}
