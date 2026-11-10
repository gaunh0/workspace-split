use std::fs;

use tauri::{AppHandle, Emitter, State};

use crate::db;
use crate::error::AppResult;
use crate::models::{MissingReport, Resource, ResourceType, Space};
use crate::paths::{self, Probe};
use crate::state::AppState;
use crate::watcher::{self, RESOURCES_CHANGED_EVENT};

#[tauri::command]
pub fn create_space(state: State<'_, AppState>, name: String) -> AppResult<Space> {
    let connection = state.db.lock()?;
    db::create_space(&connection, &name)
}

#[tauri::command]
pub fn list_spaces(state: State<'_, AppState>) -> AppResult<Vec<Space>> {
    let connection = state.db.lock()?;
    db::list_spaces(&connection)
}

#[tauri::command]
pub fn delete_space(app: AppHandle, state: State<'_, AppState>, space_id: i64) -> AppResult<()> {
    {
        let connection = state.db.lock()?;
        db::delete_space(&connection, space_id)?;
    }

    refresh_watches(&app);
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn add_resource(
    app: AppHandle,
    state: State<'_, AppState>,
    space_id: i64,
    path: String,
    title: Option<String>,
    note: Option<String>,
) -> AppResult<Resource> {
    // Canonicalize first: `dunce` removes the `\\?\` prefix and resolves the
    // real on-disk casing, so FTS and de-duplication see one stable path.
    let normalized = paths::normalize_existing(&path)?;
    let metadata = fs::metadata(&normalized)?;

    let kind = if metadata.is_dir() {
        ResourceType::Folder
    } else {
        ResourceType::File
    };
    let stored_path = paths::to_stored_path(&normalized);
    let title = title
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| paths::default_title(&normalized));
    let note = note.unwrap_or_default();

    let resource = {
        let connection = state.db.lock()?;
        db::insert_resource(&connection, space_id, kind, &stored_path, &title, &note)?
    };

    refresh_watches(&app);
    emit_changed(&app);
    Ok(resource)
}

#[tauri::command]
pub fn list_resources(state: State<'_, AppState>, space_id: i64) -> AppResult<Vec<Resource>> {
    let connection = state.db.lock()?;
    db::list_resources(&connection, space_id)
}

#[tauri::command]
pub fn search_resources(
    state: State<'_, AppState>,
    query: String,
    space_id: Option<i64>,
    limit: Option<u32>,
) -> AppResult<Vec<Resource>> {
    let connection = state.db.lock()?;
    db::search_resources(&connection, &query, space_id, limit.unwrap_or(100))
}

#[tauri::command]
pub fn update_resource(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    title: String,
    note: String,
) -> AppResult<Resource> {
    let resource = {
        let connection = state.db.lock()?;
        db::update_resource(&connection, id, &title, &note)?
    };

    emit_changed(&app);
    Ok(resource)
}

#[tauri::command]
pub fn delete_resource(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    {
        let connection = state.db.lock()?;
        db::delete_resource(&connection, id)?;
    }

    refresh_watches(&app);
    emit_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn check_missing_files(
    app: AppHandle,
    state: State<'_, AppState>,
    space_id: Option<i64>,
) -> AppResult<MissingReport> {
    // Phase 1: snapshot under the DB lock, then release it while hitting the
    // filesystem. A slow or disconnected network share cannot block the UI.
    let references = {
        let connection = state.db.lock()?;
        db::resource_refs(&connection, space_id)?
    };

    let mut report = MissingReport {
        checked: references.len(),
        ..MissingReport::default()
    };
    let mut updates = Vec::new();

    for reference in &references {
        match paths::probe(&reference.path) {
            Probe::Exists if reference.is_missing => {
                updates.push((reference.id, false));
                report.recovered += 1;
            }
            Probe::Missing if !reference.is_missing => {
                updates.push((reference.id, true));
                report.marked_missing += 1;
            }
            // Exists + present, missing + already flagged, or locked/unknown.
            _ => {}
        }
    }

    if !updates.is_empty() {
        let mut connection = state.db.lock()?;
        db::apply_missing(&mut connection, &updates)?;
    }

    if report.changed() > 0 {
        emit_changed(&app);
    }

    Ok(report)
}

#[tauri::command]
pub fn reveal_in_explorer(path: String) -> AppResult<()> {
    let target = paths::normalize_existing(&path)?;

    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("explorer.exe");
        if target.is_dir() {
            command.arg(&target);
        } else {
            // `/select,` is a single argument; the path is quoted by Command.
            command.arg("/select,").arg(&target);
        }
        command.spawn()?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = target;
        Err(crate::error::AppError::InvalidInput(
            "revealing files is only supported on Windows".to_owned(),
        ))
    }
}

/// Watches are derived from the resource table, so mutations rebuild them.
fn refresh_watches(app: &AppHandle) {
    if let Err(error) = watcher::restart(app) {
        log::warn!("could not refresh the file watcher: {error}");
    }
}

fn emit_changed(app: &AppHandle) {
    if let Err(error) = app.emit(RESOURCES_CHANGED_EVENT, ()) {
        log::warn!("could not notify the frontend: {error}");
    }
}
