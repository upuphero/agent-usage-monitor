//! Versioned, bounded, read-only account adapter. Raw responses and secrets never leave it.
use chrono::{DateTime, Duration, NaiveDate, Utc};
use monitor_contracts::*;
use monitor_core::{error, parse_count, validate_row};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read, path::Path, time::Duration as Timeout};
use zeroize::Zeroizing;

const MAX_BODY: u64 = 4 * 1024 * 1024;
pub const HISTORY_URL: &str =
    "https://chatgpt.com/backend-api/wham/analytics/daily-workspace-usage-counts";
pub const QUOTA_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub struct Credentials {
    access_token: Zeroizing<String>,
    account_id: Zeroizing<String>,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials([redacted])")
    }
}
impl Credentials {
    pub fn account_ref(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"codex-account-v1:");
        hash.update(self.account_id.as_bytes());
        format!("account-{:x}", hash.finalize())
    }
    /// Called only by an explicitly enabled manual sync. Does not refresh or modify login.
    pub fn read(home: &Path) -> Result<Self, ApiError> {
        let file = std::fs::File::open(home.join("auth.json"))
            .map_err(|_| error(ErrorCode::AuthRequired, "Sign in using official Codex"))?;
        let mut bytes = Zeroizing::new(Vec::new());
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| error(ErrorCode::AuthRequired, "Official Codex login unavailable"))?;
        if bytes.len() > 1024 * 1024 {
            return Err(error(
                ErrorCode::AuthRequired,
                "Official Codex login unavailable",
            ));
        }
        Self::decode(&bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, ApiError> {
        #[derive(Deserialize)]
        struct Login {
            tokens: Option<Tokens>,
        }
        #[derive(Deserialize)]
        struct Tokens {
            access_token: String,
            account_id: String,
        }
        let login: Login = serde_json::from_slice(bytes)
            .map_err(|_| error(ErrorCode::AuthRequired, "Official Codex login unavailable"))?;
        let tokens = login
            .tokens
            .ok_or_else(|| error(ErrorCode::AuthRequired, "ChatGPT login required"))?;
        let credentials = Self {
            access_token: Zeroizing::new(tokens.access_token),
            account_id: Zeroizing::new(tokens.account_id),
        };
        if credentials.access_token.is_empty()
            || credentials.access_token.len() > 16384
            || !credentials
                .access_token
                .bytes()
                .all(|c| c.is_ascii_graphic())
            || credentials.account_id.is_empty()
            || credentials.account_id.len() > 128
            || !credentials
                .account_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(error(
                ErrorCode::AuthRequired,
                "Official Codex login unavailable",
            ));
        }
        Ok(credentials)
    }
}

pub fn validate_range(request: &AccountSyncRequest) -> Result<Vec<String>, ApiError> {
    let parse = |value: &str| {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|date| date.format("%Y-%m-%d").to_string() == value)
    };
    let (Some(start), Some(end)) = (parse(&request.start_date), parse(&request.end_date)) else {
        return Err(error(ErrorCode::InvalidQuery, "Use exact YYYY-MM-DD dates"));
    };
    let days = (end - start).num_days();
    if !(0..31).contains(&days) {
        return Err(error(
            ErrorCode::InvalidQuery,
            "Select an inclusive range of at most 31 days",
        ));
    }
    Ok((0..=days)
        .map(|day| (start + Duration::days(day)).to_string())
        .collect())
}

