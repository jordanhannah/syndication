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
    // Import tracking
    pub imported: bool,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub imported_at: Option<DateTime<Utc>>,
}

// Custom implementation to parse from SQLite string fields
impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for TerminologyVersion {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let downloaded_at: Option<String> = row.try_get("downloaded_at")?;
        let created_at: String = row.try_get("created_at")?;
        let imported_at: Option<String> = row.try_get("imported_at").ok().flatten();

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
            imported: row.try_get("imported").unwrap_or(false),
            imported_at: imported_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
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
        // Terminology versions metadata table
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
                imported BOOLEAN NOT NULL DEFAULT 0,
                imported_at TEXT,
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

        // SNOMED CT-AU tables (RF2 SNAPSHOT format)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS snomed_concepts (
                id TEXT PRIMARY KEY,
                effective_time TEXT NOT NULL,
                active INTEGER NOT NULL,
                module_id TEXT NOT NULL,
                definition_status_id TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_concepts_active
            ON snomed_concepts(active)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_concepts_version
            ON snomed_concepts(version_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS snomed_descriptions (
                id TEXT PRIMARY KEY,
                effective_time TEXT NOT NULL,
                active INTEGER NOT NULL,
                module_id TEXT NOT NULL,
                concept_id TEXT NOT NULL,
                language_code TEXT NOT NULL,
                type_id TEXT NOT NULL,
                term TEXT NOT NULL,
                case_significance_id TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                FOREIGN KEY (concept_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
                FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_descriptions_concept
            ON snomed_descriptions(concept_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_descriptions_active
            ON snomed_descriptions(active)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_descriptions_type
            ON snomed_descriptions(type_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_descriptions_term
            ON snomed_descriptions(term)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS snomed_relationships (
                id TEXT PRIMARY KEY,
                effective_time TEXT NOT NULL,
                active INTEGER NOT NULL,
                module_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                destination_id TEXT NOT NULL,
                relationship_group INTEGER NOT NULL,
                type_id TEXT NOT NULL,
                characteristic_type_id TEXT NOT NULL,
                modifier_id TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                FOREIGN KEY (source_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
                FOREIGN KEY (destination_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
                FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_relationships_source
            ON snomed_relationships(source_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_relationships_destination
            ON snomed_relationships(destination_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_relationships_type
            ON snomed_relationships(type_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_snomed_relationships_active
            ON snomed_relationships(active)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // AMT (Australian Medicines Terminology) tables - CSV format
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS amt_codes (
                id TEXT PRIMARY KEY,
                preferred_term TEXT NOT NULL,
                code_type TEXT NOT NULL,
                parent_code TEXT,
                properties TEXT,
                version_id INTEGER NOT NULL,
                FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_amt_codes_version
            ON amt_codes(version_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_amt_codes_type
            ON amt_codes(code_type)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_amt_codes_term
            ON amt_codes(preferred_term)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_amt_codes_parent
            ON amt_codes(parent_code)
            "#,
        )
        .execute(&self.pool)
        .await?;

        // FHIR R4 ValueSet tables - for ValueSet expansion and code validation
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS valuesets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                version TEXT,
                name TEXT,
                title TEXT,
                status TEXT,
                description TEXT,
                publisher TEXT,
                version_id INTEGER NOT NULL,
                FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valuesets_url
            ON valuesets(url)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valuesets_version_id
            ON valuesets(version_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS valueset_concepts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                valueset_id INTEGER NOT NULL,
                system TEXT NOT NULL,
                code TEXT NOT NULL,
                display TEXT,
                FOREIGN KEY (valueset_id) REFERENCES valuesets(id) ON DELETE CASCADE,
                UNIQUE(valueset_id, system, code)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valueset_concepts_valueset
            ON valueset_concepts(valueset_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valueset_concepts_code
            ON valueset_concepts(code)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valueset_concepts_system
            ON valueset_concepts(system)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_valueset_concepts_lookup
            ON valueset_concepts(valueset_id, system, code)
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
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version,
                   imported, imported_at
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
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version,
                   imported, imported_at
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
                   content_item_identifier, content_item_version, sha256_hash, sct_base_version,
                   imported, imported_at
            FROM terminology_versions
            WHERE is_latest = 1
            ORDER BY terminology_type
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(results)
    }

    /// Mark a version as imported
    pub async fn mark_imported(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE terminology_versions
            SET imported = 1, imported_at = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get access to the database pool for import operations
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Sanitize a version string to make it safe for use in filenames
    /// Replaces spaces with underscores and removes/replaces problematic characters
    fn sanitize_version_for_filename(version: &str) -> String {
        version
            .replace(' ', "_")
            .replace('/', "-")
            .replace('\\', "-")
            .replace('(', "")
            .replace(')', "")
            .replace('[', "")
            .replace(']', "")
            .replace('{', "")
            .replace('}', "")
            .replace(':', "-")
            .replace('*', "")
            .replace('?', "")
            .replace('"', "")
            .replace('<', "")
            .replace('>', "")
            .replace('|', "-")
    }

    /// Generate a file path for a terminology download
    pub fn generate_file_path(&self, terminology_type: &str, version: &str) -> PathBuf {
        // Sanitize version string to ensure valid filename
        let safe_version = Self::sanitize_version_for_filename(version);

        // Determine the correct file extension based on terminology type
        let extension = match terminology_type {
            "snomed" => "zip",     // SNOMED RF2 SNAPSHOT is a ZIP archive
            "amt" => "csv",        // AMT is in CSV format
            "valuesets" => "json", // FHIR R4 ValueSet Bundles are JSON
            "loinc" => "zip",      // LOINC (not used, but would be ZIP)
            _ => "zip",            // Default to ZIP for unknown types
        };
        let filename = format!("{}_{}.{}", terminology_type, safe_version, extension);
        self.data_dir.join(filename)
    }
}
