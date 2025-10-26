use crate::ncts::{FeedEntry, NctsClient, TerminologyType};
use crate::storage::{TerminologyStorage, TerminologyVersion};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub terminology_type: String,
    pub success: bool,
    pub latest_version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub phase: String,
    pub message: String,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageStats {
    pub snomed_concepts: i64,
    pub snomed_descriptions: i64,
    pub amt_codes: i64,
    pub valuesets: i64,
    pub database_size_mb: f64,
}

pub struct AppState {
    pub ncts_client: NctsClient,
    pub storage: Arc<Mutex<TerminologyStorage>>,
}

/// Fetch the latest version information for a terminology type
#[tauri::command]
pub async fn fetch_latest_version(
    terminology_type: String,
    state: State<'_, AppState>,
) -> Result<Option<FeedEntry>, String> {
    let term_type = parse_terminology_type(&terminology_type)?;

    state
        .ncts_client
        .fetch_latest(term_type)
        .await
        .map_err(|e| format!("Failed to fetch latest version: {}", e))
}

/// Fetch all available versions for a terminology type
#[tauri::command]
pub async fn fetch_all_versions(
    terminology_type: String,
    state: State<'_, AppState>,
) -> Result<Vec<FeedEntry>, String> {
    let term_type = parse_terminology_type(&terminology_type)?;

    state
        .ncts_client
        .fetch_feed(term_type)
        .await
        .map_err(|e| format!("Failed to fetch versions: {}", e))
}

/// Sync the latest version of a terminology type
#[tauri::command]
pub async fn sync_terminology(
    terminology_type: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
    println!("🔵 sync_terminology called for: {}", terminology_type);
    let term_type = parse_terminology_type(&terminology_type)?;

    // Fetch the latest version from NCTS
    let latest_entry = match state.ncts_client.fetch_latest(term_type.clone()).await {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            return Ok(SyncResult {
                terminology_type: terminology_type.clone(),
                success: false,
                latest_version: None,
                error: Some("No versions found".to_string()),
            });
        }
        Err(e) => {
            return Ok(SyncResult {
                terminology_type: terminology_type.clone(),
                success: false,
                latest_version: None,
                error: Some(format!("Failed to fetch: {}", e)),
            });
        }
    };

    let storage = state.storage.lock().await;

    // Use content_item_version if available, otherwise fall back to title
    let version = latest_entry.content_item_version.as_ref()
        .or(latest_entry.version.as_ref())
        .unwrap_or(&latest_entry.title)
        .clone();

    // Check if we already have this version
    let existing = storage
        .get_latest(&terminology_type)
        .await
        .map_err(|e| format!("Storage error: {}", e))?;

    if let Some(existing) = existing {
        // Compare using content_item_version if available, otherwise use version
        let existing_version = existing.content_item_version.as_ref()
            .unwrap_or(&existing.version);

        // Only skip download if version matches AND file actually exists on disk
        if existing_version == &version {
            if let Some(ref file_path) = existing.file_path {
                let path = std::path::Path::new(file_path);
                if path.exists() {
                    return Ok(SyncResult {
                        terminology_type: terminology_type.clone(),
                        success: true,
                        latest_version: Some(version.clone()),
                        error: Some("Already up to date".to_string()),
                    });
                } else {
                    println!("⚠ Version exists in database but file is missing, re-downloading...");
                }
            } else {
                println!("⚠ Version exists in database but no file_path recorded, downloading...");
            }
        }
    }

    // Record the new version with NCTS metadata
    let version_id = storage
        .record_version(
            &terminology_type,
            &version,
            latest_entry.effective_date.as_deref(),
            latest_entry.download_url.as_deref().unwrap_or(""),
            latest_entry.content_item_identifier.as_deref(),
            latest_entry.content_item_version.as_deref(),
            latest_entry.sha256_hash.as_deref(),
            latest_entry.sct_base_version.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to record version: {}", e))?;

    // Download the file if a download URL is available
    if let Some(download_url) = &latest_entry.download_url {
        let file_path = storage.generate_file_path(&terminology_type, &version);

        // Emit download start event
        let _ = app_handle.emit("sync-progress", SyncProgress {
            phase: "Downloading".to_string(),
            message: format!("Downloading {} from NCTS...", terminology_type),
            percentage: 0.0,
        });

        // Download the file
        match state
            .ncts_client
            .download_terminology(download_url, &file_path, Some(app_handle.clone()))
            .await
        {
            Ok(_) => {
                // Validate SHA-256 hash if provided (CP 94)
                if let Some(expected_hash) = &latest_entry.sha256_hash {
                    println!("Validating downloaded file hash...");
                    match NctsClient::validate_file_hash(&file_path, expected_hash).await {
                        Ok(_) => {
                            println!("✓ File integrity validated");
                        }
                        Err(e) => {
                            // Hash validation failed - delete the file and return error
                            let _ = tokio::fs::remove_file(&file_path).await;
                            return Ok(SyncResult {
                                terminology_type: terminology_type.clone(),
                                success: false,
                                latest_version: Some(version.clone()),
                                error: Some(format!("Hash validation failed: {}", e)),
                            });
                        }
                    }
                } else {
                    println!("⚠ Warning: No SHA-256 hash provided in feed, skipping validation");
                }

                // Mark as downloaded
                let file_path_str = file_path.to_str().unwrap().to_string();
                storage
                    .mark_downloaded(version_id, &file_path_str)
                    .await
                    .map_err(|e| format!("Failed to mark downloaded: {}", e))?;
            }
            Err(e) => {
                return Ok(SyncResult {
                    terminology_type: terminology_type.clone(),
                    success: false,
                    latest_version: Some(version.clone()),
                    error: Some(format!("Download failed: {}", e)),
                });
            }
        }
    }

    // Mark as latest
    storage
        .mark_as_latest(version_id, &terminology_type)
        .await
        .map_err(|e| format!("Failed to mark as latest: {}", e))?;

    Ok(SyncResult {
        terminology_type: terminology_type.clone(),
        success: true,
        latest_version: Some(version.clone()),
        error: None,
    })
}

