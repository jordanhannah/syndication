mod auth;
mod commands;
mod import;
mod ncts;
mod parsers;
mod queries;
mod search;
mod storage;

fn main() {
    run();
}

use auth::TokenManager;
use commands::{
    cleanup_ghost_versions, debug_amt_codes, diagnose_amt_index, delete_all_terminology_data,
    delete_terminology_data, delete_terminology_file, expand_valueset, fetch_all_versions,
    fetch_latest_version, get_all_local_latest, get_amt_code_type_stats,
    get_detailed_storage_info, get_local_latest, get_local_versions, import_terminology,
    list_valuesets, lookup_code, rebuild_amt_index, search_amt_doctor, search_amt_patient,
    search_terminology, sync_all_terminologies, sync_terminology, test_connection, validate_code,
    AppState,
}; // Note: get_storage_stats temporarily disabled during redb migration
use directories::ProjectDirs;
use import::TerminologyImporter;
use ncts::NctsClient;
use search::TerminologySearch;
use storage::TerminologyStorage;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok(); // Ignore error if .env doesn't exist

    // Clean up any orphaned temporary extraction directories from previous runs
    println!("Performing startup cleanup...");
    if let Err(e) = TerminologyImporter::cleanup_orphaned_temp_dirs() {
        eprintln!("Warning: Failed to cleanup orphaned temp directories: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Get the app data directory
            let project_dirs = ProjectDirs::from("com", "ncts", "syndication")
                .expect("Failed to get project directories");

            let data_dir = project_dirs.data_dir();
            let db_path = data_dir.join("syndication.redb");
            let terminology_data_dir = data_dir.join("terminology");
            let index_dir = data_dir.join("indexes");

            println!("Database path: {:?}", db_path);
            println!("Data directory: {:?}", terminology_data_dir);
            println!("Index directory: {:?}", index_dir);

            // Initialize storage (redb)
            let storage = TerminologyStorage::new(db_path, terminology_data_dir)
                .expect("Failed to initialize storage");

            // Initialize Tantivy search indexes
            let searcher = TerminologySearch::new(&index_dir)
                .expect("Failed to initialize search indexes");

            // Initialize token manager from environment variables
            let token_manager = TokenManager::from_env()
                .expect("Failed to create token manager - ensure NCTS_CLIENT_ID and NCTS_CLIENT_SECRET are set");

            // Initialize NCTS client with authentication
            let ncts_client = NctsClient::new(token_manager)
                .expect("Failed to create NCTS client");

            // Create app state
            let state = AppState {
                ncts_client,
                storage: Arc::new(Mutex::new(storage)),
                searcher: Arc::new(Mutex::new(searcher)),
            };

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fetch_latest_version,
            fetch_all_versions,
            sync_terminology,
            sync_all_terminologies,
            get_local_latest,
            get_local_versions,
            get_all_local_latest,
            import_terminology,
            search_terminology,
            search_amt_patient,
            search_amt_doctor,
            get_amt_code_type_stats,
            debug_amt_codes,
            rebuild_amt_index,
            diagnose_amt_index,
            lookup_code,
            expand_valueset,
            validate_code,
            list_valuesets,
            get_detailed_storage_info,
            delete_terminology_file,
            delete_terminology_data,
            delete_all_terminology_data,
            test_connection,
            cleanup_ghost_versions,
            // get_storage_stats temporarily disabled during redb migration
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
