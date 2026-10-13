use monitor_adapters::Store;
use monitor_contracts::*;
use std::{fs, io::Write, path::PathBuf, sync::atomic::AtomicBool};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    log: PathBuf,
    db: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local-data/collector-tests")
            .join(uuid::Uuid::new_v4().to_string());
        let home = root.join("home");
        let log = home.join("sessions/2026/10/rollout-test.jsonl");
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        fs::create_dir_all(home.join("archived_sessions")).unwrap();
        let db = root.join("monitor.sqlite3");
        Self {
            root,
            home,
            log,
            db,
        }
    }
    fn store(&self) -> Store {
        Store::open(&self.db).unwrap()
    }
    fn write(&self, text: &str) {
        fs::write(&self.log, text).unwrap();
    }
    fn append(&self, text: &str) {
        fs::OpenOptions::new()
            .append(true)
            .open(&self.log)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    fn sync(&self, store: &mut Store) -> SyncReport {
        store
            .synchronize_local(std::slice::from_ref(&self.home), &AtomicBool::new(false))
            .unwrap()
    }
}
fn header(id: &str, account: &str) -> String {
    format!("{{\"type\":\"session_meta\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{{\"id\":\"{id}\",\"creator_account_id\":\"{account}\",\"cwd\":\"C:\\\\PRIVATE_PROJECT\",\"base_instructions\":\"PRIVATE_BODY_NEVER_SAVE\"}}}}\n{{\"type\":\"turn_context\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{{\"model\":\"synthetic-model\",\"effort\":\"high\"}}}}\n")
}
fn usage(at: &str, input: i64, output: i64, last_input: i64, last_output: i64) -> String {
    serde_json::json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":input,"cached_input_tokens":0,"output_tokens":output,"total_tokens":input+output},"last_token_usage":{"input_tokens":last_input,"cached_input_tokens":0,"output_tokens":last_output,"total_tokens":last_input+last_output}}}}).to_string()+"\n"
}
fn first() -> String {
    header("thread-a", "account-a") + &usage("2026-10-01T08:01:00Z", 20, 6, 20, 6)
}
fn total(store: &Store) -> Option<String> {
    store
        .local_dashboard()
        .unwrap()
        .local_rows
        .first()
        .and_then(|row| row.tokens.text_total.value.clone())
}
#[test]
fn append_is_incremental_and_restart_resumes_complete_line_checkpoint() {
    let f = Fixture::new();
    f.write(&first());
    let mut store = f.store();
    let initial = f.sync(&mut store);
    assert_eq!(initial.new_snapshots, 1);
    assert_eq!(total(&store).as_deref(), Some("26"));
    let repeated = f.sync(&mut store);
    assert_eq!(repeated.bytes_parsed, "0");
    assert_eq!(repeated.files_unchanged, 1);
    drop(store);
    f.append(&usage("2026-10-01T08:02:00Z", 30, 9, 10, 3));
    let mut reopened = f.store();
    let added = f.sync(&mut reopened);
    assert_eq!(added.new_snapshots, 1);
    assert!(added.bytes_parsed.parse::<usize>().unwrap() < first().len());
    assert_eq!(total(&reopened).as_deref(), Some("39"));
    assert_eq!(f.sync(&mut reopened).new_snapshots, 0);
}
#[test]
fn half_line_is_not_committed_or_counted_until_newline_arrives() {
    let f = Fixture::new();
    let next = usage("2026-10-01T08:02:00Z", 30, 9, 10, 3);
    f.write(&(first() + &next[..next.len() / 2]));
    let mut store = f.store();
    assert_eq!(f.sync(&mut store).partial_tails, 1);
    assert_eq!(total(&store).as_deref(), Some("26"));
    drop(store);
    f.append(&next[next.len() / 2..]);
    let mut store = f.store();
    assert_eq!(f.sync(&mut store).new_snapshots, 1);
    assert_eq!(total(&store).as_deref(), Some("39"));
}
#[test]
fn copies_and_archive_moves_do_not_double_count() {
    let f = Fixture::new();
    f.write(&first());
    let mut store = f.store();
    f.sync(&mut store);
    let copy = f.home.join("archived_sessions/copy.jsonl");
    fs::copy(&f.log, &copy).unwrap();
    let copied = f.sync(&mut store);
    assert_eq!(copied.duplicate_snapshots, 1);
    assert_eq!(total(&store).as_deref(), Some("26"));
    fs::remove_file(&f.log).unwrap();
    f.sync(&mut store);
    assert_eq!(total(&store).as_deref(), Some("26"));
    assert_eq!(
        store
            .local_dashboard()
            .unwrap()
            .collector
            .unwrap()
            .retained_sources,
        0
    );
}
#[test]
fn deletion_retains_verified_history_with_an_explicit_gap() {
    let f = Fixture::new();
    f.write(&first());
    let mut store = f.store();
    f.sync(&mut store);
    fs::remove_file(&f.log).unwrap();
    f.sync(&mut store);
    let data = store.local_dashboard().unwrap();
    assert_eq!(total(&store).as_deref(), Some("26"));
    assert!(data
        .collector
        .unwrap()
        .reason_codes
        .contains(&ReasonCode::SourceMissing));
}
#[test]
fn truncate_or_same_size_rewrite_rebuilds_source_projection_atomically() {
    let f = Fixture::new();
    f.write(&first());
    let mut store = f.store();
    f.sync(&mut store);
    let replacement =
        header("thread-a", "account-a") + &usage("2026-10-01T08:01:00Z", 10, 3, 10, 3);
    f.write(&replacement);
    assert_eq!(f.sync(&mut store).files_rebuilt, 1);
    assert_eq!(total(&store).as_deref(), Some("13"));
    assert!(store
        .local_dashboard()
        .unwrap()
        .collector
        .unwrap()
        .reason_codes
        .contains(&ReasonCode::SourceRewritten));
}
#[test]
fn cancellation_does_not_advance_the_checkpoint_and_resume_is_idempotent() {
    let f = Fixture::new();
    f.write(&first());
    let mut store = f.store();
    assert_eq!(
        store
            .synchronize_local(std::slice::from_ref(&f.home), &AtomicBool::new(true))
            .unwrap_err()
            .code,
        ErrorCode::Cancelled
    );
    assert!(store.local_dashboard().unwrap().local_rows.is_empty());
    drop(store);
    let mut store = f.store();
    assert_eq!(f.sync(&mut store).new_snapshots, 1);
    assert_eq!(f.sync(&mut store).bytes_parsed, "0");
}
#[test]
fn missing_sources_are_unknown_and_private_bodies_are_not_stored() {
    let f = Fixture::new();
    let mut store = f.store();
    f.sync(&mut store);
    assert!(store.local_dashboard().unwrap().local_rows.is_empty());
    fs::write(f.home.join("auth.json"), "PRIVATE_CREDENTIAL_NEVER_READ").unwrap();
    f.write(&(first()+"{\"type\":\"response_item\",\"payload\":{\"content\":\"PRIVATE_BODY_NEVER_SAVE\"}}\n"));
    f.sync(&mut store);
    let report = serde_json::to_string(&store.local_dashboard().unwrap()).unwrap();
    let db = fs::read(&f.db).unwrap();
    let text = String::from_utf8_lossy(&db);
    for private in [
        "PRIVATE_BODY_NEVER_SAVE",
        "PRIVATE_CREDENTIAL_NEVER_READ",
        "PRIVATE_PROJECT",
        "account-a",
        "thread-a",
    ] {
        assert!(!report.contains(private));
        assert!(!text.contains(private));
    }
    assert!(!report.contains(f.root.to_string_lossy().as_ref()));
}
#[test]
fn accounts_and_child_scopes_are_kept_separate() {
    let f = Fixture::new();
    f.write(&first());
    let other = f.home.join("sessions/other.jsonl");
    fs::write(
        &other,
        header("thread-b", "account-b") + &usage("2026-10-01T08:02:00Z", 7, 2, 7, 2),
    )
    .unwrap();
    let child = f.home.join("sessions/child.jsonl");
    let child_header = header("thread-child", "account-a")
        .replace("\"cwd\":", "\"parent_thread_id\":\"thread-a\",\"cwd\":");
    fs::write(
        child,
        child_header + &usage("2026-10-01T08:03:00Z", 8, 3, 8, 3),
    )
    .unwrap();
    let mut store = f.store();
    f.sync(&mut store);
    let data = store.local_dashboard().unwrap();
    assert_eq!(data.local_rows.len(), 3);
    assert!(data
        .local_rows
        .iter()
        .any(|row| row.scope.product_id == "codex-subagent"));
    assert!(data
        .local_rows
        .iter()
        .all(|row| row.scope.identity_evidence == IdentityEvidence::Unknown));
    assert!(data.account_rows.is_empty() && data.quota.is_none() && data.comparisons.is_empty());
}
#[test]
fn root_configuration_accepts_directories_and_never_files() {
    let f = Fixture::new();
    f.write(&first());
    let store = f.store();
    assert!(store
        .set_additional_roots(vec![f.log.to_string_lossy().into_owned()])
        .is_err());
    store
        .set_additional_roots(vec![f.home.to_string_lossy().into_owned()])
        .unwrap();
    assert_eq!(store.additional_roots().unwrap().len(), 1);
}
#[cfg(unix)]
#[test]
fn symlinked_logs_cannot_escape_the_session_root() {
    let f = Fixture::new();
    fs::write(f.root.join("outside.jsonl"), first()).unwrap();
    std::os::unix::fs::symlink(f.root.join("outside.jsonl"), &f.log).unwrap();
    let mut store = f.store();
    let report = f.sync(&mut store);
    assert_eq!(report.files_found, 0);
    assert!(report
        .reason_codes
        .contains(&ReasonCode::DiscoveryIncomplete));
    assert!(store.local_dashboard().unwrap().local_rows.is_empty());
}
