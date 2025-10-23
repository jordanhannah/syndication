use crate::ncts::{FeedEntry, NctsClient, TerminologyType};
use crate::storage::{TerminologyStorage, TerminologyVersion};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub terminology_type: String,
    pub success: bool,
    pub latest_version: Option<String>,
    pub error: Option<String>,
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
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
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

        if existing_version == &version {
            return Ok(SyncResult {
                terminology_type: terminology_type.clone(),
                success: true,
                latest_version: Some(version.clone()),
                error: Some("Already up to date".to_string()),
            });
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

        // Download the file
        match state
            .ncts_client
            .download_terminology(download_url, &file_path)
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
pub async fn sync_all_terminologies(state: State<'_, AppState>) -> Result<Vec<SyncResult>, String> {
    let terminology_types = vec!["snomed", "valuesets", "amt"]; // LOINC excluded - proprietary binary only

    let mut results = Vec::new();

    for term_type in terminology_types {
        let result = sync_terminology(term_type.to_string(), state.clone()).await?;
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

    // Create importer
    let importer = crate::import::TerminologyImporter::new(&storage, version.id);

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
