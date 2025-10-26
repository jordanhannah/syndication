mod auth;
mod commands;
mod import;
mod ncts;
mod parsers;
mod queries;
mod storage;

use auth::TokenManager;
use commands::{
    expand_valueset, fetch_all_versions, fetch_latest_version, get_all_local_latest,
    get_local_latest, get_local_versions, get_storage_stats, import_terminology, list_valuesets,
    lookup_code, search_terminology, sync_all_terminologies, sync_terminology, validate_code,
    AppState,
};
use directories::ProjectDirs;
use ncts::NctsClient;
use storage::TerminologyStorage;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok(); // Ignore error if .env doesn't exist

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Get the app data directory
            let project_dirs = ProjectDirs::from("com", "ncts", "syndication")
                .expect("Failed to get project directories");

            let data_dir = project_dirs.data_dir();
            let db_path = data_dir.join("syndication.db");
            let terminology_data_dir = data_dir.join("terminology");

            println!("Database path: {:?}", db_path);
            println!("Data directory: {:?}", terminology_data_dir);

            // Initialize storage
            let storage = tauri::async_runtime::block_on(async {
                TerminologyStorage::new(db_path, terminology_data_dir)
                    .await
                    .expect("Failed to initialize storage")
            });

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
            lookup_code,
            expand_valueset,
            validate_code,
            list_valuesets,
            get_storage_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
