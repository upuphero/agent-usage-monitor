//! Whitelisted log projection and conservative counter accounting. No filesystem or network IO.
use crate::{error, validate_tokens};
use chrono::{DateTime, Utc};
use monitor_contracts::*;
use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PARSER_VERSION: &str = "codex-jsonl/1";
pub fn opaque_id(kind: &str, value: &str) -> String {
    format!(
        "{kind}-{:x}",
        Sha256::digest(format!("{kind}\0{value}").as_bytes())
    )
}
pub fn utc(value: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(value).ok().map(|date| {
        date.with_timezone(&Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    })
}
fn label(value: Option<&str>) -> Option<String> {
    value
        .filter(|text| {
            !text.is_empty()
                && text.len() <= 100
                && text
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._:+-".contains(&c))
        })
        .map(str::to_owned)
}
fn id_value(value: Option<&Value>, kind: &str) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.is_empty() && text.len() <= 1024 => {
            Some(opaque_id(kind, text))
        }
        Value::Number(number) => Some(opaque_id(kind, &number.to_string())),
        _ => None,
    })
}
pub fn add_reason(reasons: &mut Vec<ReasonCode>, reason: ReasonCode) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Counters {
    pub input: Option<i64>,
    pub cached: Option<i64>,
    pub output: Option<i64>,
    pub reasoning: Option<i64>,
}
impl Counters {
    fn valid(&self) -> bool {
        [self.input, self.cached, self.output, self.reasoning]
            .into_iter()
            .flatten()
            .all(|n| n >= 0)
            && !matches!((self.cached, self.input), (Some(c), Some(i)) if c > i)
            && !matches!((self.reasoning, self.output), (Some(r), Some(o)) if r > o)
            && !matches!((self.input, self.output), (Some(i), Some(o)) if i.checked_add(o).is_none())
    }
    fn delta(&self, previous: &Self) -> Self {
        let subtract = |a: Option<i64>, b: Option<i64>| a.zip(b).map(|(a, b)| a - b);
        Self {
            input: subtract(self.input, previous.input),
            cached: subtract(self.cached, previous.cached),
            output: subtract(self.output, previous.output),
            reasoning: subtract(self.reasoning, previous.reasoning),
        }
    }
    fn regressed(&self, previous: &Self) -> bool {
        [
            (self.input, previous.input),
            (self.cached, previous.cached),
            (self.output, previous.output),
            (self.reasoning, previous.reasoning),
        ]
        .iter()
        .any(|(a, b)| matches!((a,b),(Some(a),Some(b)) if a<b))
    }
    fn agrees(&self, last: &Self) -> bool {
        self.input.is_some()
            && self.output.is_some()
            && self.valid()
            && self.input == last.input
            && self.output == last.output
            && !matches!((self.cached,last.cached),(Some(a),Some(b)) if a!=b)
            && !matches!((self.reasoning,last.reasoning),(Some(a),Some(b)) if a!=b)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub thread_ref: String,
    pub account_ref: Option<String>,
    pub parent_ref: Option<String>,
    pub created_at: Option<String>,
    pub inherited: bool,
    pub child: bool,
    pub imported: bool,
}
impl Metadata {
    pub fn product(&self) -> &'static str {
        if self.child {
            "codex-subagent"
        } else {
            "codex-local"
        }
    }
    pub fn reasons(&self) -> Vec<ReasonCode> {
        let mut reasons = vec![
            ReasonCode::IdentityUnknown,
            ReasonCode::CoverageIncomplete,
            ReasonCode::ProductScopeUnverified,
        ];
        if self.inherited {
            reasons.push(ReasonCode::InheritedHistory);
        }
        if self.child {
            reasons.push(ReasonCode::ChildScopeUnverified);
        }
        if self.imported {
            reasons.push(ReasonCode::ImportedSource);
        }
        reasons
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub key: String,
    pub ownership_key: String,
    pub metadata: Metadata,
    pub at: String,
    pub turn_start: Option<String>,
    pub total: Counters,
    pub last: Counters,
    pub response_ref: Option<String>,
    pub model_id: Option<String>,
    pub effort: Option<String>,
    pub speed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ParserState {
    pub metadata: Option<Metadata>,
    pub turn_start: Option<String>,
    pub model_id: Option<String>,
    pub effort: Option<String>,
    pub speed: Option<String>,
    pub reason_codes: Vec<ReasonCode>,
    pub invalid_records: u32,
}

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    timestamp: Option<&'a str>,
    #[serde(borrow)]
    payload: &'a RawValue,
}
#[derive(Deserialize)]
struct RawMetadata {
    id: Option<String>,
    creator_account_id: Option<Value>,
    parent_thread_id: Option<Value>,
    timestamp: Option<String>,
    cwd: Option<String>,
    source: Option<Value>,
    thread_source: Option<Value>,
    forked_from_id: Option<Value>,
    history_base: Option<Value>,
    subagent_history_start_ordinal: Option<i64>,
}
#[derive(Deserialize)]
struct Context {
    model: Option<String>,
    effort: Option<String>,
    reasoning_effort: Option<String>,
    speed: Option<String>,
    collaboration_mode: Option<Value>,
}
#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    reasoning_output_tokens: Option<i64>,
    total_tokens: Option<i64>,
}
impl Usage {
    fn normalized(self) -> Option<Counters> {
        let counters = Counters {
            input: self.input_tokens,
            cached: self.cached_input_tokens,
            output: self.output_tokens,
            reasoning: self.reasoning_output_tokens,
        };
        if !counters.valid()
            || self.total_tokens.is_some_and(|total| {
                total < 0
                    || counters
                        .input
                        .zip(counters.output)
                        .is_some_and(|(i, o)| i.checked_add(o) != Some(total))
            })
        {
            None
        } else {
            Some(counters)
        }
    }
}
#[derive(Deserialize)]
struct UsageInfo {
    total_token_usage: Option<Usage>,
    last_token_usage: Option<Usage>,
    response_id: Option<Value>,
}
#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    info: Option<UsageInfo>,
    response_id: Option<Value>,
}

