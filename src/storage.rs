use chrono::{DateTime, Utc};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<redb::DatabaseError> for StorageError {
    fn from(err: redb::DatabaseError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<redb::TransactionError> for StorageError {
    fn from(err: redb::TransactionError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<redb::TableError> for StorageError {
    fn from(err: redb::TableError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<redb::StorageError> for StorageError {
    fn from(err: redb::StorageError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<redb::CommitError> for StorageError {
    fn from(err: redb::CommitError) -> Self {
        StorageError::Database(err.to_string())
    }
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::Io(err.to_string())
    }
}

impl From<bincode::Error> for StorageError {
    fn from(err: bincode::Error) -> Self {
        StorageError::Serialization(err.to_string())
    }
}

// Table definitions
const TERMINOLOGY_VERSIONS: TableDefinition<u64, &[u8]> = TableDefinition::new("terminology_versions");
const TERMINOLOGY_VERSION_COUNTER: TableDefinition<&str, u64> = TableDefinition::new("version_counter");
const SNOMED_CONCEPTS: TableDefinition<&str, &[u8]> = TableDefinition::new("snomed_concepts");
const SNOMED_DESCRIPTIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("snomed_descriptions");
// AMT_CODES uses composite key (SCTID, code_type) because same SCTID can appear in multiple product types
const AMT_CODES: TableDefinition<(&str, &str), &[u8]> = TableDefinition::new("amt_codes");
const VALUESETS: TableDefinition<&str, &[u8]> = TableDefinition::new("valuesets");
const VALUESET_CONCEPTS: TableDefinition<(&str, &str), &[u8]> = TableDefinition::new("valueset_concepts");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminologyVersion {
    pub id: u64,
    pub terminology_type: String,
    pub version: String,
    pub effective_date: Option<String>,
    pub download_url: String,
    pub file_path: Option<String>,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub is_latest: bool,
    pub created_at: DateTime<Utc>,
    // NCTS extension fields
    pub content_item_identifier: Option<String>,
    pub content_item_version: Option<String>,
    pub sha256_hash: Option<String>,
    pub sct_base_version: Option<String>,
    // Import tracking
    pub imported: bool,
    pub imported_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnomedConcept {
    pub id: String,
    pub effective_time: String,
    pub active: bool,
    pub module_id: String,
    pub definition_status_id: String,
    pub version_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnomedDescription {
    pub id: String,
    pub effective_time: String,
    pub active: bool,
    pub module_id: String,
    pub concept_id: String,
    pub language_code: String,
    pub type_id: String,
    pub term: String,
    pub case_significance_id: String,
    pub version_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmtCode {
    pub id: String,
    pub preferred_term: String,
    pub code_type: String,
    pub parent_code: Option<String>,
    pub properties: Option<String>,
    pub version_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSet {
    pub url: String,
    pub version: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub version_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetConcept {
    pub valueset_url: String,
    pub system: String,
    pub code: String,
    pub display: Option<String>,
}

pub struct TerminologyStorage {
    db: Database,
    data_dir: PathBuf,
    db_path: PathBuf,
}

impl TerminologyStorage {
    /// Create a new storage instance with the given database path
    pub fn new(db_path: PathBuf, data_dir: PathBuf) -> Result<Self, StorageError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir)?;

        // Open or create redb database
        let db = Database::create(&db_path)?;

        let storage = Self {
            db,
            data_dir,
            db_path,
        };

        // Initialize tables (redb creates tables lazily, but we can ensure they exist)
        storage.initialize_tables()?;

        Ok(storage)
    }

    /// Initialize database tables
    fn initialize_tables(&self) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let _ = write_txn.open_table(TERMINOLOGY_VERSIONS)?;
            let _ = write_txn.open_table(TERMINOLOGY_VERSION_COUNTER)?;
            let _ = write_txn.open_table(SNOMED_CONCEPTS)?;
            let _ = write_txn.open_table(SNOMED_DESCRIPTIONS)?;
            let _ = write_txn.open_table(AMT_CODES)?;
            let _ = write_txn.open_table(VALUESETS)?;
            let _ = write_txn.open_table(VALUESET_CONCEPTS)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get next version ID (auto-increment)
    fn next_version_id(&self, write_txn: &redb::WriteTransaction) -> Result<u64, StorageError> {
        let mut table = write_txn.open_table(TERMINOLOGY_VERSION_COUNTER)?;
        let current = table.get("counter")?.map(|v| v.value()).unwrap_or(0);
        let next = current + 1;
        table.insert("counter", next)?;
        Ok(next)
    }

    /// Record a new terminology version with NCTS metadata
    pub fn record_version(
        &self,
        terminology_type: &str,
        version: &str,
        effective_date: Option<&str>,
        download_url: &str,
        content_item_identifier: Option<&str>,
        content_item_version: Option<&str>,
        sha256_hash: Option<&str>,
        sct_base_version: Option<&str>,
    ) -> Result<u64, StorageError> {
        let write_txn = self.db.begin_write()?;

        let version_id = {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            // Check if version already exists (using the already-opened table)
            let existing = Self::find_version_in_table(&table, terminology_type, version)?;

            if let Some(existing_version) = existing {
                // Update existing
                let mut updated = existing_version;
                updated.download_url = download_url.to_string();
                updated.effective_date = effective_date.map(|s| s.to_string());
                updated.content_item_identifier = content_item_identifier.map(|s| s.to_string());
                updated.content_item_version = content_item_version.map(|s| s.to_string());
                updated.sha256_hash = sha256_hash.map(|s| s.to_string());
                updated.sct_base_version = sct_base_version.map(|s| s.to_string());

                let bytes = bincode::serialize(&updated)?;
                table.insert(updated.id, bytes.as_slice())?;
                updated.id
            } else {
                // Create new - need to open counter table separately
                drop(table); // Release the table lock temporarily
                let id = self.next_version_id(&write_txn)?;
                let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

                let new_version = TerminologyVersion {
                    id,
                    terminology_type: terminology_type.to_string(),
                    version: version.to_string(),
                    effective_date: effective_date.map(|s| s.to_string()),
                    download_url: download_url.to_string(),
                    file_path: None,
                    downloaded_at: None,
                    is_latest: false,
                    created_at: Utc::now(),
                    content_item_identifier: content_item_identifier.map(|s| s.to_string()),
                    content_item_version: content_item_version.map(|s| s.to_string()),
                    sha256_hash: sha256_hash.map(|s| s.to_string()),
                    sct_base_version: sct_base_version.map(|s| s.to_string()),
                    imported: false,
                    imported_at: None,
                };

                let bytes = bincode::serialize(&new_version)?;
                table.insert(id, bytes.as_slice())?;
                id
            }
        };

        write_txn.commit()?;
        Ok(version_id)
    }

    /// Find version by terminology type and version string in an already-opened table
    fn find_version_in_table(
        table: &redb::Table<u64, &[u8]>,
        terminology_type: &str,
        version: &str,
    ) -> Result<Option<TerminologyVersion>, StorageError> {
        for item in table.iter()? {
            let (_, value) = item?;
            let ver: TerminologyVersion = bincode::deserialize(value.value())?;
            if ver.terminology_type == terminology_type && ver.version == version {
                return Ok(Some(ver));
            }
        }

        Ok(None)
    }

    /// Mark a version as downloaded and set its file path
    pub fn mark_downloaded(&self, id: u64, file_path: &str) -> Result<(), StorageError> {
        if file_path.is_empty() {
            return self.clear_downloaded(id);
        }

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            let version_bytes = match table.get(id)? {
                Some(value) => {
                    let bytes = value.value().to_vec();
                    drop(value);
                    Some(bytes)
                }
                None => None,
            };

            if let Some(bytes) = version_bytes {
                let mut version: TerminologyVersion = bincode::deserialize(&bytes)?;
                version.file_path = Some(file_path.to_string());
                version.downloaded_at = Some(Utc::now());

                let new_bytes = bincode::serialize(&version)?;
                table.insert(id, new_bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Clear download metadata (file_path and downloaded_at) for a version
    pub fn clear_downloaded(&self, id: u64) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            let version_bytes = match table.get(id)? {
                Some(value) => {
                    let bytes = value.value().to_vec();
                    drop(value);
                    Some(bytes)
                }
                None => None,
            };

            if let Some(bytes) = version_bytes {
                let mut version: TerminologyVersion = bincode::deserialize(&bytes)?;
                version.file_path = None;
                version.downloaded_at = None;

                let new_bytes = bincode::serialize(&version)?;
                table.insert(id, new_bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Mark a version as the latest for its terminology type
    pub fn mark_as_latest(&self, id: u64, terminology_type: &str) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            // Unmark all other versions for this terminology type
            let mut updates = Vec::new();
            for item in table.iter()? {
                let (key, value) = item?;
                let mut version: TerminologyVersion = bincode::deserialize(value.value())?;

                if version.terminology_type == terminology_type {
                    if version.id == id {
                        version.is_latest = true;
                    } else {
                        version.is_latest = false;
                    }
                    updates.push((key.value(), version));
                }
            }

            // Apply updates
            for (key, version) in updates {
                let bytes = bincode::serialize(&version)?;
                table.insert(key, bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get the latest version for a terminology type
    pub fn get_latest(&self, terminology_type: &str) -> Result<Option<TerminologyVersion>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TERMINOLOGY_VERSIONS)?;

        for item in table.iter()? {
            let (_, value) = item?;
            let version: TerminologyVersion = bincode::deserialize(value.value())?;

            if version.terminology_type == terminology_type && version.is_latest {
                return Ok(Some(version));
            }
        }

        Ok(None)
    }

    /// Get all versions for a terminology type
    pub fn get_all_versions(&self, terminology_type: &str) -> Result<Vec<TerminologyVersion>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TERMINOLOGY_VERSIONS)?;

        let mut versions = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let version: TerminologyVersion = bincode::deserialize(value.value())?;

            if version.terminology_type == terminology_type {
                versions.push(version);
            }
        }

        // Sort by created_at descending
        versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(versions)
    }

    /// Get all latest versions across all terminology types
    pub fn get_all_latest(&self) -> Result<Vec<TerminologyVersion>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TERMINOLOGY_VERSIONS)?;

        let mut versions = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let version: TerminologyVersion = bincode::deserialize(value.value())?;

            if version.is_latest {
                versions.push(version);
            }
        }

        // Sort by terminology_type
        versions.sort_by(|a, b| a.terminology_type.cmp(&b.terminology_type));

        Ok(versions)
    }

    /// Mark a version as imported
    pub fn mark_imported(&self, id: u64) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            let version_bytes = match table.get(id)? {
                Some(value) => {
                    let bytes = value.value().to_vec();
                    drop(value);
                    Some(bytes)
                }
                None => None,
            };

            if let Some(bytes) = version_bytes {
                let mut version: TerminologyVersion = bincode::deserialize(&bytes)?;
                version.imported = true;
                version.imported_at = Some(Utc::now());

                let new_bytes = bincode::serialize(&version)?;
                table.insert(id, new_bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Insert a SNOMED concept
    pub fn insert_snomed_concept(&self, concept: &SnomedConcept) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SNOMED_CONCEPTS)?;
            let bytes = bincode::serialize(concept)?;
            table.insert(concept.id.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Insert a SNOMED description
    pub fn insert_snomed_description(&self, description: &SnomedDescription) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SNOMED_DESCRIPTIONS)?;
            let bytes = bincode::serialize(description)?;
            table.insert(description.id.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a SNOMED concept by ID
    pub fn get_snomed_concept(&self, id: &str) -> Result<Option<SnomedConcept>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SNOMED_CONCEPTS)?;

        if let Some(value) = table.get(id)? {
            let concept: SnomedConcept = bincode::deserialize(value.value())?;
            Ok(Some(concept))
        } else {
            Ok(None)
        }
    }

    /// Get all descriptions for a SNOMED concept
    pub fn get_snomed_descriptions(&self, concept_id: &str) -> Result<Vec<SnomedDescription>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(SNOMED_DESCRIPTIONS)?;

        let mut descriptions = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let desc: SnomedDescription = bincode::deserialize(value.value())?;

            if desc.concept_id == concept_id {
                descriptions.push(desc);
            }
        }

        Ok(descriptions)
    }

    /// Insert an AMT code
    pub fn insert_amt_code(&self, code: &AmtCode) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(AMT_CODES)?;
            let bytes = bincode::serialize(code)?;
            // Use composite key (SCTID, code_type) to allow same SCTID across multiple product types
            table.insert((code.id.as_str(), code.code_type.as_str()), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get an AMT code by ID (returns first match across all product types)
    pub fn get_amt_code(&self, id: &str) -> Result<Option<AmtCode>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(AMT_CODES)?;

        // Since we use composite key (SCTID, code_type), find the first entry with matching SCTID
        for item in table.iter()? {
            let (key, value) = item?;
            let (sctid, _code_type) = key.value();
            if sctid == id {
                let code: AmtCode = bincode::deserialize(value.value())?;
                return Ok(Some(code));
            }
        }

        Ok(None)
    }

    /// Get all AMT codes (used for statistics/diagnostics)
    pub fn get_all_amt_codes(&self) -> Result<Vec<AmtCode>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(AMT_CODES)?;

        let mut codes = Vec::new();
        for entry in table.iter()? {
            let (_key, value) = entry?;
            let code: AmtCode = bincode::deserialize(value.value())?;
            codes.push(code);
        }

        Ok(codes)
    }

    /// Insert a ValueSet
    pub fn insert_valueset(&self, valueset: &ValueSet) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VALUESETS)?;
            let bytes = bincode::serialize(valueset)?;
            table.insert(valueset.url.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a ValueSet by URL
    pub fn get_valueset(&self, url: &str) -> Result<Option<ValueSet>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VALUESETS)?;

        if let Some(value) = table.get(url)? {
            let valueset: ValueSet = bincode::deserialize(value.value())?;
            Ok(Some(valueset))
        } else {
            Ok(None)
        }
    }

    /// Get all ValueSets
    pub fn get_all_valuesets(&self) -> Result<Vec<ValueSet>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VALUESETS)?;

        let mut valuesets = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let valueset: ValueSet = bincode::deserialize(value.value())?;
            valuesets.push(valueset);
        }

        Ok(valuesets)
    }

    /// Insert a ValueSet concept
    pub fn insert_valueset_concept(&self, concept: &ValueSetConcept) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VALUESET_CONCEPTS)?;
            let bytes = bincode::serialize(concept)?;
            table.insert(
                (concept.valueset_url.as_str(), concept.code.as_str()),
                bytes.as_slice(),
            )?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get all concepts in a ValueSet
    pub fn get_valueset_concepts(&self, valueset_url: &str) -> Result<Vec<ValueSetConcept>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VALUESET_CONCEPTS)?;

        let mut concepts = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let concept: ValueSetConcept = bincode::deserialize(value.value())?;

            if concept.valueset_url == valueset_url {
                concepts.push(concept);
            }
        }

        Ok(concepts)
    }

    /// Check if a code exists in a ValueSet
    pub fn valueset_contains_code(
        &self,
        valueset_url: &str,
        system: &str,
        code: &str,
    ) -> Result<bool, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VALUESET_CONCEPTS)?;

        for item in table.iter()? {
            let (_, value) = item?;
            let concept: ValueSetConcept = bincode::deserialize(value.value())?;

            if concept.valueset_url == valueset_url
                && concept.system == system
                && concept.code == code
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get the database file path
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Sanitize a version string to make it safe for use in filenames
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
        let safe_version = Self::sanitize_version_for_filename(version);

        let extension = match terminology_type {
            "snomed" => "zip",
            "amt" => "csv",
            "valuesets" => "json",
            "loinc" => "zip",
            _ => "zip",
        };
        let filename = format!("{}_{}.{}", terminology_type, safe_version, extension);
        self.data_dir.join(filename)
    }

    /// Get reference to the database for batch operations
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Delete all SNOMED data for a specific version
    pub fn delete_snomed_by_version(&self, version_id: u64) -> Result<i64, StorageError> {
        let mut deleted_count = 0i64;

        let write_txn = self.db.begin_write()?;
        {
            // Delete concepts
            let mut concepts_table = write_txn.open_table(SNOMED_CONCEPTS)?;
            let mut to_delete = Vec::new();

            for item in concepts_table.iter()? {
                let (key, value) = item?;
                let concept: SnomedConcept = bincode::deserialize(value.value())?;
                if concept.version_id == version_id {
                    to_delete.push(key.value().to_string());
                }
            }

            for key in &to_delete {
                concepts_table.remove(key.as_str())?;
                deleted_count += 1;
            }

            // Delete descriptions
            let mut descriptions_table = write_txn.open_table(SNOMED_DESCRIPTIONS)?;
            to_delete.clear();

            for item in descriptions_table.iter()? {
                let (key, value) = item?;
                let desc: SnomedDescription = bincode::deserialize(value.value())?;
                if desc.version_id == version_id {
                    to_delete.push(key.value().to_string());
                }
            }

            for key in &to_delete {
                descriptions_table.remove(key.as_str())?;
                deleted_count += 1;
            }
        }
        write_txn.commit()?;

        Ok(deleted_count)
    }

    /// Delete all AMT data for a specific version
    pub fn delete_amt_by_version(&self, version_id: u64) -> Result<i64, StorageError> {
        let mut deleted_count = 0i64;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(AMT_CODES)?;
            let mut to_delete = Vec::new();

            for item in table.iter()? {
                let (key, value) = item?;
                let code: AmtCode = bincode::deserialize(value.value())?;
                if code.version_id == version_id {
                    // Store composite key (SCTID, code_type)
                    let (sctid, code_type) = key.value();
                    to_delete.push((sctid.to_string(), code_type.to_string()));
                }
            }

            for (sctid, code_type) in &to_delete {
                table.remove((sctid.as_str(), code_type.as_str()))?;
                deleted_count += 1;
            }
        }
        write_txn.commit()?;

        Ok(deleted_count)
    }

    /// Delete all ValueSet data for a specific version
    pub fn delete_valuesets_by_version(&self, version_id: u64) -> Result<i64, StorageError> {
        let mut deleted_count = 0i64;

        let write_txn = self.db.begin_write()?;
        {
            // First, collect ValueSet URLs to delete
            let valuesets_table = write_txn.open_table(VALUESETS)?;
            let mut valueset_urls = Vec::new();

            for item in valuesets_table.iter()? {
                let (key, value) = item?;
                let valueset: ValueSet = bincode::deserialize(value.value())?;
                if valueset.version_id == version_id {
                    valueset_urls.push(key.value().to_string());
                }
            }

            // Delete ValueSet concepts for these URLs
            let mut concepts_table = write_txn.open_table(VALUESET_CONCEPTS)?;
            let mut concepts_to_delete = Vec::new();

            for item in concepts_table.iter()? {
                let (key, _) = item?;
                let (url, code) = key.value();
                if valueset_urls.contains(&url.to_string()) {
                    concepts_to_delete.push((url.to_string(), code.to_string()));
                }
            }

            for (url, code) in &concepts_to_delete {
                concepts_table.remove((url.as_str(), code.as_str()))?;
                deleted_count += 1;
            }

            // Delete ValueSets
            let mut valuesets_table = write_txn.open_table(VALUESETS)?;
            for url in &valueset_urls {
                valuesets_table.remove(url.as_str())?;
                deleted_count += 1;
            }
        }
        write_txn.commit()?;

        Ok(deleted_count)
    }

    /// Clear imported status for a version
    pub fn clear_imported_status(&self, version_id: u64) -> Result<(), StorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TERMINOLOGY_VERSIONS)?;

            let version_bytes = match table.get(version_id)? {
                Some(value) => {
                    let bytes = value.value().to_vec();
                    drop(value);
                    Some(bytes)
                }
                None => None,
            };

            if let Some(bytes) = version_bytes {
                let mut version: TerminologyVersion = bincode::deserialize(&bytes)?;
                version.imported = false;
                version.imported_at = None;

                let new_bytes = bincode::serialize(&version)?;
                table.insert(version_id, new_bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get all versions that are "ghost" records (have downloaded_at but no file_path)
    pub fn get_ghost_versions(&self) -> Result<Vec<u64>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TERMINOLOGY_VERSIONS)?;

        let mut ghost_ids = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let version: TerminologyVersion = bincode::deserialize(value.value())?;

            if version.downloaded_at.is_some() &&
               (version.file_path.is_none() || version.file_path.as_ref().map_or(true, |p| p.is_empty())) {
                ghost_ids.push(version.id);
            }
        }

        Ok(ghost_ids)
    }
}
