use notify::{Event, EventKind, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::{self, ResourceRef};
use crate::error::AppResult;
use crate::models::ResourceType;
use crate::paths::{self, Probe};
use crate::state::AppState;

pub const RESOURCES_CHANGED_EVENT: &str = "resources-changed";

/// Rebuild the watch set from the database.
///
/// `notify` picks `ReadDirectoryChangesW` on Windows automatically. Rebuilding
/// is cheap for a personal workspace and avoids tracking individual watch
/// handles after adds/deletes.
pub fn restart(app: &AppHandle) -> AppResult<()> {
    let targets = {
        let state = app.state::<AppState>();
        let connection = state.db.lock()?;
        db::watch_targets(&connection)?
    };
    let target_count = targets.len();

    let handle = app.clone();
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event) => handle_event(&handle, event),
            Err(error) => log::warn!("file watcher error: {error}"),
        })?;

    for target in targets {
        let path = paths::from_stored_path(&target.path);
        let mode = if target.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        // A locked, deleted or disconnected directory must not abort startup.
        if let Err(error) = watcher.watch(&path, mode) {
            log::warn!("cannot watch {}: {error}", path.display());
        }
    }

    let state = app.state::<AppState>();
    let mut slot = state.watcher.lock()?;
    // Assigning drops the previous watcher, which stops its OS thread.
    *slot = Some(watcher);

    log::info!("file watcher active for {target_count} location(s)");
    Ok(())
}

fn handle_event(app: &AppHandle, event: Event) {
    if !is_relevant(&event) {
        return;
    }

    let changed_paths: Vec<String> = event
        .paths
        .iter()
        .map(|path| paths::to_stored_path(path))
        .collect();

    if changed_paths.is_empty() {
        return;
    }

    let state = app.state::<AppState>();
    let updates = {
        let connection = match state.db.lock() {
            Ok(connection) => connection,
            Err(_) => {
                log::warn!("database lock poisoned while handling a file event");
                return;
            }
        };

        let references = match db::resource_refs(&connection, None) {
            Ok(references) => references,
            Err(error) => {
                log::warn!("could not load resources after a file event: {error}");
                return;
            }
        };
        drop(connection);

        let mut updates = Vec::new();
        for reference in references {
            if !affects(&reference, &changed_paths) {
                continue;
            }

            match paths::probe(&reference.path) {
                Probe::Exists if reference.is_missing => updates.push((reference.id, false)),
                Probe::Missing if !reference.is_missing => updates.push((reference.id, true)),
                _ => {}
            }
        }
        updates
    };

    if updates.is_empty() {
        return;
    }

    match state.db.lock() {
        Ok(mut connection) => {
            if let Err(error) = db::apply_missing(&mut connection, &updates) {
                log::warn!("could not update missing flags: {error}");
                return;
            }
        }
        Err(_) => {
            log::warn!("database lock poisoned while updating missing flags");
            return;
        }
    }

    if let Err(error) = app.emit(RESOURCES_CHANGED_EVENT, ()) {
        log::warn!("could not notify the frontend: {error}");
    }
}

fn is_relevant(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Remove(_)
            | EventKind::Create(_)
            | EventKind::Modify(notify::event::ModifyKind::Name(_))
    )
}

fn affects(reference: &ResourceRef, changed_paths: &[String]) -> bool {
    let resource_path = reference.path.trim_end_matches('/');

    changed_paths.iter().any(|changed| {
        let changed = changed.trim_end_matches('/');

        if changed.eq_ignore_ascii_case(resource_path) {
            return true;
        }

        match reference.kind {
            // A folder is affected when one of its descendants is removed
            // (Windows sometimes reports children before the directory itself).
            ResourceType::Folder => paths::is_descendant(changed, resource_path),
            // A file is affected when its parent directory disappears.
            ResourceType::File => paths::from_stored_path(resource_path)
                .parent()
                .map(|parent| paths::to_stored_path(parent).eq_ignore_ascii_case(changed))
                .unwrap_or(false),
        }
    })
}
