use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::{Resource, ResourceType, Space};
use crate::paths;

const MIGRATION_V1: &str = r#"
CREATE TABLE spaces (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE resources (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id    INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
    type        TEXT    NOT NULL CHECK (type IN ('file', 'folder')),
    path        TEXT    NOT NULL COLLATE NOCASE,
    title       TEXT    NOT NULL,
    note        TEXT    NOT NULL DEFAULT '',
    is_missing  INTEGER NOT NULL DEFAULT 0 CHECK (is_missing IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (space_id, path)
);

CREATE INDEX idx_resources_space ON resources(space_id);
CREATE INDEX idx_resources_path ON resources(path COLLATE NOCASE);
CREATE INDEX idx_resources_missing ON resources(is_missing) WHERE is_missing = 1;

-- Metadata-only full-text index. `content='resources'` keeps the text out of
-- the FTS table (no duplicate storage); the triggers keep it in sync.
CREATE VIRTUAL TABLE resources_fts USING fts5(
    title,
    note,
    path,
    content = 'resources',
    content_rowid = 'id',
    tokenize = "unicode61 remove_diacritics 2",
    prefix = '2 3'
);

CREATE TRIGGER resources_ai AFTER INSERT ON resources BEGIN
    INSERT INTO resources_fts (rowid, title, note, path)
    VALUES (new.id, new.title, new.note, new.path);
END;

CREATE TRIGGER resources_ad AFTER DELETE ON resources BEGIN
    INSERT INTO resources_fts (resources_fts, rowid, title, note, path)
    VALUES ('delete', old.id, old.title, old.note, old.path);
END;

CREATE TRIGGER resources_au AFTER UPDATE ON resources BEGIN
    INSERT INTO resources_fts (resources_fts, rowid, title, note, path)
    VALUES ('delete', old.id, old.title, old.note, old.path);
    INSERT INTO resources_fts (rowid, title, note, path)
    VALUES (new.id, new.title, new.note, new.path);
END;
"#;

const SPACE_COLUMNS: &str = "id, name, created_at";

#[derive(Debug)]
pub struct ResourceRef {
    pub id: i64,
    pub kind: ResourceType,
    pub path: String,
    pub is_missing: bool,
}

#[derive(Debug)]
pub struct WatchTarget {
    pub path: String,
    pub recursive: bool,
}

pub fn open(database_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut connection = Connection::open(database_path)?;
    // WAL keeps reader/writer concurrency healthy while the watcher updates flags.
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(Duration::from_secs(5))?;
    migrate(&mut connection)?;

    Ok(connection)
}

fn migrate(connection: &mut Connection) -> AppResult<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(MIGRATION_V1)?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
    }

    Ok(())
}

pub fn create_space(connection: &Connection, name: &str) -> AppResult<Space> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput(
            "space name must not be empty".to_owned(),
        ));
    }
    if name.chars().count() > 80 {
        return Err(AppError::InvalidInput(
            "space name must be 80 characters or fewer".to_owned(),
        ));
    }

    connection
        .execute("INSERT INTO spaces (name) VALUES (?1)", [name])
        .map_err(|error| {
            if is_constraint_violation(&error) {
                AppError::InvalidInput(format!("a space named \"{name}\" already exists"))
            } else {
                AppError::Database(error)
            }
        })?;

    let id = connection.last_insert_rowid();
    get_space(connection, id)?
        .ok_or_else(|| AppError::InvalidInput("space could not be persisted".to_owned()))
}

pub fn list_spaces(connection: &Connection) -> AppResult<Vec<Space>> {
    let sql = format!("SELECT {SPACE_COLUMNS} FROM spaces ORDER BY name COLLATE NOCASE");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], Space::from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn delete_space(connection: &Connection, space_id: i64) -> AppResult<()> {
    let affected = connection.execute("DELETE FROM spaces WHERE id = ?1", [space_id])?;
    if affected == 0 {
        return Err(AppError::InvalidInput(format!(
            "space {space_id} does not exist"
        )));
    }
    Ok(())
}

fn get_space(connection: &Connection, space_id: i64) -> AppResult<Option<Space>> {
    let sql = format!("SELECT {SPACE_COLUMNS} FROM spaces WHERE id = ?1");
    connection
        .query_row(&sql, [space_id], Space::from_row)
        .optional()
        .map_err(AppError::from)
}

