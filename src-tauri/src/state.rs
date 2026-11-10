use std::sync::Mutex;

use notify::RecommendedWatcher;
use rusqlite::Connection;

/// Shared application state.
///
/// A single SQLite connection is enough for this workload and serializes
/// writes; WAL mode keeps reads fast. The watcher is kept alive here because
/// dropping `RecommendedWatcher` stops its OS-level thread.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
}

impl AppState {
    pub fn new(db: Connection) -> Self {
        Self {
            db: Mutex::new(db),
            watcher: Mutex::new(None),
        }
    }
}