impl ParserState {
    /// Only metadata, context and token-count fields become owned objects. Message/tool bodies are skipped.
    pub fn project_line(&mut self, line: &[u8]) -> Option<Snapshot> {
        let result = self.project(line);
        match result {
            Ok(snapshot) => snapshot,
            Err(reason) => {
                add_reason(&mut self.reason_codes, reason);
                self.invalid_records = self.invalid_records.saturating_add(1);
                None
            }
        }
    }
    fn project(&mut self, line: &[u8]) -> Result<Option<Snapshot>, ReasonCode> {
        let envelope: Envelope<'_> =
            serde_json::from_slice(line).map_err(|_| ReasonCode::InvalidRecord)?;
        let at = envelope.timestamp.and_then(utc);
        match envelope.kind {
            "session_meta" => {
                let raw: RawMetadata = serde_json::from_str(envelope.payload.get())
                    .map_err(|_| ReasonCode::InvalidRecord)?;
                let thread_ref = raw
                    .id
                    .as_deref()
                    .filter(|id| !id.is_empty() && id.len() <= 1024)
                    .map(|id| opaque_id("thread", id))
                    .ok_or(ReasonCode::ThreadIdentityMissing)?;
                let nested_parent = raw
                    .source
                    .as_ref()
                    .and_then(|v| v.pointer("/subagent/thread_spawn/parent_thread_id"))
                    .or_else(|| {
                        raw.thread_source
                            .as_ref()
                            .and_then(|v| v.pointer("/subagent/thread_spawn/parent_thread_id"))
                    });
                let parent_ref =
                    id_value(raw.parent_thread_id.as_ref().or(nested_parent), "thread");
                let child = parent_ref.is_some()
                    || raw
                        .source
                        .as_ref()
                        .is_some_and(|v| v.get("subagent").is_some());
                let metadata = Metadata {
                    thread_ref,
                    account_ref: id_value(raw.creator_account_id.as_ref(), "account"),
                    parent_ref,
                    created_at: raw.timestamp.as_deref().and_then(utc).or(at),
                    inherited: raw.forked_from_id.is_some_and(|v| !v.is_null())
                        || raw.history_base.is_some_and(|v| !v.is_null())
                        || raw.subagent_history_start_ordinal.is_some_and(|n| n > 0),
                    child,
                    imported: raw.cwd.as_deref().is_some_and(|cwd| {
                        cwd.starts_with("/Users/")
                            || cwd.starts_with("/home/")
                            || cwd.starts_with("/mnt/")
                    }),
                };
                if self.metadata.as_ref().is_some_and(|old| old != &metadata) {
                    add_reason(&mut self.reason_codes, ReasonCode::MetadataConflict);
                }
                self.metadata = Some(metadata);
                Ok(None)
            }
            "turn_context" => {
                let context: Context = serde_json::from_str(envelope.payload.get())
                    .map_err(|_| ReasonCode::InvalidRecord)?;
                self.model_id = label(context.model.as_deref().or_else(|| {
                    context
                        .collaboration_mode
                        .as_ref()
                        .and_then(|v| v.pointer("/settings/model"))
                        .and_then(Value::as_str)
                }));
                self.effort = label(
                    context
                        .effort
                        .as_deref()
                        .or(context.reasoning_effort.as_deref())
                        .or_else(|| {
                            context
                                .collaboration_mode
                                .as_ref()
                                .and_then(|v| v.pointer("/settings/reasoning_effort"))
                                .and_then(Value::as_str)
                        }),
                );
                self.speed = context
                    .speed
                    .filter(|speed| matches!(speed.as_str(), "fast" | "standard"));
                self.turn_start = at;
                Ok(None)
            }
            "event_msg" => {
                let event: Event = serde_json::from_str(envelope.payload.get())
                    .map_err(|_| ReasonCode::InvalidRecord)?;
                if matches!(event.kind.as_str(), "task_started" | "turn_started") {
                    self.turn_start = at;
                    return Ok(None);
                }
                if event.kind != "token_count" {
                    return Ok(None);
                }
                let Some(info) = event.info else {
                    return Ok(None);
                }; // Quota-only observations have no token usage.
                let metadata = self
                    .metadata
                    .clone()
                    .ok_or(ReasonCode::ThreadIdentityMissing)?;
                let at = at.ok_or(ReasonCode::TimeBasisUnknown)?;
                let total = info
                    .total_token_usage
                    .and_then(Usage::normalized)
                    .ok_or(ReasonCode::InvalidToken)?;
                let last = info
                    .last_token_usage
                    .and_then(Usage::normalized)
                    .ok_or(ReasonCode::InvalidToken)?;
                let response_ref = id_value(
                    event.response_id.as_ref().or(info.response_id.as_ref()),
                    "response",
                );
                let ownership_key = opaque_id(
                    "observation",
                    &serde_json::to_string(&(&at, &total, &last, &response_ref))
                        .map_err(|_| ReasonCode::InvalidRecord)?,
                );
                let key = opaque_id(
                    "event",
                    &format!("{}\0{}", metadata.thread_ref, ownership_key),
                );
                Ok(Some(Snapshot {
                    key,
                    ownership_key,
                    metadata,
                    at,
                    turn_start: self.turn_start.clone(),
                    total,
                    last,
                    response_ref,
                    model_id: self.model_id.clone(),
                    effort: self.effort.clone(),
                    speed: self.speed.clone(),
                }))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalEvent {
    pub key: String,
    pub metadata: Metadata,
    pub at: String,
    pub date: String,
    pub allocated: bool,
    pub model_id: Option<String>,
    pub effort: Option<String>,
    pub speed: Option<String>,
    pub counts: Counters,
    pub reason_codes: Vec<ReasonCode>,
}

/// Cumulative and per-response values must agree. Reset epochs are diagnostic only.
pub fn project_thread(
    mut snapshots: Vec<Snapshot>,
    ownership_conflicts: &BTreeSet<String>,
    metadata_conflict: bool,
) -> Vec<LocalEvent> {
    snapshots.sort_by(|a, b| {
        a.at.cmp(&b.at)
            .then(a.total.input.cmp(&b.total.input))
            .then(a.total.output.cmp(&b.total.output))
            .then(a.key.cmp(&b.key))
    });
    let mut previous: Option<Snapshot> = None;
    let mut reset_epoch = false;
    let mut responses: BTreeMap<String, String> = BTreeMap::new();
    let mut result = Vec::new();
    for snapshot in snapshots {
        if previous.as_ref().is_some_and(|old| {
            snapshot.total == old.total
                && snapshot.last == old.last
                && (snapshot.response_ref.is_none() || snapshot.response_ref == old.response_ref)
        }) {
            previous = Some(snapshot);
            continue;
        }
        let mut reasons = snapshot.metadata.reasons();
        let mut allocated = true;
        let mut counts = snapshot.total.clone();
        if metadata_conflict {
            add_reason(&mut reasons, ReasonCode::MetadataConflict);
            allocated = false;
        }
        if snapshot.metadata.imported {
            allocated = false;
        }
        if ownership_conflicts.contains(&snapshot.ownership_key) {
            add_reason(&mut reasons, ReasonCode::EventOwnershipAmbiguous);
            allocated = false;
        }
        if let Some(response) = &snapshot.response_ref {
            if responses
                .insert(response.clone(), snapshot.key.clone())
                .is_some()
            {
                add_reason(&mut reasons, ReasonCode::EventOwnershipAmbiguous);
                allocated = false;
            }
        }
        if let Some(old) = &previous {
            if snapshot.total == old.total {
                if snapshot.last != old.last
                    || (snapshot.response_ref.is_some()
                        && snapshot.response_ref != old.response_ref)
                {
                    add_reason(&mut reasons, ReasonCode::RepeatedCounterConflict);
                }
                allocated = false;
                counts = Counters::default();
            } else if snapshot.total.regressed(&old.total) {
                reset_epoch = true;
                add_reason(&mut reasons, ReasonCode::CounterReset);
                allocated = false;
            } else {
                counts = snapshot.total.delta(&old.total);
                if !counts.agrees(&snapshot.last) {
                    add_reason(&mut reasons, ReasonCode::CounterGap);
                    allocated = false;
                }
            }
        } else if snapshot.metadata.inherited || !snapshot.total.agrees(&snapshot.last) {
            add_reason(&mut reasons, ReasonCode::InitialCounterBaseline);
            allocated = false;
        }
        if reset_epoch {
            add_reason(&mut reasons, ReasonCode::CounterReset);
            allocated = false;
        }
        if snapshot
            .metadata
            .created_at
            .as_ref()
            .is_some_and(|created| snapshot.at < *created)
        {
            add_reason(&mut reasons, ReasonCode::InheritedHistory);
            allocated = false;
        }
        let lower = snapshot
            .turn_start
            .as_ref()
            .or_else(|| previous.as_ref().map(|old| &old.at))
            .or(snapshot.metadata.created_at.as_ref());
        if snapshot.metadata.inherited
            && snapshot
                .metadata
                .created_at
                .as_ref()
                .is_none_or(|created| lower.is_none_or(|lower| lower < created))
        {
            add_reason(&mut reasons, ReasonCode::InheritedHistory);
            allocated = false;
        }
        let date = snapshot.at[..10].to_string();
        if lower.is_none_or(|lower| {
            lower.as_str() > snapshot.at.as_str() || lower.get(..10) != Some(date.as_str())
        }) {
            add_reason(&mut reasons, ReasonCode::CrossDayInterval);
            allocated = false;
        }
        if snapshot.model_id.is_none() {
            add_reason(&mut reasons, ReasonCode::ModelUnknown);
        }
        result.push(LocalEvent {
            key: snapshot.key.clone(),
            metadata: snapshot.metadata.clone(),
            at: snapshot.at.clone(),
            date,
            allocated,
            model_id: snapshot.model_id.clone(),
            effort: snapshot.effort.clone(),
            speed: snapshot.speed.clone(),
            counts: if allocated {
                counts
            } else {
                Counters::default()
            },
            reason_codes: reasons,
        });
        // A known cumulative state is a baseline even when the interval itself is unallocatable.
        previous = Some(snapshot);
    }
    result
}

pub fn aggregate(events: &[&LocalEvent]) -> Result<TokenComponents, ApiError> {
    let selected: Vec<&LocalEvent> = events
        .iter()
        .copied()
        .filter(|event| event.allocated)
        .collect();
    let sum = |values: Vec<Option<i64>>| -> Result<Metric<String>, ApiError> {
        if values.is_empty() || values.iter().any(Option::is_none) {
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
        let total = values.into_iter().try_fold(0i64, |sum, value| {
            sum.checked_add(value.unwrap()).ok_or_else(|| {
                error(
                    ErrorCode::InvalidData,
                    "Local aggregate exceeds storage range",
                )
            })
        })?;
        Ok(Metric {
            value: Some(total.to_string()),
            quality: QualityStatus::Partial,
            reason_codes: vec![ReasonCode::CoverageIncomplete, ReasonCode::IdentityUnknown],
        })
    };
    let tokens = TokenComponents {
        inclusive_input: sum(selected.iter().map(|event| event.counts.input).collect())?,
        cached_input: sum(selected.iter().map(|event| event.counts.cached).collect())?,
        uncached_input: sum(selected
            .iter()
            .map(|event| {
                event
                    .counts
                    .input
                    .zip(event.counts.cached)
                    .map(|(input, cached)| input - cached)
            })
            .collect())?,
        output: sum(selected.iter().map(|event| event.counts.output).collect())?,
        reasoning_output: sum(selected
            .iter()
            .map(|event| event.counts.reasoning)
            .collect())?,
        text_total: sum(selected
            .iter()
            .map(|event| {
                event
                    .counts
                    .input
                    .zip(event.counts.output)
                    .and_then(|(input, output)| input.checked_add(output))
            })
            .collect())?,
    };
    validate_tokens(&tokens)?;
    Ok(tokens)
}