fn ensure_space(connection: &Connection, space_id: i64) -> AppResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM spaces WHERE id = ?1)",
        [space_id],
        |row| row.get(0),
    )?;

    if exists {
        Ok(())
    } else {
        Err(AppError::InvalidInput(format!(
            "space {space_id} does not exist"
        )))
    }
}

pub fn insert_resource(
    connection: &Connection,
    space_id: i64,
    kind: ResourceType,
    path: &str,
    title: &str,
    note: &str,
) -> AppResult<Resource> {
    ensure_space(connection, space_id)?;

    connection
        .execute(
            "INSERT INTO resources (space_id, type, path, title, note)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![space_id, kind, path, title, note],
        )
        .map_err(|error| {
            if is_constraint_violation(&error) {
                AppError::InvalidInput("that path is already in this space".to_owned())
            } else {
                AppError::Database(error)
            }
        })?;

    let id = connection.last_insert_rowid();
    get_resource(connection, id)?
        .ok_or_else(|| AppError::InvalidInput("resource could not be persisted".to_owned()))
}

pub fn list_resources(connection: &Connection, space_id: i64) -> AppResult<Vec<Resource>> {
    query_resources(connection, Some(space_id), None)
}

pub fn search_resources(
    connection: &Connection,
    query: &str,
    space_id: Option<i64>,
    limit: u32,
) -> AppResult<Vec<Resource>> {
    let limit = limit.clamp(1, 500);
    let term = query.trim();

    if term.is_empty() {
        return query_resources(connection, space_id, Some(limit));
    }

    let fts_query = build_fts_query(term);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = format!(
        "SELECT {} FROM resources_fts f
         JOIN resources r ON r.id = f.rowid
         WHERE resources_fts MATCH ?",
        Resource::COLUMNS
    );
    let mut arguments = vec![Value::Text(fts_query)];

    if let Some(space_id) = space_id {
        sql.push_str(" AND r.space_id = ?");
        arguments.push(Value::Integer(space_id));
    }

    // bm25 weights: title=10, note=2, path=1. Lower (more negative) is better.
    sql.push_str(" ORDER BY bm25(resources_fts, 10.0, 2.0, 1.0), r.title COLLATE NOCASE LIMIT ?");
    arguments.push(Value::Integer(i64::from(limit)));

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(arguments), Resource::from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

fn query_resources(
    connection: &Connection,
    space_id: Option<i64>,
    limit: Option<u32>,
) -> AppResult<Vec<Resource>> {
    let mut sql = format!("SELECT {} FROM resources r WHERE 1 = 1", Resource::COLUMNS);
    let mut arguments: Vec<Value> = Vec::new();

    if let Some(space_id) = space_id {
        sql.push_str(" AND r.space_id = ?");
        arguments.push(Value::Integer(space_id));
    }

    sql.push_str(" ORDER BY r.is_missing ASC, r.title COLLATE NOCASE ASC");

    if let Some(limit) = limit {
        sql.push_str(" LIMIT ?");
        arguments.push(Value::Integer(i64::from(limit)));
    }

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(arguments), Resource::from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn update_resource(
    connection: &Connection,
    id: i64,
    title: &str,
    note: &str,
) -> AppResult<Resource> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::InvalidInput(
            "resource title must not be empty".to_owned(),
        ));
    }

    let affected = connection.execute(
        "UPDATE resources
         SET title = ?1,
             note = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3",
        params![title, note, id],
    )?;

    if affected == 0 {
        return Err(AppError::InvalidInput(format!(
            "resource {id} does not exist"
        )));
    }

    get_resource(connection, id)?
        .ok_or_else(|| AppError::InvalidInput(format!("resource {id} does not exist")))
}

pub fn delete_resource(connection: &Connection, id: i64) -> AppResult<()> {
    let affected = connection.execute("DELETE FROM resources WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(AppError::InvalidInput(format!(
            "resource {id} does not exist"
        )));
    }
    Ok(())
}

fn get_resource(connection: &Connection, id: i64) -> AppResult<Option<Resource>> {
    let sql = format!(
        "SELECT {} FROM resources r WHERE r.id = ?1",
        Resource::COLUMNS
    );
    connection
        .query_row(&sql, [id], Resource::from_row)
        .optional()
        .map_err(AppError::from)
}