#[derive(Clone, Copy)]
pub enum Endpoint {
    History,
    Quota,
}
pub struct HttpReply {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after_seconds: Option<u32>,
}
pub trait Transport {
    fn get(
        &self,
        endpoint: Endpoint,
        request: &AccountSyncRequest,
        credentials: &Credentials,
    ) -> Result<HttpReply, ApiError>;
}
pub struct HttpsTransport {
    client: reqwest::blocking::Client,
}
impl HttpsTransport {
    pub fn new() -> Result<Self, ApiError> {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Timeout::from_secs(10))
            .timeout(Timeout::from_secs(20))
            .user_agent("AgentUsageMonitor/0.3.0")
            .build()
            .map_err(|_| error(ErrorCode::Offline, "Account connection unavailable"))?;
        Ok(Self { client })
    }
}
impl Transport for HttpsTransport {
    fn get(
        &self,
        endpoint: Endpoint,
        request: &AccountSyncRequest,
        credentials: &Credentials,
    ) -> Result<HttpReply, ApiError> {
        let url = match endpoint {
            Endpoint::History => HISTORY_URL,
            Endpoint::Quota => QUOTA_URL,
        };
        let mut token = reqwest::header::HeaderValue::from_str(&format!(
            "Bearer {}",
            credentials.access_token.as_str()
        ))
        .map_err(|_| error(ErrorCode::AuthRequired, "Official Codex login unavailable"))?;
        token.set_sensitive(true);
        let mut account = reqwest::header::HeaderValue::from_str(&credentials.account_id)
            .map_err(|_| error(ErrorCode::AuthRequired, "Official Codex login unavailable"))?;
        account.set_sensitive(true);
        let mut builder = self
            .client
            .get(url)
            .header(reqwest::header::AUTHORIZATION, token)
            .header("ChatGPT-Account-Id", account)
            .header(reqwest::header::ACCEPT, "application/json");
        if matches!(endpoint, Endpoint::History) {
            builder = builder.query(&[
                ("start_date", request.start_date.as_str()),
                ("end_date", request.end_date.as_str()),
                ("group_by", "day"),
                ("workspace_user", "true"),
            ]);
        }
        let response = builder
            .send()
            .map_err(|_| error(ErrorCode::Offline, "Account request failed or timed out"))?;
        let status = response.status().as_u16();
        let retry = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                value.parse::<u32>().ok().or_else(|| {
                    DateTime::parse_from_rfc2822(value).ok().and_then(|time| {
                        u32::try_from(time.signed_duration_since(Utc::now()).num_seconds().max(0))
                            .ok()
                    })
                })
            })
            .map(|value| value.max(60));
        // Error bodies are discarded, even if they contain credentials or arbitrary HTML.
        if status != 200 {
            return Ok(HttpReply {
                status,
                body: Vec::new(),
                retry_after_seconds: retry,
            });
        }
        let mut body = Vec::new();
        response
            .take(MAX_BODY + 1)
            .read_to_end(&mut body)
            .map_err(|_| error(ErrorCode::Offline, "Account response incomplete"))?;
        if body.len() as u64 > MAX_BODY {
            return Err(incompatible());
        }
        Ok(HttpReply {
            status,
            body,
            retry_after_seconds: retry,
        })
    }
}
fn incompatible() -> ApiError {
    error(
        ErrorCode::ResponseIncompatible,
        "Account response schema is incompatible",
    )
}
fn http_error(status: u16) -> ApiError {
    let code = match status {
        401 => ErrorCode::AuthRequired,
        403 => ErrorCode::AccessDenied,
        404 => ErrorCode::EndpointUnavailable,
        429 => ErrorCode::RateLimited,
        500..=599 => ErrorCode::Offline,
        _ => ErrorCode::ResponseIncompatible,
    };
    error(
        code,
        "Account endpoint unavailable; cached observations retained",
    )
}
fn count(value: Option<&Value>) -> Result<Option<i64>, ApiError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => parse_count(s).map(Some).map_err(|_| incompatible()),
        Some(Value::Number(n)) => n
            .as_i64()
            .filter(|n| *n >= 0)
            .map(Some)
            .ok_or_else(incompatible),
        _ => Err(incompatible()),
    }
}
fn metric(value: Option<i64>) -> Metric<String> {
    Metric {
        value: value.map(|v| v.to_string()),
        quality: if value.is_some() {
            QualityStatus::Known
        } else {
            QualityStatus::Unknown
        },
        reason_codes: if value.is_some() {
            vec![]
        } else {
            vec![ReasonCode::MissingComponent]
        },
    }
}
fn token_vector(value: &Value) -> Result<TokenComponents, ApiError> {
    let fields = value.as_object().ok_or_else(incompatible)?;
    for key in ["unit", "units"] {
        if fields
            .get(key)
            .is_some_and(|unit| unit.as_str() != Some("tokens"))
        {
            return Err(incompatible());
        }
    }
    let u = count(fields.get("uncached_text_input_tokens"))?;
    let c = count(fields.get("cached_text_input_tokens"))?;
    let o = count(fields.get("text_output_tokens"))?;
    let reported = count(fields.get("text_total_tokens"))?;
    let add = |a: Option<i64>, b: Option<i64>| -> Result<Option<i64>, ApiError> {
        match (a, b) {
            (Some(a), Some(b)) => a.checked_add(b).map(Some).ok_or_else(incompatible),
            _ => Ok(None),
        }
    };
    let input = add(u, c)?;
    let total = add(input, o)?;
    if reported.is_some() && total.is_some() && reported != total {
        return Err(incompatible());
    }
    let mut text_total = metric(total);
    if total.is_some() && reported.is_none() {
        text_total.quality = QualityStatus::Partial;
        text_total.reason_codes.push(ReasonCode::MissingComponent);
    }
    Ok(TokenComponents {
        uncached_input: metric(u),
        cached_input: metric(c),
        inclusive_input: metric(input),
        output: metric(o),
        reasoning_output: metric(None),
        text_total,
    })
}
pub fn product_id(client: &str) -> String {
    match client {
        "CODEX_DESKTOP_APP" => "codex-desktop".into(),
        "CODEX_CLI" => "codex-cli".into(),
        "CODEX_IDE_VSCODE" => "codex-vscode".into(),
        "CODEX_WEB" => "codex-web".into(),
        _ => format!("server-client-{:x}", Sha256::digest(client.as_bytes())),
    }
}
pub fn normalize_history(
    body: &[u8],
    request: &AccountSyncRequest,
    account_ref: &str,
    collected: &str,
) -> Result<(Vec<LedgerRow>, Vec<String>), ApiError> {
    let days = validate_range(request)?;
    if body.len() as u64 > MAX_BODY {
        return Err(incompatible());
    }
    let raw: Value = serde_json::from_slice(body).map_err(|_| incompatible())?;
    if raw
        .get("group_by")
        .is_some_and(|v| v.as_str() != Some("day"))
        || ["unit", "units"]
            .iter()
            .any(|key| raw.get(key).is_some_and(|v| v.as_str() != Some("tokens")))
    {
        return Err(incompatible());
    }
    let data = raw
        .get("data")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() <= 1000)
        .ok_or_else(incompatible)?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for entry in data {
        let date = entry
            .get("date")
            .and_then(Value::as_str)
            .ok_or_else(incompatible)?;
        if !days.iter().any(|day| day == date) {
            continue;
        }
        if !seen.insert(date.to_owned()) {
            return Err(incompatible());
        }
        let mut vectors = vec![(
            "codex-account".to_owned(),
            entry
                .get("totals")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )];
        if let Some(clients) = entry.get("clients").filter(|value| !value.is_null()) {
            let clients = clients
                .as_array()
                .filter(|clients| clients.len() <= 128)
                .ok_or_else(incompatible)?;
            let mut ids = BTreeSet::new();
            for client in clients {
                let id = client
                    .get("client_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty() && id.len() <= 128)
                    .ok_or_else(incompatible)?;
                if !ids.insert(id) {
                    return Err(incompatible());
                }
                vectors.push((product_id(id), client.clone()));
            }
        }
        for (product, value) in vectors {
            let tokens = token_vector(&value)?;
            let mut reasons = vec![
                ReasonCode::TimeBasisUnknown,
                ReasonCode::ProductScopeUnverified,
                ReasonCode::CoverageIncomplete,
                ReasonCode::FreshnessUnknown,
            ];
            if tokens.text_total.value.is_none()
                || tokens.text_total.quality == QualityStatus::Partial
            {
                reasons.push(ReasonCode::MissingComponent);
            }
            let row = LedgerRow {
                row_id: uuid::Uuid::new_v4().to_string(),
                ledger: LedgerKind::Account,
                scope: Scope {
                    provider_id: "codex".into(),
                    product_id: product,
                    account_ref: Some(account_ref.into()),
                    identity_evidence: IdentityEvidence::Verified,
                },
                day: DayBucket {
                    date_label: date.into(),
                    basis: TimeBasis::ServerDateLabel,
                    timezone: None,
                    start_utc: None,
                    end_utc: None,
                },
                source: Source {
                    source_id: "codex-analytics-observed-2026-10".into(),
                    kind: SourceKind::ServerObservation,
                    collected_at: collected.into(),
                    data_mode: DataMode::Observed,
                },
                tokens,
                quality: Quality {
                    status: QualityStatus::Partial,
                    coverage: CoverageStatus::Unknown,
                    freshness: FreshnessStatus::Unknown,
                    reason_codes: reasons,
                },
            };
            validate_row(&row).map_err(|_| incompatible())?;
            rows.push(row);
        }
    }
    Ok((
        rows,
        days.into_iter()
            .filter(|date| !seen.contains(date))
            .collect(),
    ))
}
pub fn normalize_quota(
    body: &[u8],
    account_ref: &str,
    collected: &str,
) -> Result<QuotaSnapshot, ApiError> {
    if body.len() as u64 > MAX_BODY || DateTime::parse_from_rfc3339(collected).is_err() {
        return Err(incompatible());
    }
    let raw: Value = serde_json::from_slice(body).map_err(|_| incompatible())?;
    if !raw.is_object()
        || !raw
            .get("rate_limit")
            .is_some_and(|v| v.is_object() || v.is_null())
    {
        return Err(incompatible());
    }
    let rate = &raw["rate_limit"];
    for key in ["primary_window", "secondary_window"] {
        if let Some(value) = rate.get(key).filter(|value| !value.is_null()) {
            if !value.is_object()
                || !value
                    .get("limit_window_seconds")
                    .and_then(Value::as_u64)
                    .is_some_and(|seconds| seconds > 0)
            {
                return Err(incompatible());
            }
        }
    }
    let mut windows = Vec::new();
    for (kind, seconds) in [(QuotaKind::FiveHour, 18000), (QuotaKind::Weekly, 604800)] {
        let matching: Vec<&Value> = ["primary_window", "secondary_window"]
            .iter()
            .map(|key| &rate[*key])
            .filter(|value| value["limit_window_seconds"].as_u64() == Some(seconds))
            .collect();
        if matching.len() > 1 {
            return Err(incompatible());
        }
        let mut window = QuotaWindow {
            kind,
            window_seconds: seconds as u32,
            used_percent: None,
            resets_at: None,
            reason_codes: vec![ReasonCode::WindowNotReported],
        };
        if let Some(value) = matching.first() {
            let percent = value["used_percent"]
                .as_f64()
                .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
                .ok_or_else(incompatible)?;
            window.used_percent = Some(percent.to_string());
            window.reason_codes.clear();
            match value.get("reset_at") {
                None | Some(Value::Null) => window.reason_codes.push(ReasonCode::TimeBasisUnknown),
                Some(reset) => {
                    window.resets_at = Some(
                        DateTime::<Utc>::from_timestamp(
                            reset.as_i64().ok_or_else(incompatible)?,
                            0,
                        )
                        .ok_or_else(incompatible)?
                        .to_rfc3339(),
                    )
                }
            }
        }
        windows.push(window);
    }
    Ok(QuotaSnapshot {
        snapshot_id: uuid::Uuid::new_v4().to_string(),
        scope: Scope {
            provider_id: "codex".into(),
            product_id: "codex-account".into(),
            account_ref: Some(account_ref.into()),
            identity_evidence: IdentityEvidence::Verified,
        },
        source: Source {
            source_id: "codex-quota-observed-2026-10".into(),
            kind: SourceKind::ServerObservation,
            collected_at: collected.into(),
            data_mode: DataMode::Observed,
        },
        windows,
    })
}

