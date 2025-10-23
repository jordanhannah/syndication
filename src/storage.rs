use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Migration error: {0}")]
    Migration(String),
}

impl From<sqlx::Error> for StorageError {
    fn from(err: sqlx::Error) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::Io(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminologyVersion {
    pub id: i64,
    pub terminology_type: String,
    pub version: String,
    pub effective_date: Option<String>,
    pub download_url: String,
    pub file_path: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub downloaded_at: Option<DateTime<Utc>>,
    pub is_latest: bool,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    // NCTS extension fields
    pub content_item_identifier: Option<String>,
    pub content_item_version: Option<String>,
    pub sha256_hash: Option<String>,
    pub sct_base_version: Option<String>,
}

// Custom implementation to parse from SQLite string fields
impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for TerminologyVersion {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let downloaded_at: Option<String> = row.try_get("downloaded_at")?;
        let created_at: String = row.try_get("created_at")?;

        Ok(Self {
            id: row.try_get("id")?,
            terminology_type: row.try_get("terminology_type")?,
            version: row.try_get("version")?,
            effective_date: row.try_get("effective_date")?,
            download_url: row.try_get("download_url")?,
            file_path: row.try_get("file_path")?,
            downloaded_at: downloaded_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
            is_latest: row.try_get("is_latest")?,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            content_item_identifier: row.try_get("content_item_identifier").ok(),
            content_item_version: row.try_get("content_item_version").ok(),
            sha256_hash: row.try_get("sha256_hash").ok(),
            sct_base_version: row.try_get("sct_base_version").ok(),
        })
    }
}

pub struct TerminologyStorage {
    pool: SqlitePool,
    data_dir: PathBuf,
}

impl TerminologyStorage {
    /// Create a new storage instance with the given database path
    pub async fn new(db_path: PathBuf, data_dir: PathBuf) -> Result<Self, StorageError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)?;

        // SQLite connection string - need three slashes for absolute paths
        // Add ?mode=rwc to allow read, write, and create permissions
        let db_url = format!("sqlite:///{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await?;

        let storage = Self { pool, data_dir };
        storage.run_migrations().await?;

        Ok(storage)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS terminology_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                terminology_type TEXT NOT NULL,
                version TEXT NOT NULL,
                effective_date TEXT,
                download_url TEXT NOT NULL,
                file_path TEXT,
                downloaded_at TEXT,
                is_latest BOOLEAN NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                content_item_identifier TEXT,
                content_item_version TEXT,
                sha256_hash TEXT,
                sct_base_version TEXT,
                UNIQUE(terminology_type, version)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_terminology_type
            ON terminology_versions(terminology_type)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_is_latest
            ON terminology_versions(is_latest)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a new terminology version with NCTS metadata
    pub async fn record_version(
        &self,
        terminology_type: &str,
        version: &str,
        effective_date: Option<&str>,
        download_url: &str,
        content_item_identifier: Option<&str>,
        content_item_version: Option<&str>,
        sha256_hash: Option<&str>,
        sct_base_version: Option<&str>,
    ) -> Result<i64, StorageError> {
        let result = sqlx::query(
            r#"
            INSERT INTO terminology_versions
                (terminology_type, version, effective_date, download_url, created_at,
                 content_item_identifier, content_item_version, sha256_hash, sct_base_version)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(terminology_type, version) DO UPDATE SET
                download_url = excluded.download_url,
                effective_date = excluded.effective_date,
                content_item_identifier = excluded.content_item_identifier,
                content_item_version = excluded.content_item_version,
                sha256_hash = excluded.sha256_hash,
                sct_base_version = excluded.sct_base_version
            RETURNING id
            "#,
        )
        .bind(terminology_type)
        .bind(version)
        .bind(effective_date)
        .bind(download_url)
        .bind(Utc::now().to_rfc3339())
        .bind(content_item_identifier)
        .bind(content_item_version)
        .bind(sha256_hash)
        .bind(sct_base_version)
        .fetch_one(&self.pool)
        .await?;

        use sqlx::Row;
        Ok(result.get(0))
    }

    /// Mark a version as downloaded and set its file path
    pub async fn mark_downloaded(
        &self,
        id: i64,
        file_path: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE terminology_versions
            SET file_path = ?, downloaded_at = ?
            WHERE id = ?
            "#,
        )
        .bind(file_path)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark a version as the latest for its terminology type
    pub async fn mark_as_latest(
        &self,
        id: i64,
        terminology_type: &str,
    ) -> Result<(), StorageError> {
        // First, unmark all other versions for this terminology type
        sqlx::query(
            r#"
            UPDATE terminology_versions
            SET is_latest = 0
            WHERE terminology_type = ?
            "#,
        )
        .bind(terminology_type)
        .execute(&self.pool)
        .await?;

        // Then mark this version as latest
        sqlx::query(
            r#"
            UPDATE terminology_versions
            SET is_latest = 1
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the latest version for a terminology type
    pub async fn get_latest(
        &self,
        terminology_type: &str,
    ) -> Result<Option<TerminologyVersion>, StorageError> {
        let result = sqlx::query_as::<_, TerminologyVersion>(
            r#"
            SELECT id, terminology_type, version, effective_date,
                   download_url, file_path, downloaded_at, is_latest, created_at,
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version
            FROM terminology_versions
            WHERE terminology_type = ? AND is_latest = 1
            LIMIT 1
            "#,
        )
        .bind(terminology_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get all versions for a terminology type
    pub async fn get_all_versions(
        &self,
        terminology_type: &str,
    ) -> Result<Vec<TerminologyVersion>, StorageError> {
        let results = sqlx::query_as::<_, TerminologyVersion>(
            r#"
            SELECT id, terminology_type, version, effective_date,
                   download_url, file_path, downloaded_at, is_latest, created_at,
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version
            FROM terminology_versions
            WHERE terminology_type = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(terminology_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(results)
    }

    /// Get all latest versions across all terminology types
    pub async fn get_all_latest(&self) -> Result<Vec<TerminologyVersion>, StorageError> {
        let results = sqlx::query_as::<_, TerminologyVersion>(
            r#"
            SELECT id, terminology_type, version, effective_date,
                   download_url, file_path, downloaded_at, is_latest, created_at,
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version
            FROM terminology_versions
            WHERE is_latest = 1
            ORDER BY terminology_type
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(results)
    }

    /// Get the data directory path for storing downloaded files
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Generate a file path for a terminology download
    pub fn generate_file_path(&self, terminology_type: &str, version: &str) -> PathBuf {
        let filename = format!("{}_{}.zip", terminology_type, version);
        self.data_dir.join(filename)
    }
}
