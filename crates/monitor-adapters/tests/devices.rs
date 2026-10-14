use monitor_adapters::Store;
use monitor_contracts::*;
use monitor_core::{devices::*, local::opaque_id};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("monitor-devices-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn remote() -> DeviceReport {
    DeviceReport {
        format: REPORT_FORMAT.into(),
        schema_version: REPORT_VERSION,
        device: DeviceIdentity {
            device_ref: opaque_id("device", "synthetic-mac"),
            platform: DevicePlatform::Macos,
        },
        generated_at: "2026-10-01T10:00:00Z".into(),
        last_sync_at: Some("2026-10-01T09:00:00Z".into()),
        files_indexed: 0,
        excluded_events: 3,
        reason_codes: vec![ReasonCode::CoverageIncomplete],
        events: vec![],
    }
}
#[test]
fn imports_update_one_device_reopen_and_reject_bad_replacement_transactionally() {
    let root = root();
    let path = root.join("monitor.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let report = remote();
    let bytes = serde_json::to_vec(&report).unwrap();
    store.import_device_report(&bytes).unwrap();
    store.import_device_report(&bytes).unwrap();
    assert_eq!(store.device_usage().unwrap().devices.len(), 2);
    drop(store);
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.device_usage().unwrap().devices[1].excluded_events, 3);
    let mut older = report.clone();
    older.generated_at = "2026-10-01T09:30:00Z".into();
    assert!(store
        .import_device_report(&serde_json::to_vec(&older).unwrap())
        .is_err());
    let mut newer = report.clone();
    newer.generated_at = "2026-10-01T11:00:00Z".into();
    newer.excluded_events = 4;
    store
        .import_device_report(&serde_json::to_vec(&newer).unwrap())
        .unwrap();
    assert_eq!(store.device_usage().unwrap().devices.len(), 2);
    assert_eq!(store.device_usage().unwrap().devices[1].excluded_events, 4);
    let mut malformed = serde_json::to_value(&newer).unwrap();
    malformed["credential"] = "not allowed".into();
    assert!(store
        .import_device_report(&serde_json::to_vec(&malformed).unwrap())
        .is_err());
    let own = store.export_device_report().unwrap();
    assert!(store
        .import_device_report(&serde_json::to_vec(&own).unwrap())
        .is_err());
    assert_eq!(store.device_usage().unwrap().devices.len(), 2);
    store
        .remove_device_report(&report.device.device_ref)
        .unwrap();
    assert_eq!(store.device_usage().unwrap().devices.len(), 1);
}
#[test]
fn real_collector_projection_exports_private_whitelist_and_copy_merge_once() {
    let root = root();
    let home = root.join("home");
    std::fs::create_dir_all(home.join("sessions")).unwrap();
    let logs = concat!(
        "{\"type\":\"session_meta\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{\"id\":\"thread\",\"creator_account_id\":\"private-account\",\"cwd\":\"/private/project\"}}\n",
        "{\"type\":\"user_msg\",\"payload\":{\"text\":\"private conversation\"}}\n",
        "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-01T08:01:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"total_tokens\":26},\"last_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"total_tokens\":26}}}}\n");
    std::fs::write(home.join("sessions/log.jsonl"), logs).unwrap();
    let mut a = Store::open(&root.join("a.sqlite3")).unwrap();
    let mut b = Store::open(&root.join("b.sqlite3")).unwrap();
    a.synchronize_local(std::slice::from_ref(&home), &AtomicBool::new(false))
        .unwrap();
    b.synchronize_local(&[home], &AtomicBool::new(false))
        .unwrap();
    let mut other = b.export_device_report().unwrap();
    other.device.platform = DevicePlatform::Macos;
    let bytes = serde_json::to_vec(&other).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    for private in [
        "private-account",
        "/private/project",
        "private conversation",
        "auth.json",
    ] {
        assert!(!text.contains(private));
    }
    assert_eq!(
        other.events[0].tokens.text_total.value.as_deref(),
        Some("26")
    );
    a.import_device_report(&bytes).unwrap();
    let usage = a.device_usage().unwrap();
    assert_eq!(usage.duplicate_events, 1);
    assert_eq!(
        usage
            .daily
            .iter()
            .find(|row| row.device_ref.is_none())
            .unwrap()
            .tokens
            .text_total
            .value
            .as_deref(),
        Some("26")
    );
    assert!(a
        .dashboard(DemoScenario::Local)
        .unwrap()
        .comparisons
        .is_empty());
    // Emulate cancellation after a committed file, before finish_sync published a successful run.
    let connection = rusqlite::Connection::open(root.join("a.sqlite3")).unwrap();
    connection
        .execute("DELETE FROM app_meta WHERE key='local-last-sync'", [])
        .unwrap();
    connection
        .execute(
            "UPDATE app_meta SET value='\"cancelled\"' WHERE key='local-sync-state'",
            [],
        )
        .unwrap();
    assert!(a.export_device_report().unwrap().last_sync_at.is_some());
    assert!(a
        .dashboard(DemoScenario::Local)
        .unwrap()
        .device_usage
        .is_some());
}
