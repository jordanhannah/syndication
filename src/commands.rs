use crate::import::TerminologyImporter;
use crate::ncts::{FeedEntry, NctsClient, TerminologyType};
use crate::queries::TerminologyQueries;
use crate::search::TerminologySearch;
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
    pub searcher: Arc<Mutex<TerminologySearch>>,
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
    let mut searcher = state.searcher.lock().await;

    // Get the latest version for this terminology
    let version = storage
        .get_latest(&terminology_type)
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
    let importer = TerminologyImporter::new(&storage, version.id)
        .with_app_handle(app_handle);

    // Import based on terminology type (passing searcher for index building)
    match terminology_type.as_str() {
        "snomed" => {
            importer
                .import_snomed(std::path::Path::new(&file_path), &mut searcher)
                .await
                .map_err(|e| format!("SNOMED import failed: {}", e))?;
        }
        "amt" => {
            importer
                .import_amt(std::path::Path::new(&file_path), &mut searcher)
                .await
                .map_err(|e| format!("AMT import failed: {}", e))?;
        }
        "valuesets" => {
            importer
                .import_valuesets(std::path::Path::new(&file_path), &mut searcher)
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
) -> Result<Vec<crate::search::SearchResult>, String> {
    let searcher = state.searcher.lock().await;
    let limit = limit.unwrap_or(20) as usize;

    if terminology_types.is_empty() || terminology_types.contains(&"all".to_string()) {
        // Search all terminologies
        TerminologyQueries::search_all(&searcher, &query, limit)
            .map_err(|e| format!("Search failed: {}", e))
    } else {
        let mut results = Vec::new();

        for term_type in terminology_types {
            match term_type.as_str() {
                "snomed" => {
                    let snomed_results =
                        TerminologyQueries::search_snomed(&searcher, &query, limit)
                            .map_err(|e| format!("SNOMED search failed: {}", e))?;
                    results.extend(snomed_results);
                }
                "amt" => {
                    let amt_results =
                        TerminologyQueries::search_amt(&searcher, &query, limit, None)
                            .map_err(|e| format!("AMT search failed: {}", e))?;
                    results.extend(amt_results);
                }
                "valuesets" => {
                    let valueset_results =
                        TerminologyQueries::search_valuesets(&searcher, &query, limit)
                            .map_err(|e| format!("ValueSet search failed: {}", e))?;
                    results.extend(valueset_results);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

/// Search AMT codes for patient use (MP PT and TPP TP PT columns only)
/// Returns Medicinal Product (MP) and Trade Product Pack (TPP TP) terms for patient-facing searches
#[tauri::command]
pub async fn search_amt_patient(
    query: String,
    limit: Option<i32>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::search::SearchResult>, String> {
    let searcher = state.searcher.lock().await;
    let limit = limit.unwrap_or(20) as usize;

    // Filter to MP (Medicinal Product) and TPP TP (Trade Product Pack - TP) only
    // Indexes the MP PT and TPP TP PT columns from AMT CSV
    let code_types = vec!["MP".to_string(), "TPP TP".to_string()];

    TerminologyQueries::search_amt(&searcher, &query, limit, Some(&code_types))
        .map_err(|e| format!("AMT patient search failed: {}", e))
}

/// Search AMT codes for doctor use (MP PT, MPUU PT, TPP TP PT, TPUU PT columns)
/// Returns Medicinal Product, Medicinal Product Unit of Use, Trade Product Pack, and Trade Product Unit of Use terms
/// Filters to: MP, MPUU, TPP TP, TPUU TP (4 types as per CLAUDE.md spec)
#[tauri::command]
pub async fn search_amt_doctor(
    query: String,
    limit: Option<i32>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::search::SearchResult>, String> {
    let searcher = state.searcher.lock().await;
    let limit = limit.unwrap_or(20) as usize;

    // Filter to doctor-relevant types: MP, MPUU, TPP TP, TPUU TP
    let code_types = vec![
        "MP".to_string(),
        "MPUU".to_string(),
        "TPP TP".to_string(),
        "TPUU TP".to_string()
    ];
    TerminologyQueries::search_amt(&searcher, &query, limit, Some(&code_types))
        .map_err(|e| format!("AMT doctor search failed: {}", e))
}

/// Get AMT code type statistics for diagnostics
#[derive(Debug, Serialize, Deserialize)]
pub struct AmtCodeTypeStats {
    pub code_type: String,
    pub count: usize,
}

#[tauri::command]
pub async fn get_amt_code_type_stats(
    state: State<'_, AppState>,
) -> Result<Vec<AmtCodeTypeStats>, String> {
    let storage = state.storage.lock().await;

    // Get all AMT codes
    let all_codes = storage
        .get_all_amt_codes()
        .map_err(|e| format!("Failed to get AMT codes: {}", e))?;

    // Count by code type
    let mut type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for code in all_codes {
        *type_counts.entry(code.code_type.clone()).or_insert(0) += 1;
    }

    // Convert to sorted vector
    let mut stats: Vec<AmtCodeTypeStats> = type_counts
        .into_iter()
        .map(|(code_type, count)| AmtCodeTypeStats { code_type, count })
        .collect();

    // Sort by count descending
    stats.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(stats)
}

/// Debug command: Get all AMT codes matching a drug name, grouped by product type
#[derive(Debug, Serialize, Deserialize)]
pub struct AmtCodeVariant {
    pub code: String,
    pub preferred_term: String,
    pub code_type: String,
}

#[tauri::command]
pub async fn debug_amt_codes(
    drug_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<AmtCodeVariant>, String> {
    let storage = state.storage.lock().await;

    // Get all AMT codes
    let all_codes = storage
        .get_all_amt_codes()
        .map_err(|e| format!("Failed to get AMT codes: {}", e))?;

    // Filter by drug name (case-insensitive)
    let drug_name_lower = drug_name.to_lowercase();
    let mut matching_codes: Vec<AmtCodeVariant> = all_codes
        .into_iter()
        .filter(|code| code.preferred_term.to_lowercase().contains(&drug_name_lower))
        .map(|code| AmtCodeVariant {
            code: code.id,
            preferred_term: code.preferred_term,
            code_type: code.code_type,
        })
        .collect();

    // Sort by code type, then by preferred term
    matching_codes.sort_by(|a, b| {
        a.code_type.cmp(&b.code_type)
            .then_with(|| a.preferred_term.cmp(&b.preferred_term))
    });

    Ok(matching_codes)
}

/// Rebuild the AMT Tantivy index from redb storage
/// Use this to force a complete reindex if search isn't working
#[tauri::command]
pub async fn rebuild_amt_index(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let mut searcher = state.searcher.lock().await;

    println!("Rebuilding AMT Tantivy index from redb storage...");

    // Clear existing index
    searcher.clear_amt()
        .map_err(|e| format!("Failed to clear AMT index: {}", e))?;

    // Read all AMT codes from redb and re-index them
    let all_codes = storage
        .get_all_amt_codes()
        .map_err(|e| format!("Failed to read AMT codes: {}", e))?;

    let total = all_codes.len();
    let mut indexed = 0;
    let mut type_counts = std::collections::HashMap::new();

    for code in all_codes {
        searcher.index_amt_code(&code.id, &code.preferred_term, &code.code_type)
            .map_err(|e| format!("Failed to index code {}: {}", code.id, e))?;

        *type_counts.entry(code.code_type.clone()).or_insert(0) += 1;
        indexed += 1;

        if indexed % 1000 == 0 {
            println!("Indexed {} / {} AMT codes...", indexed, total);
        }
    }

    // Commit the index
    searcher.commit()
        .map_err(|e| format!("Failed to commit AMT index: {}", e))?;

    println!("AMT index rebuild complete: {} codes indexed", indexed);

    // Build breakdown message
    let mut breakdown: Vec<_> = type_counts.iter().collect();
    breakdown.sort_by(|a, b| b.1.cmp(a.1));
    let breakdown_str = breakdown.iter()
        .map(|(k, v)| format!("  {}: {} codes", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "Successfully rebuilt AMT index: {} total codes indexed\n\nBreakdown by type:\n{}",
        indexed, breakdown_str
    ))
}

/// Diagnose AMT index health - compares storage vs index
#[tauri::command]
pub async fn diagnose_amt_index(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let storage = state.storage.lock().await;
    let searcher = state.searcher.lock().await;

    // Check redb storage
    let all_codes = storage
        .get_all_amt_codes()
        .map_err(|e| format!("Failed to read AMT codes: {}", e))?;

    let mut type_counts = std::collections::HashMap::new();
    for code in &all_codes {
        *type_counts.entry(code.code_type.clone()).or_insert(0) += 1;
    }

    let mut storage_breakdown: Vec<_> = type_counts.iter().collect();
    storage_breakdown.sort_by(|a, b| b.1.cmp(a.1));
    let storage_summary = format!(
        "Storage (redb): {} total codes\n{}",
        all_codes.len(),
        storage_breakdown.iter()
            .map(|(k, v)| format!("  {}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Test searches
    let test_queries = vec![
        ("rivaroxaban", None),
        ("rivaroxaban", Some(vec!["MP".to_string()])),
        ("rivaroxaban", Some(vec!["MP".to_string(), "MPUU".to_string()])),
        ("xarelto", None),
        ("xarelto", Some(vec!["TPP TP".to_string()])),
    ];

    let mut search_results = Vec::new();
    for (query, filter) in test_queries {
        let filter_ref = filter.as_ref().map(|v| v.as_slice());
        let results = searcher.search_amt(query, 10, filter_ref)
            .map_err(|e| format!("Search failed for '{}': {}", query, e))?;

        let filter_str = match filter {
            Some(ref f) => format!(" [filter: {}]", f.join(", ")),
            None => " [no filter]".to_string(),
        };
        search_results.push(format!("  '{}'{}: {} results", query, filter_str, results.len()));
    }

    let search_summary = format!(
        "Search Tests:\n{}",
        search_results.join("\n")
    );

    Ok(format!("{}\n\n{}", storage_summary, search_summary))
}

/// Look up a specific code with synonyms
#[tauri::command]
pub async fn lookup_code(
    code: String,
    system: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::queries::CodeLookupResult>, String> {
    let storage = state.storage.lock().await;

    if system.contains("snomed") {
        TerminologyQueries::lookup_snomed_code(&storage, &code)
            .map_err(|e| format!("Lookup failed: {}", e))
    } else if system.contains("amt") {
        TerminologyQueries::lookup_amt_code(&storage, &code)
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

    TerminologyQueries::expand_valueset(&storage, &valueset_url)
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

    TerminologyQueries::validate_code(&storage, &code, &system, &valueset_url)
        .map_err(|e| format!("Code validation failed: {}", e))
}

/// List all available ValueSets
#[tauri::command]
pub async fn list_valuesets(
    state: State<'_, AppState>,
) -> Result<Vec<crate::queries::ValueSetListItem>, String> {
    let storage = state.storage.lock().await;

    TerminologyQueries::list_valuesets(&storage)
        .map_err(|e| format!("Failed to list ValueSets: {}", e))
}

/// Get storage statistics (record counts)
/// TEMPORARILY DISABLED - requires table iteration optimization for redb
// #[tauri::command]
// pub async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
//     // TODO: Implement with redb table iteration
//     Err("Storage stats temporarily disabled during redb migration".to_string())
// }

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

    let mut terminologies = Vec::new();
    let mut total_file_size = 0u64;

    // Get all terminology versions
    let all_versions = storage
        .get_all_latest()
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

        // Record counting temporarily disabled during redb migration
        // TODO: Implement efficient table iteration for redb
        let db_records = 0i64;
        let has_data = false;
        let db_size_estimate = 0u64;

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
    let mut searcher = state.searcher.lock().await;

    // Emit progress: Starting
    let _ = app_handle.emit("delete-progress", SyncProgress {
        phase: "Starting".to_string(),
        message: "Preparing to delete data...".to_string(),
        percentage: 0.0,
    });

    // Get the version ID
    let version = storage
        .get_latest(&terminology_type)
        .map_err(|e| format!("Failed to get version: {}", e))?;

    if let Some(version) = version {
        // Small delay so user sees "Starting" phase
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        // Emit progress: Deleting
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Deleting".to_string(),
            message: format!("Deleting {} data...", terminology_type),
            percentage: 10.0,
        });

        // Delete records based on terminology type
        let deleted_count = match terminology_type.as_str() {
            "snomed" => {
                storage
                    .delete_snomed_by_version(version.id)
                    .map_err(|e| format!("Failed to delete SNOMED data: {}", e))?
            }
            "amt" => {
                storage
                    .delete_amt_by_version(version.id)
                    .map_err(|e| format!("Failed to delete AMT data: {}", e))?
            }
            "valuesets" => {
                storage
                    .delete_valuesets_by_version(version.id)
                    .map_err(|e| format!("Failed to delete ValueSets data: {}", e))?
            }
            _ => return Err("Unknown terminology type".to_string()),
        };

        // Emit progress: Clearing indexes
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Clearing Indexes".to_string(),
            message: "Clearing search indexes...".to_string(),
            percentage: 70.0,
        });

        // Clear Tantivy indexes
        match terminology_type.as_str() {
            "snomed" => {
                searcher.clear_snomed()
                    .map_err(|e| format!("Failed to clear SNOMED index: {}", e))?;
            }
            "amt" => {
                searcher.clear_amt()
                    .map_err(|e| format!("Failed to clear AMT index: {}", e))?;
            }
            "valuesets" => {
                searcher.clear_valuesets()
                    .map_err(|e| format!("Failed to clear ValueSets index: {}", e))?;
            }
            _ => {}
        }

        // Emit progress: Updating version
        let _ = app_handle.emit("delete-progress", SyncProgress {
            phase: "Updating".to_string(),
            message: "Updating version metadata...".to_string(),
            percentage: 80.0,
        });

        // Update version to mark as not imported
        storage
            .clear_imported_status(version.id)
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

    // Get all ghost version IDs
    let ghost_ids = storage
        .get_ghost_versions()
        .map_err(|e| format!("Failed to get ghost versions: {}", e))?;

    // Clear download metadata for each ghost version
    for version_id in &ghost_ids {
        storage
            .clear_downloaded(*version_id)
            .map_err(|e| format!("Failed to clear ghost version {}: {}", version_id, e))?;
    }

    let cleaned = ghost_ids.len();

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
