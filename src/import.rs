use crate::parsers::{AmtCsvParser, SnomedRf2Parser, ValueSetR4Parser};
use crate::storage::TerminologyStorage;
use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::fs;

/// RAII guard for temporary directory cleanup
/// Automatically removes the directory when dropped, even on error/cancellation
struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        // Best-effort cleanup - ignore errors since we can't recover in Drop
        if self.path.exists() {
            println!("Cleaning up temporary directory: {:?}", self.path);
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ImportProgress {
    pub phase: String,
    pub phase_status: String, // "pending", "in_progress", "completed"
    pub current: usize,
    pub total: Option<usize>,
    pub percentage: f32,
    pub message: String,
}

/// Import terminology content into the database
pub struct TerminologyImporter<'a> {
    storage: &'a TerminologyStorage,
    version_id: i64,
    app_handle: Option<AppHandle>,
}

impl<'a> TerminologyImporter<'a> {
    pub fn new(storage: &'a TerminologyStorage, version_id: i64) -> Self {
        Self {
            storage,
            version_id,
            app_handle: None,
        }
    }

    /// Clean up any orphaned temporary extraction directories from previous runs
    /// Call this on app startup to prevent accumulation of temp files
    pub fn cleanup_orphaned_temp_dirs() -> Result<()> {
        let temp_base = std::env::temp_dir();

        println!("Checking for orphaned SNOMED extraction directories...");

        let entries = std::fs::read_dir(&temp_base)
            .context("Failed to read temp directory")?;

        let mut cleaned_count = 0;
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("snomed_extract_") {
                    let path = entry.path();
                    println!("Removing orphaned temp directory: {:?}", path);
                    if let Err(e) = std::fs::remove_dir_all(&path) {
                        eprintln!("Warning: Failed to remove {:?}: {}", path, e);
                    } else {
                        cleaned_count += 1;
                    }
                }
            }
        }

        if cleaned_count > 0 {
            println!("Cleaned up {} orphaned temp directories", cleaned_count);
        } else {
            println!("No orphaned temp directories found");
        }

        Ok(())
    }

    pub fn with_app_handle(mut self, app_handle: AppHandle) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    fn emit_progress(&self, progress: ImportProgress) {
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit("import-progress", progress);
        }
    }

    /// Count lines in a file (excluding header)
    fn count_file_lines(path: &Path) -> Result<usize> {
        use std::io::{BufRead, BufReader};
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open file for counting: {:?}", path))?;
        let reader = BufReader::new(file);
        let mut count = 0;
        let mut lines = reader.lines();

        // Skip header line
        if lines.next().is_some() {
            for _ in lines {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Import SNOMED CT-AU SNAPSHOT from ZIP file
    pub async fn import_snomed(&self, zip_path: &Path) -> Result<()> {
        println!("Importing SNOMED CT-AU from: {:?}", zip_path);

        self.emit_progress(ImportProgress {
            phase: "Extracting".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: None,
            percentage: 0.0,
            message: "Extracting SNOMED ZIP archive...".to_string(),
        });

        // Extract ZIP to temporary directory with RAII cleanup guard
        let temp_dir_path = self.extract_zip(zip_path).await?;
        let _temp_guard = TempDirGuard::new(temp_dir_path.clone());
        println!("Extracted to: {:?}", temp_dir_path);

        // Mark extraction as complete
        self.emit_progress(ImportProgress {
            phase: "Extracting".to_string(),
            phase_status: "completed".to_string(),
            current: 0,
            total: None,
            percentage: 100.0,
            message: "Extraction complete".to_string(),
        });

        self.emit_progress(ImportProgress {
            phase: "Locating Files".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: None,
            percentage: 0.0,
            message: "Locating RF2 files...".to_string(),
        });

        let pool = self.storage.pool();

        // Find RF2 SNAPSHOT files
        let concept_file = self.find_file(&temp_dir_path, "sct2_Concept_Snapshot").await?;
        let description_file = self
            .find_file(&temp_dir_path, "sct2_Description_Snapshot-en")
            .await?;
        let relationship_file = self
            .find_file(&temp_dir_path, "sct2_Relationship_Snapshot")
            .await?;

        println!("Found concept file: {:?}", concept_file);
        println!("Found description file: {:?}", description_file);
        println!("Found relationship file: {:?}", relationship_file);

        // Mark file location as complete
        self.emit_progress(ImportProgress {
            phase: "Locating Files".to_string(),
            phase_status: "completed".to_string(),
            current: 0,
            total: None,
            percentage: 100.0,
            message: "Files located".to_string(),
        });

        // Count file lines for accurate progress
        println!("Counting concepts...");
        let concept_total = Self::count_file_lines(&concept_file)?;
        println!("Found {} concepts to import", concept_total);

        println!("Counting descriptions...");
        let description_total = Self::count_file_lines(&description_file)?;
        println!("Found {} descriptions to import", description_total);

        println!("Counting relationships...");
        let relationship_total = Self::count_file_lines(&relationship_file)?;
        println!("Found {} relationships to import", relationship_total);

        // Import concepts with batch inserts
        println!("Importing concepts...");

        self.emit_progress(ImportProgress {
            phase: "Importing Concepts".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: Some(concept_total),
            percentage: 0.0,
            message: "Importing SNOMED concepts...".to_string(),
        });

        let mut concept_batch = Vec::new();
        let mut concept_count_tracker = 0;
        let concept_file_handle = std::fs::File::open(&concept_file)
            .context("Failed to open concepts file")?;
        let concept_reader = BufReader::new(concept_file_handle);
        let concept_count = SnomedRf2Parser::parse_concepts(concept_reader, |concept| {
            concept_batch.push(concept);

            // Batch insert every 1000 records
            if concept_batch.len() >= 1000 {
                let batch = std::mem::take(&mut concept_batch);
                concept_count_tracker += batch.len();

                // Emit progress every batch
                self.emit_progress(ImportProgress {
                    phase: "Importing Concepts".to_string(),
                    phase_status: "in_progress".to_string(),
                    current: concept_count_tracker,
                    total: Some(concept_total),
                    percentage: (concept_count_tracker as f32 / concept_total as f32 * 100.0).min(100.0),
                    message: format!("Imported {} concepts...", concept_count_tracker),
                });

                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(Self::insert_concept_batch(pool, self.version_id, batch))
                })?;
            }

            Ok(())
        })?;

        // Insert remaining concepts
        if !concept_batch.is_empty() {
            Self::insert_concept_batch(pool, self.version_id, concept_batch).await?;
        }

        println!("Imported {} concepts", concept_count);

        // Mark concepts as complete
        self.emit_progress(ImportProgress {
            phase: "Importing Concepts".to_string(),
            phase_status: "completed".to_string(),
            current: concept_count,
            total: Some(concept_count),
            percentage: 100.0,
            message: format!("Imported {} concepts", concept_count),
        });

        // Import descriptions with batch inserts
        println!("Importing descriptions...");

        self.emit_progress(ImportProgress {
            phase: "Importing Descriptions".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: Some(description_total),
            percentage: 0.0,
            message: "Importing SNOMED descriptions...".to_string(),
        });

        let mut description_batch = Vec::new();
        let mut description_count_tracker = 0;
        let description_file_handle = std::fs::File::open(&description_file)
            .context("Failed to open descriptions file")?;
        let description_reader = BufReader::new(description_file_handle);
        let description_count =
            SnomedRf2Parser::parse_descriptions(description_reader, |description| {
                description_batch.push(description);

                // Batch insert every 1000 records
                if description_batch.len() >= 1000 {
                    let batch = std::mem::take(&mut description_batch);
                    description_count_tracker += batch.len();

                    // Emit progress every batch
                    self.emit_progress(ImportProgress {
                        phase: "Importing Descriptions".to_string(),
                        phase_status: "in_progress".to_string(),
                        current: description_count_tracker,
                        total: Some(description_total),
                        percentage: (description_count_tracker as f32 / description_total as f32 * 100.0).min(100.0),
                        message: format!("Imported {} descriptions...", description_count_tracker),
                    });

                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(
                            Self::insert_description_batch(pool, self.version_id, batch),
                        )
                    })?;
                }

                Ok(())
            })?;

        // Insert remaining descriptions
        if !description_batch.is_empty() {
            Self::insert_description_batch(pool, self.version_id, description_batch).await?;
        }

        println!("Imported {} descriptions", description_count);

        // Mark descriptions as complete
        self.emit_progress(ImportProgress {
            phase: "Importing Descriptions".to_string(),
            phase_status: "completed".to_string(),
            current: description_count,
            total: Some(description_count),
            percentage: 100.0,
            message: format!("Imported {} descriptions", description_count),
        });

        // Import relationships with batch inserts
        println!("Importing relationships...");

        self.emit_progress(ImportProgress {
            phase: "Importing Relationships".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: Some(relationship_total),
            percentage: 0.0,
            message: "Importing SNOMED relationships...".to_string(),
        });

        let mut relationship_batch = Vec::new();
        let mut relationship_count_tracker = 0;
        let relationship_file_handle = std::fs::File::open(&relationship_file)
            .context("Failed to open relationships file")?;
        let relationship_reader = BufReader::new(relationship_file_handle);
        let relationship_count =
            SnomedRf2Parser::parse_relationships(relationship_reader, |relationship| {
                relationship_batch.push(relationship);

                // Batch insert every 1000 records
                if relationship_batch.len() >= 1000 {
                    let batch = std::mem::take(&mut relationship_batch);
                    relationship_count_tracker += batch.len();

                    // Emit progress every batch
                    self.emit_progress(ImportProgress {
                        phase: "Importing Relationships".to_string(),
                        phase_status: "in_progress".to_string(),
                        current: relationship_count_tracker,
                        total: Some(relationship_total),
                        percentage: (relationship_count_tracker as f32 / relationship_total as f32 * 100.0).min(100.0),
                        message: format!("Imported {} relationships...", relationship_count_tracker),
                    });

                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(
                            Self::insert_relationship_batch(pool, self.version_id, batch),
                        )
                    })?;
                }

                Ok(())
            })?;

        // Insert remaining relationships
        if !relationship_batch.is_empty() {
            Self::insert_relationship_batch(pool, self.version_id, relationship_batch).await?;
        }

        println!("Imported {} relationships", relationship_count);

        // Mark relationships as complete
        self.emit_progress(ImportProgress {
            phase: "Importing Relationships".to_string(),
            phase_status: "completed".to_string(),
            current: relationship_count,
            total: Some(relationship_count),
            percentage: 100.0,
            message: format!("Imported {} relationships", relationship_count),
        });

        // Cleanup happens automatically when _temp_guard is dropped
        self.emit_progress(ImportProgress {
            phase: "Complete".to_string(),
            phase_status: "completed".to_string(),
            current: concept_count + description_count + relationship_count,
            total: Some(concept_count + description_count + relationship_count),
            percentage: 100.0,
            message: format!(
                "Import complete! {} concepts, {} descriptions, {} relationships",
                concept_count, description_count, relationship_count
            ),
        });

        Ok(())
        // _temp_guard drops here, automatically cleaning up temp directory
    }

    /// Import AMT from CSV file
    pub async fn import_amt(&self, csv_path: &Path) -> Result<()> {
        println!("Importing AMT from: {:?}", csv_path);

        // Pass 1: Count actual codes (not CSV lines)
        // AMT CSV is wide-format - each row expands into multiple codes (CTPP, TPP, TPUU, etc.)
        println!("Counting AMT codes...");

        self.emit_progress(ImportProgress {
            phase: "Counting AMT".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: None,
            percentage: 0.0,
            message: "Counting AMT codes...".to_string(),
        });

        let amt_total = AmtCsvParser::count_codes(csv_path)
            .context("Failed to count AMT codes")?;
        println!("Found {} AMT codes to import", amt_total);

        self.emit_progress(ImportProgress {
            phase: "Counting AMT".to_string(),
            phase_status: "completed".to_string(),
            current: amt_total,
            total: Some(amt_total),
            percentage: 100.0,
            message: format!("Found {} AMT codes", amt_total),
        });

        // Pass 2: Import with accurate progress
        self.emit_progress(ImportProgress {
            phase: "Importing AMT".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: Some(amt_total),
            percentage: 0.0,
            message: "Importing AMT codes...".to_string(),
        });

        let pool = self.storage.pool();

        // Import AMT codes with batch inserts
        let mut batch = Vec::new();
        let mut count_tracker = 0;
        let count = AmtCsvParser::parse(csv_path, |code| {
            batch.push(code);

            // Batch insert every 1000 records
            if batch.len() >= 1000 {
                let batch_to_insert = std::mem::take(&mut batch);
                count_tracker += batch_to_insert.len();

                // Emit progress
                self.emit_progress(ImportProgress {
                    phase: "Importing AMT".to_string(),
                    phase_status: "in_progress".to_string(),
                    current: count_tracker,
                    total: Some(amt_total),
                    percentage: (count_tracker as f32 / amt_total as f32 * 100.0).min(100.0),
                    message: format!("Imported {} AMT codes...", count_tracker),
                });

                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(Self::insert_amt_batch(pool, self.version_id, batch_to_insert))
                })?;
            }

            Ok(())
        })?;

        // Insert remaining codes
        if !batch.is_empty() {
            Self::insert_amt_batch(pool, self.version_id, batch).await?;
        }

        println!("Imported {} AMT codes", count);

        // Mark AMT import as complete
        self.emit_progress(ImportProgress {
            phase: "Importing AMT".to_string(),
            phase_status: "completed".to_string(),
            current: count,
            total: Some(count),
            percentage: 100.0,
            message: format!("Imported {} AMT codes", count),
        });

        self.emit_progress(ImportProgress {
            phase: "Complete".to_string(),
            phase_status: "completed".to_string(),
            current: count,
            total: Some(count),
            percentage: 100.0,
            message: format!("Import complete! {} AMT codes imported", count),
        });

        Ok(())
    }

    /// Import FHIR ValueSets from JSON bundle
    pub async fn import_valuesets(&self, json_path: &Path) -> Result<()> {
        println!("Importing ValueSets from: {:?}", json_path);

        self.emit_progress(ImportProgress {
            phase: "Importing ValueSets".to_string(),
            phase_status: "in_progress".to_string(),
            current: 0,
            total: Some(500), // Typical ValueSet count
            percentage: 0.0,
            message: "Importing FHIR ValueSets...".to_string(),
        });

        let pool = self.storage.pool();

        let mut count_tracker = 0;
        let count = ValueSetR4Parser::parse_bundle(json_path, |valueset| {
            count_tracker += 1;

            // Emit progress every 10 valuesets
            if count_tracker % 10 == 0 {
                self.emit_progress(ImportProgress {
                    phase: "Importing ValueSets".to_string(),
                    phase_status: "in_progress".to_string(),
                    current: count_tracker,
                    total: Some(500),
                    percentage: (count_tracker as f32 / 500.0 * 100.0).min(100.0),
                    message: format!("Imported {} ValueSets...", count_tracker),
                });
            }

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(Self::insert_valueset(pool, self.version_id, valueset))
            })
        })?;

        println!("Imported {} ValueSets", count);

        // Mark ValueSets import as complete
        self.emit_progress(ImportProgress {
            phase: "Importing ValueSets".to_string(),
            phase_status: "completed".to_string(),
            current: count,
            total: Some(count),
            percentage: 100.0,
            message: format!("Imported {} ValueSets", count),
        });

        self.emit_progress(ImportProgress {
            phase: "Complete".to_string(),
            phase_status: "completed".to_string(),
            current: count,
            total: Some(count),
            percentage: 100.0,
            message: format!("Import complete! {} ValueSets imported", count),
        });

        Ok(())
    }

    /// Extract ZIP file to temporary directory
    async fn extract_zip(&self, zip_path: &Path) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir().join(format!(
            "snomed_extract_{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).await?;

        // Use std::fs for ZIP extraction (synchronous)
        let file = std::fs::File::open(zip_path)
            .context("Failed to open ZIP file")?;
        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to read ZIP archive")?;

        let total_files = archive.len();

        for i in 0..total_files {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => temp_dir.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            // Emit progress updates every 5 files or on every file if total < 50
            let update_interval = if total_files < 50 { 1 } else { 5 };
            if (i + 1) % update_interval == 0 || i + 1 == total_files {
                let percentage = ((i + 1) as f32 / total_files as f32 * 100.0).min(100.0);
                self.emit_progress(ImportProgress {
                    phase: "Extracting".to_string(),
                    phase_status: "in_progress".to_string(),
                    current: i + 1,
                    total: Some(total_files),
                    percentage,
                    message: format!("Extracting file {} of {}...", i + 1, total_files),
                });
            }
        }

        Ok(temp_dir)
    }

    /// Find a file by name pattern in directory (recursive)
    async fn find_file(&self, dir: &Path, pattern: &str) -> Result<PathBuf> {
        let mut dirs_to_search = vec![dir.to_path_buf()];

        while let Some(current_dir) = dirs_to_search.pop() {
            let mut entries = fs::read_dir(&current_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_file() {
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().contains(pattern) {
                            return Ok(path);
                        }
                    }
                } else if path.is_dir() {
                    dirs_to_search.push(path);
                }
            }
        }

        Err(anyhow::anyhow!("File matching '{}' not found", pattern))
    }

    /// Batch insert SNOMED concepts
    async fn insert_concept_batch(
        pool: &SqlitePool,
        version_id: i64,
        batch: Vec<crate::parsers::SnomedConcept>,
    ) -> Result<()> {
        for concept in batch {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO snomed_concepts
                    (id, effective_time, active, module_id, definition_status_id, version_id)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&concept.id)
            .bind(&concept.effective_time)
            .bind(concept.active as i32)
            .bind(&concept.module_id)
            .bind(&concept.definition_status_id)
            .bind(version_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Batch insert SNOMED descriptions
    async fn insert_description_batch(
        pool: &SqlitePool,
        version_id: i64,
        batch: Vec<crate::parsers::SnomedDescription>,
    ) -> Result<()> {
        for description in batch {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO snomed_descriptions
                    (id, effective_time, active, module_id, concept_id, language_code,
                     type_id, term, case_significance_id, version_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&description.id)
            .bind(&description.effective_time)
            .bind(description.active as i32)
            .bind(&description.module_id)
            .bind(&description.concept_id)
            .bind(&description.language_code)
            .bind(&description.type_id)
            .bind(&description.term)
            .bind(&description.case_significance_id)
            .bind(version_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Batch insert SNOMED relationships
    async fn insert_relationship_batch(
        pool: &SqlitePool,
        version_id: i64,
        batch: Vec<crate::parsers::SnomedRelationship>,
    ) -> Result<()> {
        for relationship in batch {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO snomed_relationships
                    (id, effective_time, active, module_id, source_id, destination_id,
                     relationship_group, type_id, characteristic_type_id, modifier_id, version_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&relationship.id)
            .bind(&relationship.effective_time)
            .bind(relationship.active as i32)
            .bind(&relationship.module_id)
            .bind(&relationship.source_id)
            .bind(&relationship.destination_id)
            .bind(relationship.relationship_group)
            .bind(&relationship.type_id)
            .bind(&relationship.characteristic_type_id)
            .bind(&relationship.modifier_id)
            .bind(version_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Batch insert AMT codes
    async fn insert_amt_batch(
        pool: &SqlitePool,
        version_id: i64,
        batch: Vec<crate::parsers::AmtCode>,
    ) -> Result<()> {
        for code in batch {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO amt_codes
                    (id, preferred_term, code_type, parent_code, properties, version_id)
                VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&code.id)
            .bind(&code.preferred_term)
            .bind(&code.code_type)
            .bind(&code.parent_code)
            .bind(&code.properties)
            .bind(version_id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Insert ValueSet and its expansion
    async fn insert_valueset(
        pool: &SqlitePool,
        version_id: i64,
        valueset: crate::parsers::ValueSetEntry,
    ) -> Result<()> {
        // Insert ValueSet metadata
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO valuesets
                (url, version, name, title, status, description, publisher, version_id)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&valueset.url)
        .bind(&valueset.version)
        .bind(&valueset.name)
        .bind(&valueset.title)
        .bind(&valueset.status)
        .bind(&valueset.description)
        .bind(&valueset.publisher)
        .bind(version_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = result {
            use sqlx::Row;
            let valueset_id: i64 = row.get(0);

            // Insert expansion concepts if present
            if let Some(expansion) = valueset.expansion {
                for concept in expansion {
                    sqlx::query(
                        r#"
                        INSERT OR IGNORE INTO valueset_concepts
                            (valueset_id, system, code, display)
                        VALUES (?, ?, ?, ?)
                        "#,
                    )
                    .bind(valueset_id)
                    .bind(&concept.system)
                    .bind(&concept.code)
                    .bind(&concept.display)
                    .execute(pool)
                    .await?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::panic;

    /// Test that TempDirGuard automatically cleans up directory on successful completion
    #[test]
    fn test_temp_dir_guard_cleanup_on_success() {
        let temp_base = std::env::temp_dir();
        let test_dir = temp_base.join("snomed_extract_test_success");

        // Create the directory
        fs::create_dir_all(&test_dir).expect("Failed to create test directory");
        assert!(test_dir.exists(), "Test directory should exist after creation");

        {
            // Create guard - directory should still exist
            let _guard = TempDirGuard::new(test_dir.clone());
            assert!(test_dir.exists(), "Directory should exist while guard is in scope");
        } // Guard drops here

        // Directory should be cleaned up automatically
        assert!(!test_dir.exists(), "Directory should be deleted after guard drops");
    }

    /// Test that TempDirGuard cleans up even when panic occurs
    #[test]
    fn test_temp_dir_guard_cleanup_on_panic() {
        let temp_base = std::env::temp_dir();
        let test_dir = temp_base.join("snomed_extract_test_panic");

        // Create the directory
        fs::create_dir_all(&test_dir).expect("Failed to create test directory");
        assert!(test_dir.exists(), "Test directory should exist after creation");

        // Catch panic to prevent test failure
        let result = panic::catch_unwind(|| {
            let _guard = TempDirGuard::new(test_dir.clone());
            assert!(test_dir.exists(), "Directory should exist before panic");
            panic!("Simulated panic during import");
        });

        // Panic should have occurred
        assert!(result.is_err(), "Should have panicked");

        // Directory should still be cleaned up despite panic
        assert!(!test_dir.exists(), "Directory should be deleted even after panic");
    }

    /// Test cleanup of orphaned directories from previous runs
    #[test]
    fn test_cleanup_orphaned_temp_dirs() {
        let temp_base = std::env::temp_dir();

        // Create some orphaned directories that should be cleaned up
        let orphan1 = temp_base.join("snomed_extract_12345");
        let orphan2 = temp_base.join("snomed_extract_67890");
        let orphan3 = temp_base.join("snomed_extract_test_cleanup");

        // Create a directory that should NOT be cleaned up (different prefix)
        let keep_dir = temp_base.join("other_temp_dir");

        // Create all test directories
        fs::create_dir_all(&orphan1).expect("Failed to create orphan1");
        fs::create_dir_all(&orphan2).expect("Failed to create orphan2");
        fs::create_dir_all(&orphan3).expect("Failed to create orphan3");
        fs::create_dir_all(&keep_dir).expect("Failed to create keep_dir");

        // Add some files to make sure recursive deletion works
        fs::write(orphan1.join("test.txt"), "test content").expect("Failed to write test file");
        fs::write(keep_dir.join("keep.txt"), "keep content").expect("Failed to write keep file");

        // Verify all directories exist
        assert!(orphan1.exists(), "Orphan1 should exist before cleanup");
        assert!(orphan2.exists(), "Orphan2 should exist before cleanup");
        assert!(orphan3.exists(), "Orphan3 should exist before cleanup");
        assert!(keep_dir.exists(), "Keep_dir should exist before cleanup");

        // Run cleanup
        let result = TerminologyImporter::cleanup_orphaned_temp_dirs();
        assert!(result.is_ok(), "Cleanup should succeed");

        // Verify orphaned directories are removed
        assert!(!orphan1.exists(), "Orphan1 should be deleted");
        assert!(!orphan2.exists(), "Orphan2 should be deleted");
        assert!(!orphan3.exists(), "Orphan3 should be deleted");

        // Verify unrelated directory is kept
        assert!(keep_dir.exists(), "Keep_dir should still exist");

        // Clean up the keep_dir manually
        fs::remove_dir_all(&keep_dir).expect("Failed to clean up keep_dir");
    }
}
