use std::fs;
use std::path::{Path, PathBuf};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

#[cfg(doc)]
use crate::EngineOptions;
use crate::Result;

/// Session for an encoder: one thread per physical core. With `cache`, a pre-optimized copy
/// of the graph is kept next to the model, see [`EngineOptions::graph_cache`].
pub fn heavy(path: &Path, cache: bool) -> Result<Session> {
    if !cache {
        return open(path, num_cpus::get_physical());
    }
    let cached = cache_path(path);
    if is_fresh(&cached, path) {
        match open(&cached, num_cpus::get_physical()) {
            Ok(session) => return Ok(session),
            // Written by another runtime version or damaged: rebuild it below.
            Err(_) => {
                let _ = fs::remove_file(&cached);
            }
        }
    }
    if save_optimized(path, &cached).is_ok() {
        if let Ok(session) = open(&cached, num_cpus::get_physical()) {
            return Ok(session);
        }
    }
    open(path, num_cpus::get_physical())
}

/// Session for a tiny graph called once per token, where a thread pool only adds overhead.
pub fn light(path: &Path) -> Result<Session> {
    open(path, 1)
}

fn open(path: &Path, threads: usize) -> Result<Session> {
    Ok(Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::All)?
        .with_intra_threads(threads)?
        .commit_from_file(path)?)
}

fn cache_path(model: &Path) -> PathBuf {
    let stem = model.file_stem().unwrap_or_default().to_string_lossy();
    model.with_file_name(format!("{stem}.optimized.onnx"))
}

fn is_fresh(cached: &Path, model: &Path) -> bool {
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(cached), modified(model)) {
        (Some(cached), Some(model)) => cached >= model,
        _ => false,
    }
}

// Saved at the extended level: the full level adds layout transforms tied to the CPU it ran
// on, so the cache would break on another machine. They are cheap to redo on every load.
fn save_optimized(model: &Path, cached: &Path) -> Result<()> {
    let tmp = cached.with_extension(format!("tmp{}", std::process::id()));
    let save = || -> Result<()> {
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level2)?
            .with_optimized_model_path(&tmp)?
            .commit_from_file(model)?;
        drop(session);
        fs::rename(&tmp, cached).map_err(|source| crate::Error::Io {
            path: cached.into(),
            source,
        })
    };
    let result = save();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_sits_next_to_the_model() {
        assert_eq!(
            cache_path(Path::new("m/parakeet-v3/encoder-model.int8.onnx")),
            Path::new("m/parakeet-v3/encoder-model.int8.optimized.onnx")
        );
    }

    #[test]
    fn missing_cache_is_stale() {
        assert!(!is_fresh(
            Path::new("no/such/cache"),
            Path::new("Cargo.toml")
        ));
    }
}