pub struct FetchResult {
    pub rows: Vec<LedgerRow>,
    pub quota: Option<QuotaSnapshot>,
    pub summary: AccountSummary,
}
/// At most one request per endpoint. An unsuccessful history request stops this run.
pub fn fetch(
    transport: &impl Transport,
    credentials: &Credentials,
    request: &AccountSyncRequest,
    mut summary: AccountSummary,
    now: DateTime<Utc>,
) -> FetchResult {
    let collected = now.to_rfc3339();
    let account = credentials.account_ref();
    summary.last_attempt_at = Some(collected.clone());
    summary.history_error = None;
    summary.quota_error = None;
    summary.requested_start = Some(request.start_date.clone());
    summary.requested_end = Some(request.end_date.clone());
    summary.next_allowed_at = Some((now + Duration::seconds(60)).to_rfc3339());
    let read =
        |endpoint| -> Result<HttpReply, ApiError> { transport.get(endpoint, request, credentials) };
    let history = read(Endpoint::History).and_then(|reply| {
        if reply.status == 429 {
            summary.next_allowed_at = Some(
                (now + Duration::seconds(i64::from(
                    reply.retry_after_seconds.unwrap_or(60).max(60),
                )))
                .to_rfc3339(),
            );
        }
        if reply.status != 200 {
            return Err(http_error(reply.status));
        }
        normalize_history(&reply.body, request, &account, &collected)
    });
    let (rows, missing) = match history {
        Ok(result) => result,
        Err(failure) => {
            summary.state = state_for(failure.code);
            summary.history_error = Some(failure.code);
            return FetchResult {
                rows: vec![],
                quota: None,
                summary,
            };
        }
    };
    summary.history_fetched_at = Some(collected.clone());
    summary.missing_dates = missing;
    let quota = read(Endpoint::Quota).and_then(|reply| {
        if reply.status == 429 {
            summary.next_allowed_at = Some(
                (now + Duration::seconds(i64::from(
                    reply.retry_after_seconds.unwrap_or(60).max(60),
                )))
                .to_rfc3339(),
            );
        }
        if reply.status != 200 {
            return Err(http_error(reply.status));
        }
        normalize_quota(&reply.body, &account, &collected)
    });
    let quota = match quota {
        Ok(quota) => {
            summary.quota_fetched_at = Some(collected);
            Some(quota)
        }
        Err(failure) => {
            summary.quota_error = Some(failure.code);
            None
        }
    };
    summary.state = if summary.quota_error.is_some()
        || !summary.missing_dates.is_empty()
        || rows.iter().any(|row| {
            row.tokens.text_total.value.is_none()
                || row.tokens.text_total.quality == QualityStatus::Partial
        }) {
        AccountState::Partial
    } else {
        AccountState::Ready
    };
    FetchResult {
        rows,
        quota,
        summary,
    }
}
pub fn state_for(code: ErrorCode) -> AccountState {
    match code {
        ErrorCode::AuthRequired => AccountState::AuthRequired,
        ErrorCode::AccessDenied => AccountState::AccessDenied,
        ErrorCode::EndpointUnavailable => AccountState::EndpointUnavailable,
        ErrorCode::RateLimited => AccountState::RateLimited,
        ErrorCode::Offline => AccountState::Offline,
        _ => AccountState::Incompatible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    use std::cell::Cell;
    const HISTORY: &[u8] = include_bytes!("../../../tests/fixtures/account-history.json");
    const QUOTA: &[u8] = include_bytes!("../../../tests/fixtures/account-quota.json");
    const COLLECTED: &str = "2026-10-12T16:00:00Z";
    fn request() -> AccountSyncRequest {
        AccountSyncRequest {
            start_date: "2026-10-08".into(),
            end_date: "2026-10-10".into(),
        }
    }
    fn credentials(id: &str) -> Credentials {
        Credentials::decode(
            &serde_json::to_vec(
                &serde_json::json!({"tokens":{"access_token":"synthetic-secret", "account_id":id}}),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn store() -> Store {
        Store::initialize(rusqlite::Connection::open_in_memory().unwrap()).unwrap()
    }
    struct Mock {
        history_status: u16,
        quota_status: u16,
        calls: Cell<u32>,
        offline: bool,
    }
    impl Mock {
        fn success() -> Self {
            Self {
                history_status: 200,
                quota_status: 200,
                calls: Cell::new(0),
                offline: false,
            }
        }
    }
    impl Transport for Mock {
        fn get(
            &self,
            endpoint: Endpoint,
            _: &AccountSyncRequest,
            _: &Credentials,
        ) -> Result<HttpReply, ApiError> {
            self.calls.set(self.calls.get() + 1);
            if self.offline {
                return Err(error(ErrorCode::Offline, "safe offline error"));
            }
            let (status, body) = match endpoint {
                Endpoint::History => (self.history_status, HISTORY),
                Endpoint::Quota => (self.quota_status, QUOTA),
            };
            Ok(HttpReply {
                status,
                body: if status == 200 {
                    body.to_vec()
                } else {
                    b"Authorization: Bearer synthetic-secret; private SQL and path".to_vec()
                },
                retry_after_seconds: Some(600),
            })
        }
    }
    fn captured(store: &mut Store, mock: &Mock, id: &str, now: DateTime<Utc>) -> FetchResult {
        let credentials = credentials(id);
        let summary = store.bind_account(&credentials.account_ref()).unwrap();
        fetch(mock, &credentials, &request(), summary, now)
    }
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(COLLECTED)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn range_is_inclusive_bounded_and_exact() {
        assert_eq!(validate_range(&request()).unwrap().len(), 3);
        for (start, end) in [
            ("2026-10-1", "2026-10-02"),
            ("2026-10-10", "2026-10-08"),
            ("2026-10-01", "2026-11-01"),
            ("2026-02-30", "2026-03-01"),
        ] {
            assert_eq!(
                validate_range(&AccountSyncRequest {
                    start_date: start.into(),
                    end_date: end.into()
                })
                .unwrap_err()
                .code,
                ErrorCode::InvalidQuery
            );
        }
    }
    #[test]
    fn absolute_counts_dates_and_product_scopes_are_independent() {
        let (rows, missing) =
            normalize_history(HISTORY, &request(), "account-test", COLLECTED).unwrap();
        assert!(missing.is_empty());
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].tokens.text_total.value.as_deref(), Some("110"));
        assert_eq!(rows[0].scope.product_id, "codex-account");
        assert_eq!(rows[1].scope.product_id, "codex-desktop");
        assert!(rows
            .iter()
            .all(|row| row.day.basis == TimeBasis::ServerDateLabel
                && row.day.timezone.is_none()
                && row.quality.coverage == CoverageStatus::Unknown
                && row.quality.freshness == FreshnessStatus::Unknown));
        assert_eq!(rows[5].tokens.text_total.value.as_deref(), Some("0"));
        assert!(rows
            .iter()
            .all(|row| row.tokens.reasoning_output.value.is_none()));
    }
    #[test]
    fn missing_dates_and_components_do_not_become_zero() {
        let raw = serde_json::json!({"data":[{"date":"2026-10-08","totals":{"uncached_text_input_tokens":4,"text_output_tokens":2,"text_total_tokens":9}}]});
        let (rows, missing) = normalize_history(
            &serde_json::to_vec(&raw).unwrap(),
            &request(),
            "test",
            COLLECTED,
        )
        .unwrap();
        assert_eq!(missing, vec!["2026-10-09", "2026-10-10"]);
        assert_eq!(rows[0].tokens.text_total.value, None);
        assert_eq!(rows[0].tokens.uncached_input.value.as_deref(), Some("4"));
    }
    #[test]
    fn large_integer_precision_and_unsafe_or_changed_units_fail_closed() {
        let raw = |u: Value, c: Value, o: Value, total: Value| serde_json::json!({"data":[{"date":"2026-10-08","totals":{"uncached_text_input_tokens":u,"cached_text_input_tokens":c,"text_output_tokens":o,"text_total_tokens":total}}]});
        let exact = raw(
            serde_json::json!(9007199254740993i64),
            serde_json::json!(0),
            serde_json::json!(0),
            serde_json::json!(9007199254740993i64),
        );
        assert_eq!(
            normalize_history(
                &serde_json::to_vec(&exact).unwrap(),
                &request(),
                "test",
                COLLECTED
            )
            .unwrap()
            .0[0]
                .tokens
                .text_total
                .value
                .as_deref(),
            Some("9007199254740993")
        );
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("01"),
            serde_json::json!("9223372036854775808"),
        ] {
            assert!(normalize_history(
                &serde_json::to_vec(&raw(
                    invalid,
                    serde_json::json!(0),
                    serde_json::json!(0),
                    serde_json::json!(0)
                ))
                .unwrap(),
                &request(),
                "test",
                COLLECTED
            )
            .is_err());
        }
        let mut overflow = raw(
            serde_json::json!(i64::MAX),
            serde_json::json!(1),
            serde_json::json!(0),
            Value::Null,
        );
        assert!(normalize_history(
            &serde_json::to_vec(&overflow).unwrap(),
            &request(),
            "test",
            COLLECTED
        )
        .is_err());
        overflow["units"] = serde_json::json!("percent");
        assert!(normalize_history(
            &serde_json::to_vec(&overflow).unwrap(),
            &request(),
            "test",
            COLLECTED
        )
        .is_err());
    }
    #[test]
    fn inconsistent_totals_duplicate_days_and_clients_are_rejected() {
        let original: Value = serde_json::from_slice(HISTORY).unwrap();
        let mut bad = original.clone();
        bad["data"][0]["totals"]["text_total_tokens"] = serde_json::json!(111);
        assert!(normalize_history(
            &serde_json::to_vec(&bad).unwrap(),
            &request(),
            "test",
            COLLECTED
        )
        .is_err());
        let mut bad = original.clone();
        let duplicate = bad["data"][0].clone();
        bad["data"].as_array_mut().unwrap().push(duplicate);
        assert!(normalize_history(
            &serde_json::to_vec(&bad).unwrap(),
            &request(),
            "test",
            COLLECTED
        )
        .is_err());
        let mut bad = original;
        let duplicate = bad["data"][0]["clients"][0].clone();
        bad["data"][0]["clients"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(normalize_history(
            &serde_json::to_vec(&bad).unwrap(),
            &request(),
            "test",
            COLLECTED
        )
        .is_err());
    }
    #[test]
    fn quota_classification_uses_duration_instead_of_primary_position() {
        let quota = normalize_quota(QUOTA, "test", COLLECTED).unwrap();
        assert_eq!(quota.windows[0].used_percent, None);
        assert_eq!(
            quota.windows[0].reason_codes,
            vec![ReasonCode::WindowNotReported]
        );
        assert_eq!(quota.windows[1].used_percent.as_deref(), Some("36.5"));
        let raw = serde_json::json!({"rate_limit":{"primary_window":{"used_percent":0,"limit_window_seconds":18000},"secondary_window":{"used_percent":100,"limit_window_seconds":604800}}});
        let quota = normalize_quota(&serde_json::to_vec(&raw).unwrap(), "test", COLLECTED).unwrap();
        assert_eq!(quota.windows[0].used_percent.as_deref(), Some("0"));
        assert_eq!(quota.windows[0].resets_at, None);
        for raw in [
            serde_json::json!({"rate_limit":{"primary_window":{"used_percent":101,"limit_window_seconds":18000}}}),
            serde_json::json!({"rate_limit":{"primary_window":{"used_percent":"private","limit_window_seconds":18000}}}),
            serde_json::json!({"credits":{"balance":"100"}}),
        ] {
            assert!(
                normalize_quota(&serde_json::to_vec(&raw).unwrap(), "test", COLLECTED).is_err()
            );
        }
    }
    #[test]
    fn non_success_responses_stop_without_retry_and_never_expose_error_bodies() {
        for (status, code) in [
            (401, ErrorCode::AuthRequired),
            (403, ErrorCode::AccessDenied),
            (404, ErrorCode::EndpointUnavailable),
            (429, ErrorCode::RateLimited),
            (302, ErrorCode::ResponseIncompatible),
            (500, ErrorCode::Offline),
        ] {
            let mut store = store();
            store.set_account_queries(true).unwrap();
            let mut mock = Mock::success();
            mock.history_status = status;
            let fetched = captured(&mut store, &mock, "synthetic-a", now());
            assert_eq!(mock.calls.get(), 1);
            assert_eq!(fetched.summary.history_error, Some(code));
            assert!(fetched.rows.is_empty());
            assert!(!serde_json::to_string(&fetched.summary)
                .unwrap()
                .contains("synthetic-secret"));
            store.commit_account_fetch(&fetched).unwrap();
            if status == 429 {
                assert_eq!(
                    store
                        .reserve_account_sync(&request(), now() + Duration::seconds(61))
                        .unwrap_err()
                        .code,
                    ErrorCode::RateLimited
                );
            }
        }
    }
    #[test]
    fn quota_failure_keeps_valid_history_and_previous_quota() {
        let mut store = store();
        store.set_account_queries(true).unwrap();
        let first = captured(&mut store, &Mock::success(), "synthetic-a", now());
        store.commit_account_fetch(&first).unwrap();
        let mut mock = Mock::success();
        mock.quota_status = 401;
        let second = captured(
            &mut store,
            &mock,
            "synthetic-a",
            now() + Duration::minutes(1),
        );
        store.commit_account_fetch(&second).unwrap();
        let data = store.dashboard(DemoScenario::Local).unwrap();
        assert_eq!(data.account_rows.len(), 14);
        assert_eq!(
            data.account.unwrap().quota_error,
            Some(ErrorCode::AuthRequired)
        );
        assert_eq!(
            data.quota.unwrap().snapshot_id,
            first.quota.unwrap().snapshot_id
        );
    }
    #[test]
    fn disabled_queries_throttle_and_account_switches_are_enforced() {
        let mut store = store();
        assert_eq!(
            store
                .reserve_account_sync(&request(), now())
                .unwrap_err()
                .code,
            ErrorCode::NetworkDisabled
        );
        store.set_account_queries(true).unwrap();
        store.reserve_account_sync(&request(), now()).unwrap();
        assert_eq!(
            store
                .reserve_account_sync(&request(), now() + Duration::seconds(1))
                .unwrap_err()
                .code,
            ErrorCode::RateLimited
        );
        let first = captured(&mut store, &Mock::success(), "synthetic-a", now());
        store.commit_account_fetch(&first).unwrap();
        store
            .bind_account(&credentials("synthetic-b").account_ref())
            .unwrap();
        let switched = store.dashboard(DemoScenario::Local).unwrap();
        assert!(switched.account_rows.is_empty());
        assert!(switched.quota.is_none());
        assert!(store.commit_account_fetch(&first).is_err());
        store
            .bind_account(&credentials("synthetic-a").account_ref())
            .unwrap();
        assert_eq!(
            store
                .dashboard(DemoScenario::Local)
                .unwrap()
                .account_rows
                .len(),
            7
        );
        store.set_account_queries(false).unwrap();
        assert!(store.commit_account_fetch(&first).is_err());
        let data = store.dashboard(DemoScenario::Local).unwrap();
        assert_eq!(data.account_rows.len(), 7);
        assert!(!data.runtime.network_enabled);
        assert!(data
            .quota
            .unwrap()
            .windows
            .iter()
            .all(|w| w.reason_codes.contains(&ReasonCode::QuotaStale)));
    }
    #[test]
    fn transaction_failure_preserves_prior_snapshots_and_sync_metadata() {
        let mut store = store();
        store.set_account_queries(true).unwrap();
        let first = captured(&mut store, &Mock::success(), "synthetic-a", now());
        store.commit_account_fetch(&first).unwrap();
        let mut second = captured(
            &mut store,
            &Mock::success(),
            "synthetic-a",
            now() + Duration::minutes(1),
        );
        second.rows[1].row_id = first.rows[0].row_id.clone();
        assert!(store.commit_account_fetch(&second).is_err());
        let data = store.dashboard(DemoScenario::Local).unwrap();
        assert_eq!(data.account_rows.len(), 7);
        assert_eq!(
            data.account.unwrap().history_fetched_at,
            Some(now().to_rfc3339())
        );
    }
    #[test]
    fn offline_and_login_failure_keep_cached_ledgers_and_clear_current_claims() {
        let mut store = store();
        store.seed_synthetic().unwrap();
        store.set_account_queries(true).unwrap();
        let first = captured(&mut store, &Mock::success(), "synthetic-a", now());
        store.commit_account_fetch(&first).unwrap();
        let mut mock = Mock::success();
        mock.offline = true;
        let failed = captured(
            &mut store,
            &mock,
            "synthetic-a",
            now() + Duration::minutes(1),
        );
        store.commit_account_fetch(&failed).unwrap();
        let data = store.dashboard(DemoScenario::Local).unwrap();
        assert_eq!(data.account_rows.len(), 7);
        assert_eq!(data.account.unwrap().state, AccountState::Offline);
        assert!(data.comparisons.is_empty());
        assert!(data
            .account_rows
            .iter()
            .all(|row| row.source.data_mode == DataMode::Observed));
        store.account_login_failed().unwrap();
        assert_eq!(
            store.account_summary().unwrap().state,
            AccountState::AuthRequired
        );
        assert!(format!("{:?}", credentials("synthetic-a")).contains("redacted"));
        assert!(Credentials::decode(
            br#"{"tokens":{"access_token":"secret","account_id":"bad\nheader"}}"#
        )
        .is_err());
        assert!(Credentials::decode(br#"{"OPENAI_API_KEY":"secret"}"#).is_err());
    }
    #[test]
    fn disk_reopen_keeps_all_account_revisions_and_binding() {
        let directory =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local-data/account-tests");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{}.sqlite3", uuid::Uuid::new_v4()));
        {
            let mut store = Store::open(&path).unwrap();
            store.set_account_queries(true).unwrap();
            let first = captured(&mut store, &Mock::success(), "synthetic-a", now());
            store.commit_account_fetch(&first).unwrap();
            let second = captured(
                &mut store,
                &Mock::success(),
                "synthetic-a",
                now() + Duration::minutes(1),
            );
            store.commit_account_fetch(&second).unwrap();
        }
        let store = Store::open(&path).unwrap();
        let data = store.dashboard(DemoScenario::Local).unwrap();
        assert_eq!(data.account_rows.len(), 14);
        assert_eq!(
            data.account.unwrap().account_ref,
            Some(credentials("synthetic-a").account_ref())
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
