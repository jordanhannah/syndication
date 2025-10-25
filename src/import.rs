use crate::parsers::{AmtCsvParser, SnomedRf2Parser, ValueSetR4Parser};
use crate::storage::TerminologyStorage;
use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Import terminology content into the database
pub struct TerminologyImporter<'a> {
    storage: &'a TerminologyStorage,
    version_id: i64,
}

impl<'a> TerminologyImporter<'a> {
    pub fn new(storage: &'a TerminologyStorage, version_id: i64) -> Self {
        Self {
            storage,
            version_id,
        }
    }

    /// Import SNOMED CT-AU SNAPSHOT from ZIP file
    pub async fn import_snomed(&self, zip_path: &Path) -> Result<()> {
        println!("Importing SNOMED CT-AU from: {:?}", zip_path);

        // Extract ZIP to temporary directory
        let temp_dir = self.extract_zip(zip_path).await?;
        println!("Extracted to: {:?}", temp_dir);

        let pool = self.storage.pool();

        // Find RF2 SNAPSHOT files
        let concept_file = self.find_file(&temp_dir, "sct2_Concept_Snapshot").await?;
        let description_file = self
            .find_file(&temp_dir, "sct2_Description_Snapshot-en")
            .await?;
        let relationship_file = self
            .find_file(&temp_dir, "sct2_Relationship_Snapshot")
            .await?;

        println!("Found concept file: {:?}", concept_file);
        println!("Found description file: {:?}", description_file);
        println!("Found relationship file: {:?}", relationship_file);

        // Import concepts with batch inserts
        println!("Importing concepts...");
        let mut concept_batch = Vec::new();
        let concept_file_handle = std::fs::File::open(&concept_file)
            .context("Failed to open concepts file")?;
        let concept_reader = BufReader::new(concept_file_handle);
        let concept_count = SnomedRf2Parser::parse_concepts(concept_reader, |concept| {
            concept_batch.push(concept);

            // Batch insert every 1000 records
            if concept_batch.len() >= 1000 {
                let batch = std::mem::take(&mut concept_batch);
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

        // Import descriptions with batch inserts
        println!("Importing descriptions...");
        let mut description_batch = Vec::new();
        let description_file_handle = std::fs::File::open(&description_file)
            .context("Failed to open descriptions file")?;
        let description_reader = BufReader::new(description_file_handle);
        let description_count =
            SnomedRf2Parser::parse_descriptions(description_reader, |description| {
                description_batch.push(description);

                // Batch insert every 1000 records
                if description_batch.len() >= 1000 {
                    let batch = std::mem::take(&mut description_batch);
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

        // Import relationships with batch inserts
        println!("Importing relationships...");
        let mut relationship_batch = Vec::new();
        let relationship_file_handle = std::fs::File::open(&relationship_file)
            .context("Failed to open relationships file")?;
        let relationship_reader = BufReader::new(relationship_file_handle);
        let relationship_count =
            SnomedRf2Parser::parse_relationships(relationship_reader, |relationship| {
                relationship_batch.push(relationship);

                // Batch insert every 1000 records
                if relationship_batch.len() >= 1000 {
                    let batch = std::mem::take(&mut relationship_batch);
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

        // Clean up temporary directory
        fs::remove_dir_all(&temp_dir).await?;

        Ok(())
    }

    /// Import AMT from CSV file
    pub async fn import_amt(&self, csv_path: &Path) -> Result<()> {
        println!("Importing AMT from: {:?}", csv_path);

        let pool = self.storage.pool();

        // Import AMT codes with batch inserts
        let mut batch = Vec::new();
        let count = AmtCsvParser::parse(csv_path, |code| {
            batch.push(code);

            // Batch insert every 1000 records
            if batch.len() >= 1000 {
                let batch_to_insert = std::mem::take(&mut batch);
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

        Ok(())
    }

    /// Import FHIR ValueSets from JSON bundle
    pub async fn import_valuesets(&self, json_path: &Path) -> Result<()> {
        println!("Importing ValueSets from: {:?}", json_path);

        let pool = self.storage.pool();

        let count = ValueSetR4Parser::parse_bundle(json_path, |valueset| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(Self::insert_valueset(pool, self.version_id, valueset))
            })
        })?;

        println!("Imported {} ValueSets", count);

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

        for i in 0..archive.len() {
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
