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