/// Sync all terminology types (excludes LOINC - not available via syndication)
#[tauri::command]
pub async fn sync_all_terminologies(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<SyncResult>, String> {
    let terminology_types = vec!["snomed", "valuesets", "amt"]; // LOINC excluded - proprietary binary only

    let mut results = Vec::new();

    for term_type in terminology_types {
        let result = sync_terminology(term_type.to_string(), app_handle.clone(), state.clone()).await?;
        results.push(result);
    }

    Ok(results)
}

/// Get the latest locally stored version for a terminology type
#[tauri::command]
pub async fn get_local_latest(
    terminology_type: String,
    state: State<'_, AppState>,
) -> Result<Option<TerminologyVersion>, String> {
    let storage = state.storage.lock().await;

    storage
        .get_latest(&terminology_type)
        .await
        .map_err(|e| format!("Storage error: {}", e))
}

/// Get all locally stored versions for a terminology type
#[tauri::command]
pub async fn get_local_versions(
    terminology_type: String,
    state: State<'_, AppState>,
) -> Result<Vec<TerminologyVersion>, String> {
    let storage = state.storage.lock().await;

    storage
        .get_all_versions(&terminology_type)
        .await
        .map_err(|e| format!("Storage error: {}", e))
}

/// Get all latest versions across all terminology types
#[tauri::command]
pub async fn get_all_local_latest(
    state: State<'_, AppState>,
) -> Result<Vec<TerminologyVersion>, String> {
    let storage = state.storage.lock().await;

    storage
        .get_all_latest()
        .await
        .map_err(|e| format!("Storage error: {}", e))
}

/// Import terminology content into database
#[tauri::command]
pub async fn import_terminology(
    terminology_type: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;

    // Get the latest version for this terminology
    let version = storage
        .get_latest(&terminology_type)
        .await
        .map_err(|e| format!("Failed to get latest version: {}", e))?;

    let version = version.ok_or_else(|| {
        format!(
            "No downloaded version found for {}. Please sync first.",
            terminology_type
        )
    })?;

    if version.imported {
        return Ok(format!(
            "{} version {} already imported",
            terminology_type, version.version
        ));
    }

    let file_path = version
        .file_path
        .ok_or_else(|| format!("No file path found for {}", terminology_type))?;

    // Create importer with app handle for progress events
    let importer = crate::import::TerminologyImporter::new(&storage, version.id)
        .with_app_handle(app_handle);

    // Import based on terminology type
    match terminology_type.as_str() {
        "snomed" => {
            importer
                .import_snomed(std::path::Path::new(&file_path))
                .await
                .map_err(|e| format!("SNOMED import failed: {}", e))?;
        }
        "amt" => {
            importer
                .import_amt(std::path::Path::new(&file_path))
                .await
                .map_err(|e| format!("AMT import failed: {}", e))?;
        }
        "valuesets" => {
            importer
                .import_valuesets(std::path::Path::new(&file_path))
                .await
                .map_err(|e| format!("ValueSets import failed: {}", e))?;
        }
        _ => {
            return Err(format!("Unknown terminology type: {}", terminology_type));
        }
    }

    // Mark as imported
    storage
        .mark_imported(version.id)
        .await
        .map_err(|e| format!("Failed to mark as imported: {}", e))?;

    Ok(format!(
        "Successfully imported {} version {}",
        terminology_type, version.version
    ))
}

/// Search for codes across terminologies
#[tauri::command]
pub async fn search_terminology(
    query: String,
    terminology_types: Vec<String>,
    limit: Option<i32>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::queries::SearchResult>, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();
    let limit = limit.unwrap_or(20);

    if terminology_types.is_empty() || terminology_types.contains(&"all".to_string()) {
        // Search all terminologies
        crate::queries::TerminologyQueries::search_all(pool, &query, limit)
            .await
            .map_err(|e| format!("Search failed: {}", e))
    } else {
        let mut results = Vec::new();

        for term_type in terminology_types {
            match term_type.as_str() {
                "snomed" => {
                    let snomed_results =
                        crate::queries::TerminologyQueries::search_snomed(pool, &query, limit)
                            .await
                            .map_err(|e| format!("SNOMED search failed: {}", e))?;
                    results.extend(snomed_results);
                }
                "amt" => {
                    let amt_results =
                        crate::queries::TerminologyQueries::search_amt(pool, &query, limit)
                            .await
                            .map_err(|e| format!("AMT search failed: {}", e))?;
                    results.extend(amt_results);
                }
                "valuesets" => {
                    let valueset_results =
                        crate::queries::TerminologyQueries::search_valuesets(pool, &query, limit)
                            .await
                            .map_err(|e| format!("ValueSet search failed: {}", e))?;
                    results.extend(valueset_results);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

/// Look up a specific code with synonyms
#[tauri::command]
pub async fn lookup_code(
    code: String,
    system: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::queries::CodeLookupResult>, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    if system.contains("snomed") {
        crate::queries::TerminologyQueries::lookup_snomed_code(pool, &code)
            .await
            .map_err(|e| format!("Lookup failed: {}", e))
    } else if system.contains("amt") {
        crate::queries::TerminologyQueries::lookup_amt_code(pool, &code)
            .await
            .map_err(|e| format!("Lookup failed: {}", e))
    } else {
        Err(format!("Unsupported system: {}", system))
    }
}

/// Expand a ValueSet by URL
#[tauri::command]
pub async fn expand_valueset(
    valueset_url: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::queries::ValueSetExpansion>, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    crate::queries::TerminologyQueries::expand_valueset(pool, &valueset_url)
        .await
        .map_err(|e| format!("ValueSet expansion failed: {}", e))
}

/// Validate a code against a ValueSet
#[tauri::command]
pub async fn validate_code(
    code: String,
    system: String,
    valueset_url: String,
    state: State<'_, AppState>,
) -> Result<crate::queries::ValidationResult, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    crate::queries::TerminologyQueries::validate_code(pool, &code, &system, &valueset_url)
        .await
        .map_err(|e| format!("Code validation failed: {}", e))
}

/// List all available ValueSets
#[tauri::command]
pub async fn list_valuesets(
    state: State<'_, AppState>,
) -> Result<Vec<(String, Option<String>)>, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    crate::queries::TerminologyQueries::list_valuesets(pool)
        .await
        .map_err(|e| format!("Failed to list ValueSets: {}", e))
}

/// Get storage statistics (record counts)
#[tauri::command]
pub async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    // Query record counts from each table
    let snomed_concepts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snomed_concepts")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let snomed_descriptions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snomed_descriptions")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let amt_codes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM amt_codes")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let valuesets: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valuesets")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    // Get database file size
    let db_path = storage.db_path();
    let database_size_mb = match tokio::fs::metadata(&db_path).await {
        Ok(metadata) => metadata.len() as f64 / 1_048_576.0, // Convert bytes to MB
        Err(_) => 0.0,
    };

    Ok(StorageStats {
        snomed_concepts: snomed_concepts.0,
        snomed_descriptions: snomed_descriptions.0,
        amt_codes: amt_codes.0,
        valuesets: valuesets.0,
        database_size_mb,
    })
}

/// Test NCTS connection
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    // Test token acquisition
    match state.ncts_client.test_auth().await {
        Ok(_) => {
            // Try to fetch the feed to verify full connectivity
            match state
                .ncts_client
                .fetch_feed(TerminologyType::Snomed)
                .await
            {
                Ok(_) => Ok(ConnectionStatus {
                    connected: true,
                    message: "Successfully connected to NCTS".to_string(),
                    auth_ok: true,
                    feed_ok: true,
                }),
                Err(e) => Ok(ConnectionStatus {
                    connected: false,
                    message: format!("Authentication OK, but feed fetch failed: {}", e),
                    auth_ok: true,
                    feed_ok: false,
                }),
            }
        }
        Err(e) => Ok(ConnectionStatus {
            connected: false,
            message: format!("Authentication failed: {}", e),
            auth_ok: false,
            feed_ok: false,
        }),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub message: String,
    pub auth_ok: bool,
    pub feed_ok: bool,
}

/// Detailed storage information per terminology
#[derive(Debug, Serialize, Deserialize)]
pub struct DetailedStorageInfo {
    pub terminology_type: String,
    pub file_size_bytes: u64,
    pub file_path: Option<String>,
    pub database_records: i64,
    pub database_size_estimate_bytes: u64,
    pub total_size_bytes: u64,
    pub has_file: bool,
    pub has_database_data: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub terminologies: Vec<DetailedStorageInfo>,
    pub total_file_size_bytes: u64,
    pub total_database_size_bytes: u64,
    pub total_size_bytes: u64,
}

/// Get detailed storage breakdown for all terminologies
#[tauri::command]
pub async fn get_detailed_storage_info(
    state: State<'_, AppState>,
) -> Result<StorageBreakdown, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    let mut terminologies = Vec::new();
    let mut total_file_size = 0u64;

    // Get all terminology versions
    let all_versions = storage
        .get_all_latest()
        .await
        .map_err(|e| format!("Failed to get versions: {}", e))?;

    // Process each terminology type
    for terminology_type in &["snomed", "amt", "valuesets"] {
        let version = all_versions
            .iter()
            .find(|v| v.terminology_type == *terminology_type);

        // Get file size if file exists
        let (file_size, file_path_str, has_file) = if let Some(v) = version {
            if let Some(ref path_str) = v.file_path {
                let path = std::path::Path::new(path_str.as_str());
                if path.exists() {
                    match tokio::fs::metadata(path).await {
                        Ok(metadata) => (metadata.len(), Some(path_str.clone()), true),
                        Err(_) => (0, Some(path_str.clone()), false),
                    }
                } else {
                    (0, Some(path_str.clone()), false)
                }
            } else {
                (0, None, false)
            }
        } else {
            (0, None, false)
        };

        // Get database record counts
        let (db_records, has_data) = match *terminology_type {
            "snomed" => {
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM snomed_concepts WHERE version_id = (SELECT id FROM terminology_versions WHERE terminology_type = 'snomed' AND is_latest = 1)"
                )
                .fetch_one(pool)
                .await
                .unwrap_or((0,));
                (count.0, count.0 > 0)
            }
            "amt" => {
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM amt_codes WHERE version_id = (SELECT id FROM terminology_versions WHERE terminology_type = 'amt' AND is_latest = 1)"
                )
                .fetch_one(pool)
                .await
                .unwrap_or((0,));
                (count.0, count.0 > 0)
            }
            "valuesets" => {
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM valuesets WHERE version_id = (SELECT id FROM terminology_versions WHERE terminology_type = 'valuesets' AND is_latest = 1)"
                )
                .fetch_one(pool)
                .await
                .unwrap_or((0,));
                (count.0, count.0 > 0)
            }
            _ => (0, false),
        };

        // Estimate database size per terminology (rough approximation)
        // SNOMED: ~2KB per concept (with descriptions and relationships)
        // AMT: ~500 bytes per code
        // ValueSets: ~1KB per valueset
        let db_size_estimate = match *terminology_type {
            "snomed" => db_records as u64 * 2048,
            "amt" => db_records as u64 * 512,
            "valuesets" => db_records as u64 * 1024,
            _ => 0,
        };

        total_file_size += file_size;

        terminologies.push(DetailedStorageInfo {
            terminology_type: terminology_type.to_string(),
            file_size_bytes: file_size,
            file_path: file_path_str,
            database_records: db_records,
            database_size_estimate_bytes: db_size_estimate,
            total_size_bytes: file_size + db_size_estimate,
            has_file,
            has_database_data: has_data,
        });
    }

    // Get actual database file size
    let db_path = storage.db_path();
    let total_db_size = match tokio::fs::metadata(&db_path).await {
        Ok(metadata) => metadata.len(),
        Err(_) => 0,
    };

    Ok(StorageBreakdown {
        terminologies,
        total_file_size_bytes: total_file_size,
        total_database_size_bytes: total_db_size,
        total_size_bytes: total_file_size + total_db_size,
    })
}

