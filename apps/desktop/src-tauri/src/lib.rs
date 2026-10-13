use monitor_adapters::Store;
use monitor_contracts::*;
use monitor_core::error;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;

struct AppState {
    store: Mutex<Result<Store, ApiError>>,
}

#[tauri::command]
fn get_dashboard(
    query: DashboardQuery,
    state: tauri::State<'_, AppState>,
) -> Result<Dashboard, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
    match &*store {
        Ok(store) => store.dashboard(query.scenario),
        Err(error) => Err(error.clone()),
    }
}

fn initialize_store(directory: PathBuf) -> Result<Store, ApiError> {
    std::fs::create_dir_all(&directory).map_err(|_| {
        error(
            ErrorCode::Storage,
            "Application data directory is unavailable",
        )
    })?;
    let mut store = Store::open(&directory.join("monitor.sqlite3"))?;
    store.seed_synthetic()?;
    Ok(store)
}

/// Developer smoke entry point: uses an explicitly provided synthetic data directory only.
pub fn verify_synthetic_store() -> Result<(), ApiError> {
    let directory = std::env::var_os("AGENT_USAGE_MONITOR_DATA_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            error(
                ErrorCode::InvalidQuery,
                "A synthetic validation directory is required",
            )
        })?;
    let store = initialize_store(directory)?;
    let dashboard = store.dashboard(DemoScenario::Ready)?;
    if dashboard.data_mode != DataMode::Synthetic
        || dashboard.account_rows.len() < 8
        || dashboard.local_rows.len() < 7
        || dashboard.runtime.network_enabled
        || dashboard.runtime.local_collection_enabled
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Synthetic application smoke validation failed",
        ));
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // Development validation can keep all generated data within this new project.
            let directory = std::env::var_os("AGENT_USAGE_MONITOR_DATA_DIR")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| app.path().app_data_dir());
            let store = directory
                .map_err(|_| {
                    error(
                        ErrorCode::Storage,
                        "Application data directory is unavailable",
                    )
                })
                .and_then(initialize_store);
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_dashboard])
        .run(tauri::generate_context!())
        .expect("Agent Usage Monitor could not start");
}
