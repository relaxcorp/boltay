use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use boltay_core::{
    Dictation, EngineOptions, Language, Model, Recognizer, Status, Store, MODELS, SILERO_VAD,
};
use serde::Serialize;

use crate::settings::Speech;

impl Speech {
    pub fn language(self) -> Language {
        match self {
            Speech::Russian => Language::Russian,
            Speech::Other => Language::Other,
        }
    }

    /// Everything that has to be on disk before this language can be dictated.
    pub fn models(self) -> [&'static Model; 2] {
        [self.language().model(), &SILERO_VAD]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelStatus {
    pub id: &'static str,
    pub size: u64,
    /// Bytes on disk, counting partial downloads.
    pub done: u64,
    pub ready: bool,
}

pub fn status(root: &Path) -> Vec<ModelStatus> {
    let store = Store::new(root);
    MODELS
        .into_iter()
        .map(|model| {
            let size = model.size();
            let (done, ready) = match store.status(model) {
                Status::Ready => (size, true),
                Status::Partial(done) => (done, false),
                Status::Missing => (0, false),
            };
            ModelStatus {
                id: model.id,
                size,
                done,
                ready,
            }
        })
        .collect()
}

pub fn model_ready(root: &Path, model: &Model) -> bool {
    Store::new(root).status(model) == Status::Ready
}

pub fn ready(root: &Path, speech: Speech) -> bool {
    let store = Store::new(root);
    speech
        .models()
        .iter()
        .all(|m| store.status(m) == Status::Ready)
}

/// Removes a model's files, partial downloads included. Silero lives in the root next
/// to the other models' folders, so only its own files go.
pub fn delete(root: &Path, model: &Model) -> Result<()> {
    let dir = Store::new(root).dir(model);
    for file in model.files {
        for path in [dir.join(file.name), dir.join(format!("{}.part", file.name))] {
            match fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(e).with_context(|| format!("cannot delete {}", path.display()))
                }
                _ => {}
            }
        }
    }
    if !model.dir.is_empty() {
        // Leftovers like LICENSE.txt from the release, and the folder itself.
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// The loaded models. Loading takes from 1.5 s (GigaAM) to 7 s (Parakeet), so it happens
/// in the background and the result stays in memory until the language, the folder or the
/// options change, or it sits idle. The translator joins on first use.
#[derive(Default)]
pub struct Engines {
    loaded: Mutex<Option<Loaded>>,
    loading: AtomicBool,
    /// Which translation model the loaded dictation holds.
    translator: Mutex<Option<&'static str>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    speech: Speech,
    graph_cache: bool,
}

struct Loaded {
    key: Key,
    root: PathBuf,
    dictation: Arc<RwLock<Dictation>>,
    last_used: Instant,
}

impl Engines {
    pub fn get(
        &self,
        speech: Speech,
        root: &Path,
        graph_cache: bool,
    ) -> Result<Arc<RwLock<Dictation>>> {
        let key = Key {
            speech,
            graph_cache,
        };
        let mut loaded = self.loaded.lock().unwrap();
        if let Some(l) = loaded.as_mut() {
            if l.key == key && l.root == root {
                l.last_used = Instant::now();
                return Ok(l.dictation.clone());
            }
        }
        // Drop the old model first: two of them do not fit in the memory budget.
        *loaded = None;
        self.loading.store(true, Ordering::Relaxed);
        let started = Instant::now();
        let result = load(speech, root, graph_cache);
        self.loading.store(false, Ordering::Relaxed);
        let dictation = Arc::new(RwLock::new(result?));
        log::info!(
            "{speech:?} model loaded in {} ms",
            started.elapsed().as_millis()
        );
        *loaded = Some(Loaded {
            key,
            root: root.to_path_buf(),
            dictation: dictation.clone(),
            last_used: Instant::now(),
        });
        Ok(dictation)
    }

    /// Puts the wanted translation model into the dictation, replacing the other one if the
    /// setting changed. The base model loads in about a second, the big one takes longer.
    pub fn ensure_translator(
        &self,
        dictation: &RwLock<Dictation>,
        root: &Path,
        model: &'static Model,
    ) -> Result<()> {
        let mut current = self.translator.lock().unwrap();
        if *current == Some(model.id) && dictation.read().unwrap().translator.is_some() {
            return Ok(());
        }
        // Free the other model before loading: both at once would not fit.
        dictation.write().unwrap().translator = None;
        self.loading.store(true, Ordering::Relaxed);
        let started = Instant::now();
        let translator = Store::new(root).translator(model);
        self.loading.store(false, Ordering::Relaxed);
        let translator = translator.context("cannot load the translation model")?;
        log::info!(
            "{} loaded in {} ms",
            model.id,
            started.elapsed().as_millis()
        );
        dictation.write().unwrap().translator = Some(translator);
        *current = Some(model.id);
        Ok(())
    }

    pub fn is_loading(&self) -> bool {
        self.loading.load(Ordering::Relaxed)
    }

    pub fn is_loaded(&self, speech: Speech, root: &Path) -> bool {
        self.loaded
            .try_lock()
            .ok()
            .and_then(|l| l.as_ref().map(|l| l.key.speech == speech && l.root == root))
            .unwrap_or(false)
    }

    pub fn has_translator(&self, model: &Model) -> bool {
        *self.translator.lock().unwrap() == Some(model.id)
            && self
                .loaded
                .try_lock()
                .ok()
                .and_then(|l| {
                    l.as_ref()
                        .map(|l| l.dictation.read().unwrap().translator.is_some())
                })
                .unwrap_or(false)
    }

    /// Frees the models if they have not been used for `idle`. A recognition in flight
    /// keeps its own reference and finishes normally.
    pub fn unload_if_idle(&self, idle: Duration) {
        let mut loaded = self.loaded.lock().unwrap();
        if loaded
            .as_ref()
            .is_some_and(|l| l.last_used.elapsed() >= idle)
        {
            *loaded = None;
            log::info!("models unloaded after {} min idle", idle.as_secs() / 60);
        }
    }

    pub fn unload(&self) {
        if self.loaded.lock().unwrap().take().is_some() {
            log::info!("models unloaded");
        }
    }
}

fn load(speech: Speech, root: &Path, graph_cache: bool) -> Result<Dictation> {
    let store = Store::new(root);
    let engine = store
        .engine(speech.language(), EngineOptions { graph_cache })
        .context("cannot load the speech model")?;
    let vad = store.vad().context("cannot load Silero VAD")?;
    Ok(Dictation {
        recognizer: Recognizer::new(engine, Some(vad)),
        text: boltay_text::Config::default(),
        translator: None,
        paragraph_pause: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use boltay_core::{GIGAAM, PARAKEET};

    fn fake(root: &Path, model: &Model, complete: bool) {
        let dir = Store::new(root).dir(model);
        fs::create_dir_all(&dir).unwrap();
        for (i, file) in model.files.iter().enumerate() {
            // Store judges by size; sparse files keep the test fast and small on disk.
            let f = fs::File::create(dir.join(file.name)).unwrap();
            let size = if complete || i > 0 {
                file.size
            } else {
                file.size - 1
            };
            f.set_len(size).unwrap();
        }
    }

    #[test]
    fn empty_root_has_nothing_ready() {
        let root = tempfile::tempdir().unwrap();
        let status = status(root.path());
        assert_eq!(status.len(), MODELS.len());
        assert!(status.iter().all(|s| !s.ready && s.done == 0 && s.size > 0));
        assert!(!ready(root.path(), Speech::Russian));
    }

    #[test]
    fn russian_needs_gigaam_and_vad() {
        let root = tempfile::tempdir().unwrap();
        fake(root.path(), &GIGAAM, true);
        assert!(!ready(root.path(), Speech::Russian));
        fake(root.path(), &SILERO_VAD, true);
        assert!(ready(root.path(), Speech::Russian));
        assert!(!ready(root.path(), Speech::Other));
    }

    #[test]
    fn partial_files_count_as_progress() {
        let root = tempfile::tempdir().unwrap();
        fake(root.path(), &PARAKEET, false);
        let parakeet = status(root.path())
            .into_iter()
            .find(|s| s.id == PARAKEET.id)
            .unwrap();
        assert!(!parakeet.ready);
        assert!(parakeet.done > 0 && parakeet.done < parakeet.size);
    }

    #[test]
    fn delete_removes_a_model_and_keeps_the_others() {
        let root = tempfile::tempdir().unwrap();
        fake(root.path(), &GIGAAM, true);
        fake(root.path(), &SILERO_VAD, true);
        fs::write(
            Store::new(root.path()).dir(&GIGAAM).join("LICENSE.txt"),
            "MIT",
        )
        .unwrap();
        delete(root.path(), &GIGAAM).unwrap();
        assert!(!Store::new(root.path()).dir(&GIGAAM).exists());
        assert_eq!(Store::new(root.path()).status(&SILERO_VAD), Status::Ready);
        delete(root.path(), &SILERO_VAD).unwrap();
        assert!(root.path().exists());
        assert_eq!(Store::new(root.path()).status(&SILERO_VAD), Status::Missing);
    }

    #[test]
    fn deleting_a_missing_model_is_fine() {
        let root = tempfile::tempdir().unwrap();
        delete(root.path(), &PARAKEET).unwrap();
    }

    #[test]
    fn loading_without_files_fails_cleanly() {
        let root = tempfile::tempdir().unwrap();
        let engines = Engines::default();
        let err = engines
            .get(Speech::Russian, root.path(), false)
            .err()
            .unwrap();
        assert!(format!("{err:#}").contains("gigaam"), "{err:#}");
        assert!(!engines.is_loading());
        assert!(!engines.is_loaded(Speech::Russian, root.path()));
    }
}
