use monitor_adapters::server::{Credentials, HttpsTransport};
use monitor_adapters::{collector::default_homes, Store};
use monitor_contracts::*;
use monitor_core::error;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::Manager;

struct AppState {
    store: Mutex<Result<Store, ApiError>>,
    syncing: AtomicBool,
    cancel: AtomicBool,
    account_syncing: AtomicBool,
    account_cancel: AtomicBool,
}

#[tauri::command]
fn get_dashboard(
    query: DashboardQuery,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Dashboard, ApiError> {
    let mut guard = state
        .store
        .lock()
        .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
    let store = guard.as_mut().map_err(|error| error.clone())?;
    if query.scenario != DemoScenario::Local {
        store.seed_synthetic()?;
    }
    store.dashboard(query.scenario)
}
#[tauri::command]
async fn sync_local(state: tauri::State<'_, Arc<AppState>>) -> Result<SyncReport, ApiError> {
    if state
        .syncing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(error(
            ErrorCode::SyncBusy,
            "Local synchronization already running",
        ));
    }
    state.cancel.store(false, Ordering::Release);
    let worker = state.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut guard = worker
            .store
            .lock()
            .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
        let store = guard.as_mut().map_err(|error| error.clone())?;
        let mut roots = default_homes();
        roots.extend(store.additional_roots()?);
        store.synchronize_local(&roots, &worker.cancel)
    })
    .await
    .map_err(|_| {
        error(
            ErrorCode::Internal,
            "Local synchronization could not finish",
        )
    });
    state.syncing.store(false, Ordering::Release);
    result?
}
#[tauri::command]
fn cancel_local_sync(state: tauri::State<'_, Arc<AppState>>) {
    state.cancel.store(true, Ordering::Release);
}
#[tauri::command]
fn set_local_roots(
    request: LocalRootsRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), ApiError> {
    if state.syncing.load(Ordering::Acquire) {
        return Err(error(
            ErrorCode::SyncBusy,
            "Wait for synchronization to finish",
        ));
    }
    let guard = state
        .store
        .lock()
        .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
    guard
        .as_ref()
        .map_err(|error| error.clone())?
        .set_additional_roots(request.roots)
}

#[tauri::command]
fn set_account_queries(
    request: AccountQueriesRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), ApiError> {
    let mut guard = state
        .store
        .lock()
        .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
    guard
        .as_mut()
        .map_err(|error| error.clone())?
        .set_account_queries(request.enabled)?;
    if !request.enabled {
        state.account_cancel.store(true, Ordering::Release);
    }
    Ok(())
}

#[tauri::command]
async fn sync_account(
    request: AccountSyncRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<AccountSummary, ApiError> {
    if state
        .account_syncing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(error(
            ErrorCode::SyncBusy,
            "Account synchronization already running",
        ));
    }
    let worker = state.inner().clone();
    state.account_cancel.store(false, Ordering::Release);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let now = chrono::Utc::now();
        {
            let guard = worker
                .store
                .lock()
                .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
            guard
                .as_ref()
                .map_err(|error| error.clone())?
                .reserve_account_sync(&request, now)?;
        }
        let home = default_homes()
            .into_iter()
            .next()
            .ok_or_else(|| error(ErrorCode::AuthRequired, "Official Codex login unavailable"))?;
        let credentials = match Credentials::read(&home) {
            Ok(credentials) => credentials,
            Err(failure) => {
                let guard = worker
                    .store
                    .lock()
                    .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
                guard
                    .as_ref()
                    .map_err(|error| error.clone())?
                    .account_login_failed()?;
                return Err(failure);
            }
        };
        let summary = {
            let mut guard = worker
                .store
                .lock()
                .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
            guard
                .as_mut()
                .map_err(|error| error.clone())?
                .bind_account(&credentials.account_ref())?
        };
        let transport = HttpsTransport::new();
        // Construction failures take the same safe persistence path as offline requests.
        struct OfflineTransport;
        impl monitor_adapters::server::Transport for OfflineTransport {
            fn get(
                &self,
                _: monitor_adapters::server::Endpoint,
                _: &AccountSyncRequest,
                _: &Credentials,
            ) -> Result<monitor_adapters::server::HttpReply, ApiError> {
                Err(error(ErrorCode::Offline, "Account connection unavailable"))
            }
        }
        let fetched = match transport {
            Ok(transport) => {
                let guarded = monitor_adapters::server::CancellableTransport {
                    inner: &transport,
                    cancel: &worker.account_cancel,
                };
                monitor_adapters::server::fetch(&guarded, &credentials, &request, summary, now)
            }
            Err(_) => monitor_adapters::server::fetch(
                &OfflineTransport,
                &credentials,
                &request,
                summary,
                now,
            ),
        };
        let mut guard = worker
            .store
            .lock()
            .map_err(|_| error(ErrorCode::Storage, "Database is unavailable"))?;
        let store = guard.as_mut().map_err(|error| error.clone())?;
        store.commit_account_fetch(&fetched)?;
        store.account_summary()
    })
    .await
    .map_err(|_| {
        error(
            ErrorCode::Internal,
            "Account synchronization could not finish",
        )
    });
    state.account_syncing.store(false, Ordering::Release);
    result?
}

