use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use daedalus_core::{DaedalusError, Result, now_utc};
use daedalus_domain::{Job, JobKind, JobStatus, LibraryItem, ModelKind, SourceKind, SourceRef};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone)]
pub struct CatalogDb {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LibraryItemInsert {
    pub display_name: String,
    pub primary_model_kind: ModelKind,
    pub source: Option<SourceRef>,
    pub installed_version: Option<String>,
    pub storage_path: String,
    pub favorite: bool,
    pub pinned: bool,
    pub notes: Option<String>,
    pub last_verified_at: Option<DateTime<Utc>>,
}

impl CatalogDb {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let db = Self { path: path.into() };
        db.initialize()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn initialize(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = self.open_connection()?;
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS sources (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                display_name TEXT NOT NULL,
                api_base_url TEXT NOT NULL,
                web_base_url TEXT NOT NULL,
                enabled INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS source_models (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id TEXT NOT NULL,
                source_model_id TEXT NOT NULL,
                title TEXT NOT NULL,
                creator TEXT,
                description TEXT,
                model_kind TEXT NOT NULL,
                source_url TEXT,
                updated_at TEXT NOT NULL,
                UNIQUE(source_id, source_model_id)
            );

            CREATE TABLE IF NOT EXISTS library_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                display_name TEXT NOT NULL,
                primary_model_kind TEXT NOT NULL,
                source_kind TEXT,
                source_model_id TEXT,
                source_version_id TEXT,
                source_url TEXT,
                source_category TEXT,
                installed_version TEXT,
                storage_path TEXT NOT NULL UNIQUE,
                favorite INTEGER NOT NULL DEFAULT 0,
                pinned INTEGER NOT NULL DEFAULT 0,
                notes TEXT,
                last_verified_at TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS library_item_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                library_item_id INTEGER NOT NULL,
                filename TEXT NOT NULL,
                local_path TEXT NOT NULL,
                local_state TEXT NOT NULL,
                size_bytes INTEGER,
                sha256 TEXT,
                mime_type TEXT,
                format TEXT,
                precision TEXT,
                source_download_url TEXT,
                FOREIGN KEY(library_item_id) REFERENCES library_items(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                progress REAL NOT NULL DEFAULT 0.0,
                summary TEXT NOT NULL,
                error_summary TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT
            );

            CREATE TABLE IF NOT EXISTS job_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL,
                status TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
            );
            "#,
        )
        .map_err(|err| DaedalusError::Database(err.to_string()))?;
        Ok(())
    }

