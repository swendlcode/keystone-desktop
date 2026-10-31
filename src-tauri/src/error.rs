use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeystoneError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("watcher error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("analysis error: {0}")]
    Analysis(String),

    #[error("tag error: {0}")]
    Tag(String),

    /// The export refused to run. Carries the sentence the modal shows: a DJ app
    /// holding the collection open is a refusal, not a warning, because Traktor
    /// rewrites `collection.nml` on exit and would discard the edit silently.
    #[error("export refused: {0}")]
    ExportRefused(String),

    #[error("export error: {0}")]
    Export(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

impl Serialize for KeystoneError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, KeystoneError>;

impl From<anyhow::Error> for KeystoneError {
    fn from(e: anyhow::Error) -> Self {
        KeystoneError::Other(e.to_string())
    }
}

/// Lets a `spawn_blocking` join error cross the command boundary without every
/// command repeating the same map_err.
pub fn joined<T>(r: std::result::Result<Result<T>, tokio::task::JoinError>) -> Result<T> {
    match r {
        Ok(inner) => inner,
        Err(e) => Err(KeystoneError::Other(e.to_string())),
    }
}
