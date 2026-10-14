//! Per-device log reports. Account quota and account history never enter this merge.
use crate::{error, local::utc, parse_count, validate_tokens};
use monitor_contracts::*;
use std::collections::{BTreeMap, BTreeSet};

pub const REPORT_FORMAT: &str = "agent-usage-monitor/device-log";
pub const REPORT_VERSION: u32 = 1;
pub const MAX_REPORT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EVENTS: usize = 50_000;
pub const MAX_DEVICES: usize = 32;

pub fn native_platform() -> DevicePlatform {
    match std::env::consts::OS {
        "windows" => DevicePlatform::Windows,
        "macos" => DevicePlatform::Macos,
        "linux" => DevicePlatform::Linux,
        _ => DevicePlatform::Unknown,
    }
}
fn invalid() -> ApiError {
    error(ErrorCode::InvalidData, "Invalid device log report")
}
pub fn hashed_ref(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
pub fn validate_report(report: &DeviceReport) -> Result<(), ApiError> {
    if report.format != REPORT_FORMAT
        || report.schema_version != REPORT_VERSION
        || !hashed_ref(&report.device.device_ref, "device-")
        || report.events.len() > MAX_EVENTS
        || report.reason_codes.len() > 100
        || utc(&report.generated_at).is_none()
        || report
            .last_sync_at
            .as_ref()
            .is_some_and(|at| utc(at).is_none() || utc(at) > utc(&report.generated_at))
        || (!report.events.is_empty() && report.last_sync_at.is_none())
    {
        return Err(invalid());
    }
    let mut ids = BTreeSet::new();
    for event in &report.events {
        if !hashed_ref(&event.event_ref, "event-")
            || !ids.insert(&event.event_ref)
            || utc(&event.at).is_none()
            || utc(&event.at) > utc(&report.generated_at)
            || event.scope.provider_id != "codex"
            || !matches!(
                event.scope.product_id.as_str(),
                "codex-local" | "codex-subagent"
            )
            || event.scope.identity_evidence != IdentityEvidence::Unknown
            || event
                .scope
                .account_ref
                .as_ref()
                .is_some_and(|id| !hashed_ref(id, "account-"))
            || event.tokens.text_total.value.is_none()
        {
            return Err(invalid());
        }
        validate_tokens(&event.tokens).map_err(|_| invalid())?;
    }
    Ok(())
}
pub fn decode_report(bytes: &[u8]) -> Result<DeviceReport, ApiError> {
    if bytes.len() > MAX_REPORT_BYTES {
        return Err(invalid());
    }
    let report = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    validate_report(&report)?;
    Ok(report)
}
pub fn sum_tokens(rows: &[&TokenComponents]) -> Result<TokenComponents, ApiError> {
    let sum = |pick: fn(&TokenComponents) -> &Metric<String>| -> Result<Metric<String>, ApiError> {
        let values: Vec<_> = rows.iter().map(|row| &pick(row).value).collect();
        if values.is_empty() || values.iter().any(|value| value.is_none()) {
            return Ok(Metric {
                value: None,
                quality: QualityStatus::Unknown,
                reason_codes: vec![if values.is_empty() {
                    ReasonCode::NoData
                } else {
                    ReasonCode::MissingComponent
                }],
            });
        }
        let value = values.iter().try_fold(0i64, |sum, value| {
            sum.checked_add(parse_count(value.as_deref().unwrap())?)
                .ok_or_else(invalid)
        })?;
        Ok(Metric {
            value: Some(value.to_string()),
            quality: QualityStatus::Partial,
            reason_codes: vec![ReasonCode::CoverageIncomplete, ReasonCode::IdentityUnknown],
        })
    };
    let result = TokenComponents {
        uncached_input: sum(|row| &row.uncached_input)?,
        cached_input: sum(|row| &row.cached_input)?,
        inclusive_input: sum(|row| &row.inclusive_input)?,
        output: sum(|row| &row.output)?,
        reasoning_output: sum(|row| &row.reasoning_output)?,
        text_total: sum(|row| &row.text_total)?,
    };
    validate_tokens(&result)?;
    Ok(result)
}
type DayKey = (Option<String>, String, Option<String>, String);
/// Same observation copied between hosts counts once. Conflicting copies are excluded from the union.
/// A device row describes where a log was collected, not a claim about execution ownership.
pub fn merge_reports(reports: &[DeviceReport]) -> Result<DeviceUsage, ApiError> {
    if reports.len() > MAX_DEVICES {
        return Err(invalid());
    }
    let mut devices = Vec::new();
    let mut device_ids = BTreeSet::new();
    let mut union: BTreeMap<&str, &DeviceEvent> = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    let mut duplicate_events = 0u32;
    let mut groups: BTreeMap<DayKey, Vec<&TokenComponents>> = BTreeMap::new();
    for report in reports {
        validate_report(report)?;
        if !device_ids.insert(&report.device.device_ref) {
            return Err(invalid());
        }
        devices.push(DeviceSummary {
            device: report.device.clone(),
            generated_at: report.generated_at.clone(),
            last_sync_at: report.last_sync_at.clone(),
            files_indexed: report.files_indexed,
            records: report.events.len() as u32,
            excluded_events: report.excluded_events,
            reason_codes: report.reason_codes.clone(),
        });
        for event in &report.events {
            groups
                .entry((
                    Some(report.device.device_ref.clone()),
                    event.scope.product_id.clone(),
                    event.scope.account_ref.clone(),
                    utc(&event.at).unwrap()[..10].to_owned(),
                ))
                .or_default()
                .push(&event.tokens);
            if let Some(previous) = union.get(event.event_ref.as_str()) {
                duplicate_events += 1;
                // Quality wording may evolve; accounting equality uses scope, UTC time and metric values.
                let same_counts = [
                    &event.tokens.uncached_input,
                    &event.tokens.cached_input,
                    &event.tokens.inclusive_input,
                    &event.tokens.output,
                    &event.tokens.reasoning_output,
                    &event.tokens.text_total,
                ]
                .iter()
                .zip([
                    &previous.tokens.uncached_input,
                    &previous.tokens.cached_input,
                    &previous.tokens.inclusive_input,
                    &previous.tokens.output,
                    &previous.tokens.reasoning_output,
                    &previous.tokens.text_total,
                ])
                .all(|(a, b)| a.value == b.value);
                if event.scope != previous.scope
                    || utc(&event.at) != utc(&previous.at)
                    || !same_counts
                {
                    conflicts.insert(event.event_ref.as_str());
                }
            } else {
                union.insert(&event.event_ref, event);
            }
        }
    }
    for (id, event) in union {
        if !conflicts.contains(id) {
            groups
                .entry((
                    None,
                    event.scope.product_id.clone(),
                    event.scope.account_ref.clone(),
                    utc(&event.at).unwrap()[..10].to_owned(),
                ))
                .or_default()
                .push(&event.tokens);
        }
    }
    let mut daily = Vec::new();
    for ((device_ref, product_id, account_ref, date_label), rows) in groups {
        daily.push(DeviceDay {
            device_ref,
            scope: Scope {
                provider_id: "codex".into(),
                product_id,
                account_ref,
                identity_evidence: IdentityEvidence::Unknown,
            },
            date_label,
            tokens: sum_tokens(&rows)?,
            events: rows.len() as u32,
        });
    }
    Ok(DeviceUsage {
        devices,
        daily,
        duplicate_events,
        conflict_events: conflicts.len() as u32,
    })
}