    pub fn library_item_count(&self) -> Result<usize> {
        let conn = self.open_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM library_items", [], |row| row.get(0))
            .map_err(|err| DaedalusError::Database(err.to_string()))?;
        Ok(count as usize)
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>> {
        let conn = self.open_connection()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    id,
                    display_name,
                    primary_model_kind,
                    source_kind,
                    source_model_id,
                    source_version_id,
                    source_url,
                    source_category,
                    installed_version,
                    storage_path,
                    favorite,
                    pinned,
                    notes,
                    last_verified_at,
                    created_at
                FROM library_items
                ORDER BY pinned DESC, favorite DESC, created_at DESC
                "#,
            )
            .map_err(|err| DaedalusError::Database(err.to_string()))?;

        let rows = stmt
            .query_map([], map_library_item)
            .map_err(|err| DaedalusError::Database(err.to_string()))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|err| DaedalusError::Database(err.to_string()))?);
        }
        Ok(items)
    }

    pub fn get_library_item(&self, id: i64) -> Result<Option<LibraryItem>> {
        let conn = self.open_connection()?;
        conn.query_row(
            r#"
            SELECT
                id,
                display_name,
                primary_model_kind,
                source_kind,
                source_model_id,
                source_version_id,
                source_url,
                source_category,
                installed_version,
                storage_path,
                favorite,
                pinned,
                notes,
                last_verified_at,
                created_at
            FROM library_items
            WHERE id = ?1
            "#,
            [id],
            map_library_item,
        )
        .optional()
        .map_err(|err| DaedalusError::Database(err.to_string()))
    }

    pub fn find_library_item_by_storage_path(&self, storage_path: &str) -> Result<Option<LibraryItem>> {
        let conn = self.open_connection()?;
        conn.query_row(
            r#"
            SELECT
                id,
                display_name,
                primary_model_kind,
                source_kind,
                source_model_id,
                source_version_id,
                source_url,
                source_category,
                installed_version,
                storage_path,
                favorite,
                pinned,
                notes,
                last_verified_at,
                created_at
            FROM library_items
            WHERE storage_path = ?1
            "#,
            [storage_path],
            map_library_item,
        )
        .optional()
        .map_err(|err| DaedalusError::Database(err.to_string()))
    }

    pub fn upsert_library_item(&self, item: &LibraryItemInsert) -> Result<LibraryItem> {
        let conn = self.open_connection()?;
        let now = now_utc().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO library_items (
                display_name,
                primary_model_kind,
                source_kind,
                source_model_id,
                source_version_id,
                source_url,
                source_category,
                installed_version,
                storage_path,
                favorite,
                pinned,
                notes,
                last_verified_at,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(storage_path) DO UPDATE SET
                display_name = excluded.display_name,
                primary_model_kind = excluded.primary_model_kind,
                source_kind = excluded.source_kind,
                source_model_id = excluded.source_model_id,
                source_version_id = excluded.source_version_id,
                source_url = excluded.source_url,
                source_category = excluded.source_category,
                installed_version = excluded.installed_version,
                favorite = excluded.favorite,
                pinned = excluded.pinned,
                notes = excluded.notes,
                last_verified_at = excluded.last_verified_at
            "#,
            params![
                item.display_name,
                item.primary_model_kind.to_string(),
                item.source.as_ref().map(|source| source_kind_to_str(source.source_kind)),
                item.source.as_ref().and_then(|source| source.source_model_id.as_deref()),
                item.source
                    .as_ref()
                    .and_then(|source| source.source_version_id.as_deref()),
                item.source.as_ref().and_then(|source| source.source_url.as_deref()),
                item.source
                    .as_ref()
                    .and_then(|source| source.source_category.as_deref()),
                item.installed_version,
                item.storage_path,
                item.favorite as i64,
                item.pinned as i64,
                item.notes,
                item.last_verified_at.map(|timestamp| timestamp.to_rfc3339()),
                now,
            ],
        )
        .map_err(|err| DaedalusError::Database(err.to_string()))?;

        self.find_library_item_by_storage_path(&item.storage_path)?
            .ok_or_else(|| DaedalusError::Database("failed to load library item after upsert".to_string()))
    }

    pub fn create_job(&self, kind: JobKind, summary: &str) -> Result<Job> {
        let conn = self.open_connection()?;
        let created_at = now_utc().to_rfc3339();
        conn.execute(
            "INSERT INTO jobs (kind, status, progress, summary, created_at) VALUES (?1, ?2, 0.0, ?3, ?4)",
            params![job_kind_to_str(kind), job_status_to_str(JobStatus::Queued), summary, created_at],
        )
        .map_err(|err| DaedalusError::Database(err.to_string()))?;
        let id = conn.last_insert_rowid();
        self.get_job(id)?
            .ok_or_else(|| DaedalusError::Database("failed to load job after insert".to_string()))
    }

    pub fn update_job_status(
        &self,
        id: i64,
        status: JobStatus,
        progress: f32,
        error_summary: Option<&str>,
    ) -> Result<Option<Job>> {
        let conn = self.open_connection()?;
        let started_at = if matches!(status, JobStatus::Running) {
            Some(now_utc().to_rfc3339())
        } else {
            None
        };
        let finished_at = if matches!(status, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled) {
            Some(now_utc().to_rfc3339())
        } else {
            None
        };

        conn.execute(
            r#"
            UPDATE jobs
            SET
                status = ?2,
                progress = ?3,
                error_summary = COALESCE(?4, error_summary),
                started_at = COALESCE(?5, started_at),
                finished_at = COALESCE(?6, finished_at)
            WHERE id = ?1
            "#,
            params![
                id,
                job_status_to_str(status),
                progress,
                error_summary,
                started_at,
                finished_at
            ],
        )
        .map_err(|err| DaedalusError::Database(err.to_string()))?;

        self.get_job(id)
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>> {
        let conn = self.open_connection()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, kind, status, progress, summary, error_summary, created_at, started_at, finished_at
                FROM jobs
                ORDER BY created_at DESC
                "#,
            )
            .map_err(|err| DaedalusError::Database(err.to_string()))?;

        let rows = stmt
            .query_map([], map_job)
            .map_err(|err| DaedalusError::Database(err.to_string()))?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row.map_err(|err| DaedalusError::Database(err.to_string()))?);
        }
        Ok(jobs)
    }

    pub fn get_job(&self, id: i64) -> Result<Option<Job>> {
        let conn = self.open_connection()?;
        conn.query_row(
            r#"
            SELECT id, kind, status, progress, summary, error_summary, created_at, started_at, finished_at
            FROM jobs
            WHERE id = ?1
            "#,
            [id],
            map_job,
        )
        .optional()
        .map_err(|err| DaedalusError::Database(err.to_string()))
    }

    fn open_connection(&self) -> Result<Connection> {
        Connection::open(&self.path).map_err(|err| DaedalusError::Database(err.to_string()))
    }
}

