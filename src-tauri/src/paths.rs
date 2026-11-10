use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Result of a non-destructive existence check.
///
/// `Unknown` covers locked or permission-denied paths: we deliberately do not
/// flag those as missing, because another process may simply hold the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Exists,
    Missing,
    Unknown,
}

/// Resolve a user-selected path to a stable, canonical Windows path.
///
/// `dunce` removes the `\\?\` verbatim prefix that `std::fs::canonicalize`
/// adds, while still resolving symlinks and normalizing 8.3 names.
pub fn normalize_existing(raw: &str) -> AppResult<PathBuf> {
    let trimmed = raw.trim().trim_matches('"');
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("path must not be empty".to_owned()));
    }

    let path = Path::new(trimmed);
    match fs::metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::PathNotFound(trimmed.to_owned()));
        }
        Err(error) => return Err(AppError::Io(error)),
    }

    let canonical = dunce::canonicalize(path).map_err(|error| match error.kind() {
        ErrorKind::NotFound => AppError::PathNotFound(trimmed.to_owned()),
        _ => AppError::Io(error),
    })?;

    Ok(dunce::simplified(&canonical).to_path_buf())
}

/// The on-disk format stored in SQLite: forward slashes, no verbatim prefix.
///
/// Forward slashes keep FTS5 tokens consistent (`C:/work/report.pdf` tokenizes
/// exactly like a URL-ish path on every Windows path form) and avoid escaped
/// backslashes in JSON and SQL literals.
pub fn to_stored_path(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().replace('\\', "/")
}

/// Stored paths are accepted directly by Windows APIs, including `//server/share`.
pub fn from_stored_path(stored: &str) -> PathBuf {
    PathBuf::from(stored)
}

pub fn default_title(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| to_stored_path(path))
}

pub fn probe(stored: &str) -> Probe {
    match fs::metadata(Path::new(stored)) {
        Ok(_) => Probe::Exists,
        Err(error) if error.kind() == ErrorKind::NotFound => Probe::Missing,
        Err(error) => {
            // Locked by another process, access denied, disconnected network
            // share, ... Leave the flag untouched instead of lying to the user.
            log::debug!("could not inspect {stored}: {error}");
            Probe::Unknown
        }
    }
}

pub fn is_descendant(path: &str, ancestor: &str) -> bool {
    let prefix = format!("{}/", ancestor.trim_end_matches('/'));
    if path.len() <= prefix.len() {
        return false;
    }
    path.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn strips_verbatim_prefix() {
        let path = Path::new(r"\\?\C:\Users\dev\report.pdf");
        assert_eq!(to_stored_path(path), "C:/Users/dev/report.pdf");
    }

    #[test]
    fn detects_descendants_case_insensitively() {
        assert!(is_descendant("C:/Work/src/main.rs", "c:/work"));
        assert!(!is_descendant("C:/workshop/file.txt", "C:/work"));
    }
}
