use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot decode audio: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("{0}")]
    Audio(String),
    #[error(transparent)]
    Onnx(#[from] ort::Error),
    #[error("{0}")]
    Model(String),
    #[error("{url}: {reason}")]
    Download { url: String, reason: String },
    #[error("{file}: every source failed: {}", join(failures))]
    Sources { file: String, failures: Vec<Error> },
    #[error("{0}: checksum mismatch")]
    Checksum(String),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Translate(#[from] boltay_translate::Error),
}

fn join(failures: &[Error]) -> String {
    failures
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<ort::Error<ort::session::builder::SessionBuilder>> for Error {
    fn from(e: ort::Error<ort::session::builder::SessionBuilder>) -> Self {
        Self::Onnx(e.into())
    }
}