/// Delete downloaded terminology file (keeps database data)
#[tauri::command]
pub async fn delete_terminology_file(
    terminology_type: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;

    // Get the version to find the file path
    let version = storage
        .get_latest(&terminology_type)
        .await
        .map_err(|e| format!("Failed to get version: {}", e))?;

    if let Some(version) = version {
        if let Some(file_path) = version.file_path {
            let path = std::path::Path::new(&file_path);
            if path.exists() {
                tokio::fs::remove_file(path)
                    .await
                    .map_err(|e| format!("Failed to delete file: {}", e))?;

                // Clear download metadata (file_path and downloaded_at)
                storage
                    .clear_downloaded(version.id)
                    .await
                    .map_err(|e| format!("Failed to update database: {}", e))?;

                Ok(format!("Deleted file: {}", file_path))
            } else {
                Err("File does not exist".to_string())
            }
        } else {
            Err("No file path recorded".to_string())
        }
    } else {
        Err("No version found".to_string())
    }
}

/// Delete imported database data for a terminology (keeps downloaded file)
#[tauri::command]
pub async fn delete_terminology_data(
    terminology_type: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    // Emit progress: Starting
    let _ = app_handle.emit("delete-progress", SyncProgress {
        phase: "Starting".to_string(),
        message: "Preparing to delete data...".to_string(),
        percentage: 0.0,
    });

    // Get the version ID
    let version = storage
        .get_latest(&terminology_type)
        .await
        .map_err(|e| format!("Failed to get version: {}", e))?;

    if let Some(version) = version {
        // Small delay so user sees "Starting" phase
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        // Emit progress: Counting
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Counting".to_string(),
            message: "Counting records to delete...".to_string(),
            percentage: 5.0,
        });

        // Count and delete records based on terminology type
        let deleted_count = match terminology_type.as_str() {
            "snomed" => {
                // Count concepts first
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM snomed_concepts WHERE version_id = ?"
                )
                .bind(version.id)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

                let total_count = count.0;

                // Small delay so user sees count
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Emit progress: Starting deletion
                let _ = app_handle.emit("delete-progress", SyncProgress {
                    phase: "Deleting".to_string(),
                    message: format!("Deleting {} SNOMED concepts...", total_count),
                    percentage: 10.0,
                });

                // Delete in batches with progress updates
                let batch_size = 10000;
                let mut total_deleted = 0i64;

                loop {
                    // Delete one batch
                    let result = sqlx::query("DELETE FROM snomed_concepts WHERE id IN (SELECT id FROM snomed_concepts WHERE version_id = ? LIMIT ?)")
                        .bind(version.id)
                        .bind(batch_size)
                        .execute(pool)
                        .await
                        .map_err(|e| format!("Failed to delete SNOMED concepts: {}", e))?;

                    let rows_deleted = result.rows_affected() as i64;
                    if rows_deleted == 0 {
                        break;
                    }

                    total_deleted += rows_deleted;

                    // Calculate progress (10% to 75% range for deletion)
                    let progress_percentage = if total_count > 0 {
                        10.0 + (total_deleted as f32 / total_count as f32 * 65.0)
                    } else {
                        75.0
                    };

                    // Emit progress update
                    let _ = app_handle.emit("delete-progress", SyncProgress {
                        phase: "Deleting".to_string(),
                        message: format!("Deleted {} / {} SNOMED concepts...", total_deleted, total_count),
                        percentage: progress_percentage,
                    });

                    // Small delay to allow UI updates
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                total_count
            }
            "amt" => {
                // Count codes first
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM amt_codes WHERE version_id = ?"
                )
                .bind(version.id)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

                let total_count = count.0;

                // Small delay so user sees count
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Emit progress: Starting deletion
                let _ = app_handle.emit("delete-progress", SyncProgress {
                    phase: "Deleting".to_string(),
                    message: format!("Deleting {} AMT codes...", total_count),
                    percentage: 10.0,
                });

                // Delete in batches with progress updates
                let batch_size = 5000;
                let mut total_deleted = 0i64;

                loop {
                    // Delete one batch
                    let result = sqlx::query("DELETE FROM amt_codes WHERE id IN (SELECT id FROM amt_codes WHERE version_id = ? LIMIT ?)")
                        .bind(version.id)
                        .bind(batch_size)
                        .execute(pool)
                        .await
                        .map_err(|e| format!("Failed to delete AMT codes: {}", e))?;

                    let rows_deleted = result.rows_affected() as i64;
                    if rows_deleted == 0 {
                        break;
                    }

                    total_deleted += rows_deleted;

                    // Calculate progress (10% to 75% range for deletion)
                    let progress_percentage = if total_count > 0 {
                        10.0 + (total_deleted as f32 / total_count as f32 * 65.0)
                    } else {
                        75.0
                    };

                    // Emit progress update
                    let _ = app_handle.emit("delete-progress", SyncProgress {
                        phase: "Deleting".to_string(),
                        message: format!("Deleted {} / {} AMT codes...", total_deleted, total_count),
                        percentage: progress_percentage,
                    });

                    // Small delay to allow UI updates
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                total_count
            }
            "valuesets" => {
                // Count valuesets first
                let count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM valuesets WHERE version_id = ?"
                )
                .bind(version.id)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

                let total_count = count.0;

                // Small delay so user sees count
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Emit progress: Starting deletion
                let _ = app_handle.emit("delete-progress", SyncProgress {
                    phase: "Deleting".to_string(),
                    message: format!("Deleting {} ValueSets...", total_count),
                    percentage: 10.0,
                });

                // Delete in batches with progress updates
                let batch_size = 100;
                let mut total_deleted = 0i64;

                loop {
                    // Delete one batch
                    let result = sqlx::query("DELETE FROM valuesets WHERE id IN (SELECT id FROM valuesets WHERE version_id = ? LIMIT ?)")
                        .bind(version.id)
                        .bind(batch_size)
                        .execute(pool)
                        .await
                        .map_err(|e| format!("Failed to delete ValueSets: {}", e))?;

                    let rows_deleted = result.rows_affected() as i64;
                    if rows_deleted == 0 {
                        break;
                    }

                    total_deleted += rows_deleted;

                    // Calculate progress (10% to 75% range for deletion)
                    let progress_percentage = if total_count > 0 {
                        10.0 + (total_deleted as f32 / total_count as f32 * 65.0)
                    } else {
                        75.0
                    };

                    // Emit progress update
                    let _ = app_handle.emit("delete-progress", SyncProgress {
                        phase: "Deleting".to_string(),
                        message: format!("Deleted {} / {} ValueSets...", total_deleted, total_count),
                        percentage: progress_percentage,
                    });

                    // Small delay to allow UI updates
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                total_count
            }
            _ => return Err("Unknown terminology type".to_string()),
        };

        // Emit progress: Updating version
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Updating".to_string(),
            message: "Updating version metadata...".to_string(),
            percentage: 80.0,
        });

        // Update version to mark as not imported
        sqlx::query("UPDATE terminology_versions SET imported = 0, imported_at = NULL WHERE id = ?")
            .bind(version.id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update version: {}", e))?;

        // Emit progress: Complete
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Complete".to_string(),
            message: format!("Deleted {} records", deleted_count),
            percentage: 100.0,
        });

        Ok(format!(
            "Deleted {} records for {}",
            deleted_count, terminology_type
        ))
    } else {
        Err("No version found".to_string())
    }
}