fn initialize_store(directory: PathBuf) -> Result<Store, ApiError> {
    std::fs::create_dir_all(&directory).map_err(|_| {
        error(
            ErrorCode::Storage,
            "Application data directory is unavailable",
        )
    })?;
    Store::open(&directory.join("monitor.sqlite3"))
}
fn validation_directory() -> Result<PathBuf, ApiError> {
    std::env::var_os("AGENT_USAGE_MONITOR_DATA_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            error(
                ErrorCode::InvalidQuery,
                "An explicit validation directory is required",
            )
        })
}
pub fn verify_synthetic_store() -> Result<(), ApiError> {
    let mut store = initialize_store(validation_directory()?.join("synthetic-preview-check"))?;
    store.seed_synthetic()?;
    let data = store.dashboard(DemoScenario::Ready)?;
    if data.data_mode != DataMode::Synthetic
        || data.account_rows.len() < 8
        || data.runtime.network_enabled
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Synthetic store validation failed",
        ));
    }
    Ok(())
}

/// Explicit CLI read-only local ingestion. Report contains the same whitelisted DTO as IPC.
pub fn collect_local_report() -> Result<(), ApiError> {
    let directory = validation_directory()?;
    let mut store = initialize_store(directory.clone())?;
    let mut roots = default_homes();
    roots.extend(store.additional_roots()?);
    store.synchronize_local(&roots, &AtomicBool::new(false))?;
    let dashboard = store.dashboard(DemoScenario::Local)?;
    let bytes = serde_json::to_vec_pretty(&dashboard)
        .map_err(|_| error(ErrorCode::Internal, "Could not encode local report"))?;
    std::fs::write(directory.join("local-dashboard.json"), bytes)
        .map_err(|_| error(ErrorCode::Storage, "Could not write local report"))
}

/// A separate fixture database verifies append, half-line recovery, restart and archive dedup.
pub fn verify_local_collector() -> Result<(), ApiError> {
    let directory = validation_directory()?.join(format!(
        "local-fixture-check-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| error(ErrorCode::Internal, "Invalid validation clock"))?
            .as_nanos()
    ));
    let home = directory.join("fixture-home");
    let sessions = home.join("sessions");
    let archive = home.join("archived_sessions");
    std::fs::create_dir_all(&sessions)
        .map_err(|_| error(ErrorCode::Storage, "Fixture directory unavailable"))?;
    std::fs::create_dir_all(&archive)
        .map_err(|_| error(ErrorCode::Storage, "Fixture directory unavailable"))?;
    let source = sessions.join("rollout-fixture.jsonl");
    let first=concat!(
        "{\"type\":\"session_meta\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{\"id\":\"fixture-thread\",\"creator_account_id\":\"fixture-account\",\"cwd\":\"C:\\\\Synthetic\"}}\n",
        "{\"type\":\"turn_context\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{\"model\":\"fixture-model\",\"effort\":\"high\"}}\n",
        "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-01T08:01:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"reasoning_output_tokens\":2,\"total_tokens\":26},\"last_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"reasoning_output_tokens\":2,\"total_tokens\":26}}}}\n"
    );
    let second="{\"type\":\"event_msg\",\"timestamp\":\"2026-10-01T08:02:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":30,\"cached_input_tokens\":12,\"output_tokens\":9,\"reasoning_output_tokens\":3,\"total_tokens\":39},\"last_token_usage\":{\"input_tokens\":10,\"cached_input_tokens\":4,\"output_tokens\":3,\"reasoning_output_tokens\":1,\"total_tokens\":13}}}}\n";
    std::fs::write(&source, format!("{first}{}", &second[..second.len() / 2]))
        .map_err(|_| error(ErrorCode::Storage, "Fixture write failed"))?;
    let mut store = initialize_store(directory.join("database"))?;
    let roots = vec![home];
    let cancel = AtomicBool::new(false);
    store.synchronize_local(&roots, &cancel)?;
    let repeated = store.synchronize_local(&roots, &cancel)?;
    if repeated.bytes_parsed != "0" {
        return Err(error(
            ErrorCode::InvalidData,
            "Unchanged files were reparsed",
        ));
    }
    drop(store);
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&source)
        .map_err(|_| error(ErrorCode::Storage, "Fixture append failed"))?;
    file.write_all(&second.as_bytes()[second.len() / 2..])
        .map_err(|_| error(ErrorCode::Storage, "Fixture append failed"))?;
    drop(file);
    let mut store = initialize_store(directory.join("database"))?;
    let resumed = store.synchronize_local(&roots, &cancel)?;
    std::fs::copy(&source, archive.join("rollout-fixture-copy.jsonl"))
        .map_err(|_| error(ErrorCode::Storage, "Fixture archive failed"))?;
    store.synchronize_local(&roots, &cancel)?;
    let dashboard = store.local_dashboard()?;
    if resumed.new_snapshots != 1
        || dashboard.local_rows.len() != 1
        || dashboard.local_rows[0].tokens.text_total.value.as_deref() != Some("39")
        || dashboard.runtime.network_enabled
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Incremental collector smoke failed",
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
            app.manage(Arc::new(AppState {
                store: Mutex::new(store),
                syncing: AtomicBool::new(false),
                cancel: AtomicBool::new(false),
                account_syncing: AtomicBool::new(false),
                account_cancel: AtomicBool::new(false),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            sync_local,
            cancel_local_sync,
            set_local_roots,
            set_account_queries,
            sync_account
        ])
        .build(tauri::generate_context!())
        .expect("Agent Usage Monitor could not start")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                app.state::<Arc<AppState>>()
                    .cancel
                    .store(true, Ordering::Release);
                app.state::<Arc<AppState>>()
                    .account_cancel
                    .store(true, Ordering::Release);
            }
        });
}
