use monitor_adapters::Store;
use monitor_contracts::*;
use std::{path::PathBuf, sync::atomic::AtomicBool};

fn fixture() -> (PathBuf, Store) {
    let root = std::env::temp_dir().join(format!("monitor-reprojection-{}", uuid::Uuid::new_v4()));
    let home = root.join("home");
    std::fs::create_dir_all(home.join("sessions")).unwrap();
    let meta = |time: &str, inherited: bool| {
        serde_json::json!({"type":"session_meta","timestamp":time,"payload":{"id":"same-thread","creator_account_id":"synthetic-account","cwd":"C:/Synthetic","history_base":if inherited { serde_json::json!({}) } else { serde_json::Value::Null }}}).to_string()
    };
    let usage = |time: &str, t: (i64, i64, i64), l: (i64, i64, i64)| {
        serde_json::json!({"type":"event_msg","timestamp":time,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":t.0,"cached_input_tokens":t.1,"output_tokens":t.2,"total_tokens":t.0+t.2},"last_token_usage":{"input_tokens":l.0,"cached_input_tokens":l.1,"output_tokens":l.2,"total_tokens":l.0+l.2}}}}).to_string()
    };
    let first = [
        meta("2026-10-01T08:00:00Z", false),
        usage("2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6)),
        usage("2026-10-01T08:04:00Z", (30, 12, 9), (10, 4, 3)),
    ]
    .join("\n")
        + "\n";
    let reverted = [
        meta("2026-10-01T08:01:30Z", true),
        usage("2026-10-01T08:02:00Z", (100, 50, 20), (10, 4, 2)),
        usage("2026-10-01T08:03:00Z", (110, 54, 22), (10, 4, 2)),
    ]
    .join("\n")
        + "\n";
    std::fs::write(home.join("sessions/original.jsonl"), first).unwrap();
    std::fs::write(home.join("sessions/reverted.jsonl"), reverted).unwrap();
    let mut store = Store::open(&root.join("monitor.sqlite3")).unwrap();
    store
        .synchronize_local(&[home], &AtomicBool::new(false))
        .unwrap();
    (root, store)
}
#[test]
fn segment_reprojection_recovers_cached_history_once_without_rescanning_or_losing_preferences() {
    let (root, store) = fixture();
    let data = store.local_dashboard().unwrap();
    assert_eq!(
        data.local_rows[0].tokens.text_total.value.as_deref(),
        Some("51")
    );
    assert_eq!(data.collector.as_ref().unwrap().ambiguous_events, 1);
    assert_eq!(
        data.collector.as_ref().unwrap().excluded_breakdown[0].reason_code,
        ReasonCode::InitialCounterBaseline
    );
    drop(store);
    let connection = rusqlite::Connection::open(root.join("monitor.sqlite3")).unwrap();
    let offsets: Vec<i64> = connection
        .prepare("SELECT byte_offset FROM local_streams ORDER BY source_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    connection.execute_batch("DELETE FROM app_meta WHERE key='local-projection-version'; UPDATE app_settings SET value='false' WHERE key='account-query-enabled'; UPDATE local_usage_events SET allocated=0,event_json=json_set(event_json,'$.allocated',json('false')); DELETE FROM local_daily WHERE data_mode='observed';").unwrap();
    let reopened = Store::open(&root.join("monitor.sqlite3")).unwrap();
    let projected = reopened.local_dashboard().unwrap();
    assert_eq!(
        projected.local_rows[0].tokens.text_total.value.as_deref(),
        Some("51")
    );
    let summary = projected.collector.unwrap();
    assert_eq!(summary.accepted_events, 3);
    assert_eq!(
        summary
            .excluded_breakdown
            .iter()
            .map(|g| g.events)
            .sum::<u32>(),
        summary.ambiguous_events
    );
    assert!(!reopened.account_summary().unwrap().enabled);
    let after: Vec<i64> = connection
        .prepare("SELECT byte_offset FROM local_streams ORDER BY source_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(offsets, after);
    let collected = projected.local_rows[0].source.collected_at.clone();
    drop(reopened);
    assert_eq!(
        Store::open(&root.join("monitor.sqlite3"))
            .unwrap()
            .local_dashboard()
            .unwrap()
            .local_rows[0]
            .source
            .collected_at,
        collected
    );
}
#[test]
fn failed_reprojection_rolls_back_events_daily_rows_and_version_marker() {
    let (root, store) = fixture();
    drop(store);
    let connection = rusqlite::Connection::open(root.join("monitor.sqlite3")).unwrap();
    connection.execute_batch("DELETE FROM app_meta WHERE key='local-projection-version'; CREATE TRIGGER block_projection BEFORE INSERT ON local_usage_events BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(Store::open(&root.join("monitor.sqlite3")).is_err());
    let rows: i64 = connection
        .query_row("SELECT count(*) FROM local_usage_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 4);
    let total: i64 = connection
        .query_row(
            "SELECT text_total FROM local_daily WHERE data_mode='observed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(total, 51);
    let marker: i64 = connection
        .query_row(
            "SELECT count(*) FROM app_meta WHERE key='local-projection-version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(marker, 0);
}
#[test]
#[cfg(not(windows))]
fn unix_cached_foreign_path_flag_is_repaired_along_with_parser_state() {
    let (root, store) = fixture();
    drop(store);
    let connection = rusqlite::Connection::open(root.join("monitor.sqlite3")).unwrap();
    connection.execute_batch("DELETE FROM app_meta WHERE key='local-projection-version'; UPDATE local_raw_snapshots SET snapshot_json=json_set(snapshot_json,'$.metadata.imported',json('true')); UPDATE local_streams SET state_json=json_set(state_json,'$.metadata.imported',json('true'));").unwrap();
    let mut store = Store::open(&root.join("monitor.sqlite3")).unwrap();
    assert_eq!(
        store.local_dashboard().unwrap().local_rows[0]
            .tokens
            .text_total
            .value
            .as_deref(),
        Some("51")
    );
    let report = store
        .synchronize_local(&[root.join("home")], &AtomicBool::new(false))
        .unwrap();
    assert_eq!(report.bytes_parsed, "0");
    assert_eq!(
        store.local_dashboard().unwrap().local_rows[0]
            .tokens
            .text_total
            .value
            .as_deref(),
        Some("51")
    );
}