fn map_library_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryItem> {
    let source_kind: Option<String> = row.get(3)?;
    let source = match source_kind.as_deref() {
        Some(raw_kind) => Some(SourceRef {
            source_kind: parse_source_kind(raw_kind).map_err(to_sql_error)?,
            source_model_id: row.get(4)?,
            source_version_id: row.get(5)?,
            source_url: row.get(6)?,
            source_category: row.get(7)?,
        }),
        None => None,
    };

    Ok(LibraryItem {
        id: row.get(0)?,
        display_name: row.get(1)?,
        primary_model_kind: parse_model_kind(&row.get::<_, String>(2)?).map_err(to_sql_error)?,
        source,
        installed_version: row.get(8)?,
        storage_path: row.get(9)?,
        favorite: row.get::<_, i64>(10)? != 0,
        pinned: row.get::<_, i64>(11)? != 0,
        notes: row.get(12)?,
        last_verified_at: row
            .get::<_, Option<String>>(13)?
            .map(|value| parse_timestamp(&value))
            .transpose()
            .map_err(to_sql_error)?,
        created_at: parse_timestamp(&row.get::<_, String>(14)?).map_err(to_sql_error)?,
    })
}

fn map_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        id: row.get(0)?,
        kind: parse_job_kind(&row.get::<_, String>(1)?).map_err(to_sql_error)?,
        status: parse_job_status(&row.get::<_, String>(2)?).map_err(to_sql_error)?,
        progress: row.get(3)?,
        summary: row.get(4)?,
        error_summary: row.get(5)?,
        created_at: parse_timestamp(&row.get::<_, String>(6)?).map_err(to_sql_error)?,
        started_at: row
            .get::<_, Option<String>>(7)?
            .map(|value| parse_timestamp(&value))
            .transpose()
            .map_err(to_sql_error)?,
        finished_at: row
            .get::<_, Option<String>>(8)?
            .map(|value| parse_timestamp(&value))
            .transpose()
            .map_err(to_sql_error)?,
    })
}

fn parse_model_kind(value: &str) -> std::result::Result<ModelKind, String> {
    value.parse()
}

fn parse_source_kind(value: &str) -> std::result::Result<SourceKind, String> {
    match value {
        "civitai" => Ok(SourceKind::Civitai),
        "local" => Ok(SourceKind::Local),
        other => Err(format!("unknown source kind '{other}'")),
    }
}

fn parse_job_kind(value: &str) -> std::result::Result<JobKind, String> {
    match value {
        "download" => Ok(JobKind::Download),
        "sync" => Ok(JobKind::Sync),
        "rescan" => Ok(JobKind::Rescan),
        "import" => Ok(JobKind::Import),
        "preview_sync" => Ok(JobKind::PreviewSync),
        other => Err(format!("unknown job kind '{other}'")),
    }
}

fn parse_job_status(value: &str) -> std::result::Result<JobStatus, String> {
    match value {
        "queued" => Ok(JobStatus::Queued),
        "running" => Ok(JobStatus::Running),
        "completed" => Ok(JobStatus::Completed),
        "failed" => Ok(JobStatus::Failed),
        "cancelled" => Ok(JobStatus::Cancelled),
        other => Err(format!("unknown job status '{other}'")),
    }
}

fn parse_timestamp(value: &str) -> std::result::Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|err| err.to_string())
}

fn source_kind_to_str(value: SourceKind) -> &'static str {
    match value {
        SourceKind::Civitai => "civitai",
        SourceKind::Local => "local",
    }
}

fn job_kind_to_str(value: JobKind) -> &'static str {
    match value {
        JobKind::Download => "download",
        JobKind::Sync => "sync",
        JobKind::Rescan => "rescan",
        JobKind::Import => "import",
        JobKind::PreviewSync => "preview_sync",
    }
}

fn job_status_to_str(value: JobStatus) -> &'static str {
    match value {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn to_sql_error(err: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn initializes_and_lists_items() {
        let temp = tempdir().expect("tempdir");
        let db = CatalogDb::new(temp.path().join("catalog.db")).expect("db");
        let item = db
            .upsert_library_item(&LibraryItemInsert {
                display_name: "Example".to_string(),
                primary_model_kind: ModelKind::Checkpoint,
                source: None,
                installed_version: Some("v1".to_string()),
                storage_path: "/models/example/example.safetensors".to_string(),
                favorite: false,
                pinned: false,
                notes: None,
                last_verified_at: None,
            })
            .expect("insert");

        let items = db.list_library_items().expect("list");
        assert_eq!(items.len(), 1);
        assert_eq!(item.display_name, "Example");
        assert_eq!(items[0].storage_path, "/models/example/example.safetensors");
    }
}
