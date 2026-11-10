use serde::ser::{Serialize, Serializer};

/// Every Tauri command returns `AppResult<T>`.
/// `Serialize` turns the error into a plain string for the frontend.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("file watcher error: {0}")]
    Watch(#[from] notify::Error),

    #[error("path does not exist: {0}")]
    PathNotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("could not resolve the application data directory")]
    DataDirUnavailable,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::LockPoisoned
    }
}

pub type AppResult<T> = Result<T, AppError>;
