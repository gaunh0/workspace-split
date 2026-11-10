use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::Row;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

impl Space {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    File,
    Folder,
}

impl ResourceType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
        }
    }
}

impl FromSql for ResourceType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            other => Err(FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown resource type: {other}"),
            )))),
        }
    }
}

impl ToSql for ResourceType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: i64,
    pub space_id: i64,
    #[serde(rename = "type")]
    pub kind: ResourceType,
    pub path: String,
    pub title: String,
    pub note: String,
    pub is_missing: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Resource {
    /// Columns are always selected with the `r` alias so this works in joins.
    pub const COLUMNS: &'static str =
        "r.id, r.space_id, r.type, r.path, r.title, r.note, r.is_missing, r.created_at, r.updated_at";

    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            space_id: row.get(1)?,
            kind: row.get(2)?,
            path: row.get(3)?,
            title: row.get(4)?,
            note: row.get(5)?,
            is_missing: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingReport {
    pub checked: usize,
    pub marked_missing: usize,
    pub recovered: usize,
}

impl MissingReport {
    pub fn changed(&self) -> usize {
        self.marked_missing + self.recovered
    }
}
