use monitor_contracts::*;
use monitor_core::local::*;
use std::collections::BTreeSet;

fn records() -> ParserState {
    let mut state = ParserState::default();
    state.project_line(br#"{"type":"session_meta","timestamp":"2026-10-01T08:00:00Z","payload":{"id":"thread-a","creator_account_id":"account-a","cwd":"C:/Synthetic","base_instructions":"PRIVATE_BODY_NEVER_PERSIST"}}"#);
    state.project_line(br#"{"type":"turn_context","timestamp":"2026-10-01T08:00:00Z","payload":{"model":"synthetic-model","effort":"high","service_tier":"fast","cwd":"C:/PRIVATE_PROJECT"}}"#);
    state
}
fn counter(
    state: &mut ParserState,
    at: &str,
    total: (i64, i64, i64),
    last: (i64, i64, i64),
) -> Snapshot {
    let (input, cached, output) = total;
    let (last_input, last_cached, last_output) = last;
    let value = serde_json::json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{
        "total_token_usage":{"input_tokens":input,"cached_input_tokens":cached,"output_tokens":output,"total_tokens":input+output},
        "last_token_usage":{"input_tokens":last_input,"cached_input_tokens":last_cached,"output_tokens":last_output,"total_tokens":last_input+last_output}}}});
    state.project_line(value.to_string().as_bytes()).unwrap()
}
#[test]
fn projection_retains_only_whitelisted_statistics_and_hashes() {
    let mut state = records();
    let snapshot = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let text = serde_json::to_string(&snapshot).unwrap();
    for private in [
        "PRIVATE_BODY",
        "PRIVATE_PROJECT",
        "thread-a",
        "account-a",
        "C:\\Synthetic",
    ] {
        assert!(!text.contains(private));
    }
    assert!(snapshot
        .metadata
        .account_ref
        .unwrap()
        .starts_with("account-"));
    assert_eq!(snapshot.speed, None); // Service tier is not a speed assertion.
    assert!(snapshot.last.reasoning.is_none());
}
#[test]
fn cumulative_delta_cache_once_and_repeated_snapshots() {
    let mut state = records();
    let first = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let second = counter(&mut state, "2026-10-01T08:02:00Z", (30, 12, 9), (10, 4, 3));
    let repeat = counter(&mut state, "2026-10-01T08:03:00Z", (30, 12, 9), (10, 4, 3));
    let events = project_thread(vec![first, second, repeat], &BTreeSet::new(), false);
    assert_eq!(events.len(), 2);
    let tokens = aggregate(&events.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(tokens.inclusive_input.value.as_deref(), Some("30"));
    assert_eq!(tokens.uncached_input.value.as_deref(), Some("18"));
    assert_eq!(tokens.cached_input.value.as_deref(), Some("12"));
    assert_eq!(tokens.text_total.value.as_deref(), Some("39"));
}
#[test]
fn initial_balance_is_baseline_not_new_usage() {
    let mut state = records();
    let first = counter(
        &mut state,
        "2026-10-01T08:01:00Z",
        (100, 50, 10),
        (20, 8, 2),
    );
    let second = counter(
        &mut state,
        "2026-10-01T08:02:00Z",
        (120, 58, 12),
        (20, 8, 2),
    );
    let events = project_thread(vec![first, second], &BTreeSet::new(), false);
    assert!(!events[0].allocated);
    assert!(events[1].allocated);
    assert!(events[0]
        .reason_codes
        .contains(&ReasonCode::InitialCounterBaseline));
    assert_eq!(
        aggregate(&events.iter().collect::<Vec<_>>())
            .unwrap()
            .text_total
            .value
            .as_deref(),
        Some("22")
    );
}
#[test]
fn reset_boundary_is_excluded_but_next_verified_delta_recovers() {
    let mut state = records();
    let first = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let reset = counter(&mut state, "2026-10-01T08:02:00Z", (5, 2, 1), (5, 2, 1));
    let later = counter(&mut state, "2026-10-01T08:03:00Z", (10, 4, 2), (5, 2, 1));
    let events = project_thread(vec![first, reset, later], &BTreeSet::new(), false);
    assert_eq!(events.iter().filter(|event| event.allocated).count(), 2);
    assert!(!events[2].reason_codes.contains(&ReasonCode::CounterReset));
    assert!(events[1].reason_codes.contains(&ReasonCode::CounterReset));
    assert_eq!(
        aggregate(&events.iter().collect::<Vec<_>>())
            .unwrap()
            .text_total
            .value
            .as_deref(),
        Some("32")
    );
}

#[test]
fn revert_segments_do_not_mix_counters_or_invalidate_original_thread() {
    let mut state = records();
    let original = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let continued = counter(&mut state, "2026-10-01T08:04:00Z", (30, 12, 9), (10, 4, 3));
    state.project_line(br#"{"type":"session_meta","timestamp":"2026-10-01T08:01:30Z","payload":{"id":"thread-a","creator_account_id":"account-a","cwd":"C:/Synthetic","history_base":{}}}"#);
    assert!(!state.reason_codes.contains(&ReasonCode::MetadataConflict));
    let baseline = counter(
        &mut state,
        "2026-10-01T08:02:00Z",
        (100, 50, 20),
        (10, 4, 2),
    );
    let verified = counter(
        &mut state,
        "2026-10-01T08:03:00Z",
        (110, 54, 22),
        (10, 4, 2),
    );
    let events = project_thread(
        vec![original, baseline, verified, continued],
        &BTreeSet::new(),
        false,
    );
    assert_eq!(events.iter().filter(|e| e.allocated).count(), 3);
    assert_eq!(
        aggregate(&events.iter().collect::<Vec<_>>())
            .unwrap()
            .text_total
            .value
            .as_deref(),
        Some("51")
    );
    assert_eq!(
        exclusion_reason(&events[1]),
        ReasonCode::InitialCounterBaseline
    );
}

#[test]
fn identity_conflicts_and_shared_responses_still_block_accounting() {
    let mut state = records();
    let mut original = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    original.response_ref = Some(opaque_id("response", "same-request"));
    let mut copied = counter(&mut state, "2026-10-01T08:02:00Z", (20, 8, 6), (20, 8, 6));
    copied.metadata.created_at = Some(utc("2026-10-01T08:01:30Z").unwrap());
    copied.response_ref = original.response_ref.clone();
    let events = project_thread(vec![original.clone(), copied], &BTreeSet::new(), false);
    assert_eq!(events.iter().filter(|e| e.allocated).count(), 1);
    assert!(events[1]
        .reason_codes
        .contains(&ReasonCode::EventOwnershipAmbiguous));
    state.project_line(br#"{"type":"session_meta","timestamp":"2026-10-01T08:03:00Z","payload":{"id":"thread-a","creator_account_id":"another-account","cwd":"C:/Synthetic"}}"#);
    assert!(state.reason_codes.contains(&ReasonCode::MetadataConflict));
    assert!(!project_thread(vec![original], &BTreeSet::new(), true)[0].allocated);
}

#[test]
fn a_long_turn_crossing_midnight_uses_latest_verified_observation() {
    let mut state = records();
    state.project_line(br#"{"type":"turn_context","timestamp":"2026-10-01T23:58:00Z","payload":{"model":"synthetic-model"}}"#);
    let first = counter(&mut state, "2026-10-01T23:59:00Z", (20, 8, 6), (20, 8, 6));
    let crossing = counter(&mut state, "2026-10-02T00:01:00Z", (30, 12, 9), (10, 4, 3));
    let same_day = counter(&mut state, "2026-10-02T00:02:00Z", (40, 16, 12), (10, 4, 3));
    let events = project_thread(vec![first, crossing, same_day], &BTreeSet::new(), false);
    assert!(!events[1].allocated);
    assert!(events[2].allocated);
    assert_eq!(events[2].date, "2026-10-02");
}

#[test]
fn native_posix_workspace_is_not_treated_as_imported_on_mac_or_linux() {
    let mut state = ParserState::default();
    state.project_line(br#"{"type":"session_meta","timestamp":"2026-10-01T08:00:00Z","payload":{"id":"native-posix","cwd":"/Users/synthetic/project"}}"#);
    let snapshot = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    assert_eq!(snapshot.metadata.imported, cfg!(windows));
    assert_eq!(
        project_thread(vec![snapshot], &BTreeSet::new(), false)[0].allocated,
        !cfg!(windows)
    );
}
#[test]
fn gap_is_excluded_then_next_verified_interval_can_resume() {
    let mut state = records();
    let first = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let gap = counter(&mut state, "2026-10-01T08:02:00Z", (60, 20, 12), (10, 4, 2));
    let later = counter(&mut state, "2026-10-01T08:03:00Z", (70, 24, 14), (10, 4, 2));
    let events = project_thread(vec![first, gap, later], &BTreeSet::new(), false);
    assert!(!events[1].allocated);
    assert!(events[2].allocated);
    assert_eq!(
        aggregate(&events.iter().collect::<Vec<_>>())
            .unwrap()
            .text_total
            .value
            .as_deref(),
        Some("38")
    );
}
#[test]
fn cross_midnight_is_unknown_without_a_same_day_lower_bound() {
    let mut state = records();
    let first = counter(&mut state, "2026-10-01T23:59:00Z", (20, 8, 6), (20, 8, 6));
    let crossed = counter(&mut state, "2026-10-02T00:01:00Z", (30, 12, 9), (10, 4, 3));
    state.project_line(br#"{"type":"turn_context","timestamp":"2026-10-02T00:02:00Z","payload":{"model":"synthetic-model"}}"#);
    let same_day = counter(&mut state, "2026-10-02T00:03:00Z", (40, 16, 12), (10, 4, 3));
    let events = project_thread(vec![first, crossed, same_day], &BTreeSet::new(), false);
    assert!(!events[1].allocated);
    assert_eq!(events[1].date, "2026-10-02");
    assert!(events[2].allocated);
    assert!(events[1]
        .reason_codes
        .contains(&ReasonCode::CrossDayInterval));
}
#[test]
fn missing_cache_does_not_erase_independently_known_input_and_total() {
    let mut state = records();
    let mut first = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    first.total.cached = None;
    first.last.cached = None;
    let events = project_thread(vec![first], &BTreeSet::new(), false);
    let tokens = aggregate(&events.iter().collect::<Vec<_>>()).unwrap();
    assert!(tokens.uncached_input.value.is_none() && tokens.cached_input.value.is_none());
    assert_eq!(tokens.text_total.value.as_deref(), Some("26"));
}
#[test]
fn malformed_negative_or_noninteger_counters_are_not_coerced() {
    let mut state = records();
    assert!(state.project_line(br#"{"type":"event_msg","timestamp":"2026-10-01T08:01:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":-1},"last_token_usage":{}}}}"#).is_none());
    assert!(state.reason_codes.contains(&ReasonCode::InvalidToken));
    assert!(state.project_line(b"malformed").is_none());
    assert!(state.reason_codes.contains(&ReasonCode::InvalidRecord));
}
#[test]
fn inherited_replay_before_creation_is_excluded() {
    let mut state = records();
    let mut replay = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    replay.metadata.inherited = true;
    replay.metadata.created_at = Some(utc("2026-10-01T09:00:00Z").unwrap());
    let events = project_thread(vec![replay], &BTreeSet::new(), false);
    assert!(!events[0].allocated);
    assert!(events[0]
        .reason_codes
        .contains(&ReasonCode::InheritedHistory));
}
#[test]
fn metadata_and_cross_thread_ownership_conflicts_are_not_assigned_arbitrarily() {
    let mut state = records();
    let first = counter(&mut state, "2026-10-01T08:01:00Z", (20, 8, 6), (20, 8, 6));
    let keys = BTreeSet::from([first.ownership_key.clone()]);
    assert!(!project_thread(vec![first.clone()], &keys, false)[0].allocated);
    assert!(!project_thread(vec![first], &BTreeSet::new(), true)[0].allocated);
}