pub fn resource_refs(
    connection: &Connection,
    space_id: Option<i64>,
) -> AppResult<Vec<ResourceRef>> {
    let mut sql = String::from("SELECT id, type, path, is_missing FROM resources");
    let mut arguments: Vec<Value> = Vec::new();

    if let Some(space_id) = space_id {
        sql.push_str(" WHERE space_id = ?");
        arguments.push(Value::Integer(space_id));
    }

    sql.push_str(" ORDER BY id");

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(arguments), |row: &Row<'_>| {
        Ok(ResourceRef {
            id: row.get(0)?,
            kind: row.get(1)?,
            path: row.get(2)?,
            is_missing: row.get(3)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

pub fn apply_missing(connection: &mut Connection, updates: &[(i64, bool)]) -> AppResult<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let transaction = connection.transaction()?;
    {
        let mut statement = transaction.prepare(
            "UPDATE resources
             SET is_missing = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
        )?;

        for &(id, is_missing) in updates {
            statement.execute(params![is_missing, id])?;
        }
    }
    transaction.commit()?;

    Ok(())
}

pub fn watch_targets(connection: &Connection) -> AppResult<Vec<WatchTarget>> {
    let mut statement = connection.prepare("SELECT type, path FROM resources")?;
    let rows = statement.query_map([], |row| {
        let kind: ResourceType = row.get(0)?;
        let stored: String = row.get(1)?;
        Ok((kind, stored))
    })?;

    let mut targets: HashMap<String, WatchTarget> = HashMap::new();

    for row in rows {
        let (kind, stored) = row?;
        let path = paths::from_stored_path(&stored);
        let (watch_path, recursive) = match kind {
            ResourceType::Folder => (path, true),
            ResourceType::File => (
                path.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| path.clone()),
                false,
            ),
        };

        let key = paths::to_stored_path(&watch_path).to_lowercase();
        match targets.get_mut(&key) {
            Some(existing) => existing.recursive |= recursive,
            None => {
                targets.insert(
                    key,
                    WatchTarget {
                        path: paths::to_stored_path(&watch_path),
                        recursive,
                    },
                );
            }
        }
    }

    Ok(targets.into_values().collect())
}

/// Convert free-form user input into a safe FTS5 prefix query.
///
/// Only alphanumeric tokens are kept and each one is quoted, so operators
/// (`AND`, `OR`, `NEAR`, `-`, `*`, `"`) cannot break the query or inject SQL.
fn build_fts_query(input: &str) -> String {
    input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .take(8)
        .map(|token| format!("\"{token}\"*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn is_constraint_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        migrate(&mut connection).expect("run migrations");
        connection
    }

    #[test]
    fn builds_safe_prefix_queries() {
        assert_eq!(
            build_fts_query("quarterly report"),
            "\"quarterly\"* AND \"report\"*"
        );
        assert_eq!(
            build_fts_query("foo\" OR bar*"),
            "\"foo\"* AND \"OR\"* AND \"bar\"*"
        );
        assert_eq!(build_fts_query("   "), "");
    }

    #[test]
    fn fts5_search_matches_title_note_and_path_prefixes() {
        let connection = test_connection();
        let space = create_space(&connection, "Demo").expect("create space");

        insert_resource(
            &connection,
            space.id,
            ResourceType::File,
            "C:/work/quarterly-report.pdf",
            "Q4 numbers",
            "finance review",
        )
        .expect("insert resource");

        let by_note =
            search_resources(&connection, "finan", Some(space.id), 20).expect("search note");
        assert_eq!(by_note.len(), 1);

        let by_path =
            search_resources(&connection, "quarter", Some(space.id), 20).expect("search path");
        assert_eq!(by_path.len(), 1);

        let no_match =
            search_resources(&connection, "unrelated", Some(space.id), 20).expect("search miss");
        assert!(no_match.is_empty());
    }

    #[test]
    fn missing_flags_round_trip() {
        let mut connection = test_connection();
        let space = create_space(&connection, "Demo").expect("create space");
        let resource = insert_resource(
            &connection,
            space.id,
            ResourceType::Folder,
            "C:/work/src",
            "src",
            "",
        )
        .expect("insert resource");

        assert!(!resource.is_missing);
        apply_missing(&mut connection, &[(resource.id, true)]).expect("mark missing");

        let resources = list_resources(&connection, space.id).expect("list resources");
        assert!(resources[0].is_missing);
    }
}