/// Delete both file and database data for a terminology
#[tauri::command]
pub async fn delete_all_terminology_data(
    terminology_type: String,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Delete database data first
    let db_result = delete_terminology_data(terminology_type.clone(), app_handle, state.clone()).await;

    // Then delete file
    let file_result = delete_terminology_file(terminology_type.clone(), state).await;

    match (db_result, file_result) {
        (Ok(db_msg), Ok(file_msg)) => Ok(format!("{}\n{}", db_msg, file_msg)),
        (Ok(db_msg), Err(_)) => Ok(db_msg), // File might not exist, that's ok
        (Err(db_err), Ok(file_msg)) => Ok(format!("{}\nWarning: {}", file_msg, db_err)),
        (Err(db_err), Err(file_err)) => {
            Err(format!("DB: {} | File: {}", db_err, file_err))
        }
    }
}

/// Clean up ghost version records (versions with timestamps but no files)
#[tauri::command]
pub async fn cleanup_ghost_versions(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let pool = storage.pool();

    // Find and clean up ghost records - versions that have downloaded_at but no file (or empty file_path)
    let result = sqlx::query(
        r#"
        UPDATE terminology_versions
        SET file_path = NULL, downloaded_at = NULL
        WHERE (file_path IS NULL OR file_path = '') AND downloaded_at IS NOT NULL
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to cleanup ghost versions: {}", e))?;

    let cleaned = result.rows_affected();

    if cleaned > 0 {
        Ok(format!("Cleaned up {} ghost version record(s)", cleaned))
    } else {
        Ok("No ghost records found".to_string())
    }
}

/// Helper function to parse terminology type string
fn parse_terminology_type(s: &str) -> Result<TerminologyType, String> {
    match s.to_lowercase().as_str() {
        "snomed" => Ok(TerminologyType::Snomed),
        "loinc" => Ok(TerminologyType::Loinc),
        "valuesets" => Ok(TerminologyType::ValueSets),
        "amt" => Ok(TerminologyType::Amt),
        _ => Err(format!("Unknown terminology type: {}", s)),
    }
}
