use monitor_contracts::*;
use monitor_core::{devices::*, local::opaque_id};

fn metric(value: &str) -> Metric<String> {
    Metric {
        value: Some(value.into()),
        quality: QualityStatus::Partial,
        reason_codes: vec![ReasonCode::CoverageIncomplete],
    }
}
fn event(id: &str, output: &str) -> DeviceEvent {
    let o: i64 = output.parse().unwrap();
    DeviceEvent {
        event_ref: opaque_id("event", id),
        at: "2026-10-01T08:01:00Z".into(),
        scope: Scope {
            provider_id: "codex".into(),
            product_id: "codex-local".into(),
            account_ref: None,
            identity_evidence: IdentityEvidence::Unknown,
        },
        tokens: TokenComponents {
            uncached_input: metric("12"),
            cached_input: metric("8"),
            inclusive_input: metric("20"),
            output: metric(output),
            reasoning_output: metric("0"),
            text_total: metric(&(20 + o).to_string()),
        },
    }
}
fn report(id: &str, events: Vec<DeviceEvent>) -> DeviceReport {
    DeviceReport {
        format: REPORT_FORMAT.into(),
        schema_version: REPORT_VERSION,
        device: DeviceIdentity {
            device_ref: opaque_id("device", id),
            platform: if id == "windows" {
                DevicePlatform::Windows
            } else {
                DevicePlatform::Macos
            },
        },
        generated_at: "2026-10-02T00:00:00Z".into(),
        last_sync_at: Some("2026-10-01T09:00:00Z".into()),
        files_indexed: 1,
        excluded_events: 2,
        reason_codes: vec![ReasonCode::CoverageIncomplete],
        events,
    }
}
#[test]
fn two_hosts_copy_dedup_and_conflicts_are_order_independent() {
    let windows = report("windows", vec![event("one", "6"), event("shared", "9")]);
    let mac = report("mac", vec![event("two", "4"), event("shared", "9")]);
    let merged = merge_reports(&[windows.clone(), mac.clone()]).unwrap();
    assert_eq!(merged.duplicate_events, 1);
    assert_eq!(merged.conflict_events, 0);
    assert_eq!(
        merged
            .daily
            .iter()
            .find(|row| row.device_ref.is_none())
            .unwrap()
            .tokens
            .text_total
            .value
            .as_deref(),
        Some("79")
    );
    assert_eq!(merged.devices[1].device.platform, DevicePlatform::Macos);
    let mut conflicting = mac.clone();
    conflicting.events[1] = event("shared", "10");
    for reports in [
        [windows.clone(), conflicting.clone()],
        [conflicting.clone(), windows.clone()],
    ] {
        let result = merge_reports(&reports).unwrap();
        assert_eq!(result.conflict_events, 1);
        assert_eq!(
            result
                .daily
                .iter()
                .find(|row| row.device_ref.is_none())
                .unwrap()
                .tokens
                .text_total
                .value
                .as_deref(),
            Some("50")
        );
    }
}
#[test]
fn date_scope_missing_cache_and_large_integer_rules_survive_merge() {
    let mut first = event("one", "6");
    first.tokens.cached_input = Metric {
        value: None,
        quality: QualityStatus::Unknown,
        reason_codes: vec![ReasonCode::MissingComponent],
    };
    first.tokens.uncached_input = first.tokens.cached_input.clone();
    let mut child = event("child", "4");
    child.scope.product_id = "codex-subagent".into();
    let mut tomorrow = event("tomorrow", "8");
    tomorrow.at = "2026-10-02T00:00:00Z".into();
    let result = merge_reports(&[report("mac", vec![first, child, tomorrow])]).unwrap();
    assert_eq!(
        result
            .daily
            .iter()
            .filter(|row| row.device_ref.is_none())
            .count(),
        3
    );
    let row = result
        .daily
        .iter()
        .find(|row| {
            row.device_ref.is_none()
                && row.scope.product_id == "codex-local"
                && row.date_label == "2026-10-01"
        })
        .unwrap();
    assert_eq!(row.tokens.text_total.value.as_deref(), Some("26"));
    assert_eq!(row.tokens.cached_input.value, None);
    let mut large = event("large", "0");
    large.tokens.uncached_input = metric("9007199254740993");
    large.tokens.cached_input = metric("0");
    large.tokens.inclusive_input = metric("9007199254740993");
    large.tokens.text_total = metric("9007199254740993");
    let result = merge_reports(&[report("mac", vec![large])]).unwrap();
    assert_eq!(
        result.daily[0].tokens.text_total.value.as_deref(),
        Some("9007199254740993")
    );
}
#[test]
fn invalid_imports_and_overflow_fail_closed() {
    let valid = report("mac", vec![event("one", "6")]);
    let encoded = serde_json::to_vec(&valid).unwrap();
    assert!(decode_report(&encoded).is_ok());
    let mut unsafe_fields = serde_json::to_value(&valid).unwrap();
    unsafe_fields["messages"] = "private body".into();
    assert!(decode_report(&serde_json::to_vec(&unsafe_fields).unwrap()).is_err());
    for changed in [
        {
            let mut r = valid.clone();
            r.schema_version = 2;
            r
        },
        {
            let mut r = valid.clone();
            r.events.push(r.events[0].clone());
            r
        },
        {
            let mut r = valid.clone();
            r.events[0].at = "invalid".into();
            r
        },
        {
            let mut r = valid.clone();
            r.events[0].scope.product_id = "codex-account".into();
            r
        },
        {
            let mut r = valid.clone();
            r.events[0].scope.account_ref = Some("private-email".into());
            r
        },
        {
            let mut r = valid.clone();
            r.events[0].tokens.text_total = metric("0");
            r
        },
    ] {
        assert!(validate_report(&changed).is_err());
    }
    assert!(merge_reports(&[valid.clone(), valid]).is_err());
    let mut large = event("large", "0");
    large.tokens.uncached_input = metric("9223372036854775807");
    large.tokens.cached_input = metric("0");
    large.tokens.inclusive_input = metric("9223372036854775807");
    large.tokens.text_total = metric("9223372036854775807");
    assert!(merge_reports(&[report("mac", vec![large, event("one", "6")])]).is_err());
}
