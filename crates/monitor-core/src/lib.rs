use chrono::{DateTime, Duration, NaiveDate};
use monitor_contracts::*;

pub fn error(code: ErrorCode, message: &str) -> ApiError {
    ApiError {
        api_version: API_VERSION.into(),
        code,
        message: message.into(),
        retryable: false,
    }
}

/// SQLite and IPC share a signed 64-bit storage ceiling; negative counts are never accepted.
pub fn parse_count(value: &str) -> Result<i64, ApiError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Token count must be a canonical nonnegative integer",
        ));
    }
    value
        .parse::<i64>()
        .map_err(|_| error(ErrorCode::InvalidData, "Token count exceeds storage range"))
}

fn metric_value(metric: &Metric<String>) -> Result<Option<i64>, ApiError> {
    match &metric.value {
        None if metric.quality == QualityStatus::Unknown && !metric.reason_codes.is_empty() => {
            Ok(None)
        }
        Some(value) if metric.quality != QualityStatus::Unknown => parse_count(value).map(Some),
        _ => Err(error(
            ErrorCode::InvalidData,
            "Metric quality and value disagree",
        )),
    }
}

pub fn validate_tokens(tokens: &TokenComponents) -> Result<(), ApiError> {
    let uncached = metric_value(&tokens.uncached_input)?;
    let cached = metric_value(&tokens.cached_input)?;
    let input = metric_value(&tokens.inclusive_input)?;
    let output = metric_value(&tokens.output)?;
    let reasoning = metric_value(&tokens.reasoning_output)?;
    let total = metric_value(&tokens.text_total)?;
    let expected_input = match (uncached, cached) {
        (Some(u), Some(c)) => Some(
            u.checked_add(c)
                .ok_or_else(|| error(ErrorCode::InvalidData, "Token sum exceeds storage range"))?,
        ),
        _ => None,
    };
    let expected_total = match (expected_input, output) {
        (Some(i), Some(o)) => Some(
            i.checked_add(o)
                .ok_or_else(|| error(ErrorCode::InvalidData, "Token sum exceeds storage range"))?,
        ),
        _ => None,
    };
    if input != expected_input
        || total != expected_total
        || matches!((reasoning, output), (Some(r), Some(o)) if r > o)
        || (reasoning.is_some() && output.is_none())
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Token components are inconsistent",
        ));
    }
    Ok(())
}

pub fn validate_row(row: &LedgerRow) -> Result<(), ApiError> {
    validate_tokens(&row.tokens)?;
    if row.row_id.is_empty()
        || row.scope.provider_id.is_empty()
        || row.scope.product_id.is_empty()
        || row.source.source_id.is_empty()
        || row.scope.account_ref.as_deref() == Some("")
        || (row.scope.identity_evidence == IdentityEvidence::Verified
            && row.scope.account_ref.is_none())
    {
        return Err(error(
            ErrorCode::InvalidData,
            "Row scope or identity is invalid",
        ));
    }
    if !matches!(
        (row.ledger, row.source.kind),
        (LedgerKind::Account, SourceKind::ServerObservation)
            | (LedgerKind::Windows, SourceKind::LocalLog)
    ) {
        return Err(error(ErrorCode::InvalidData, "Ledger and source disagree"));
    }
    let date = NaiveDate::parse_from_str(&row.day.date_label, "%Y-%m-%d")
        .map_err(|_| error(ErrorCode::InvalidData, "Invalid date label"))?;
    if date.format("%Y-%m-%d").to_string() != row.day.date_label
        || DateTime::parse_from_rfc3339(&row.source.collected_at).is_err()
    {
        return Err(error(ErrorCode::InvalidData, "Invalid observation time"));
    }
    if row.day.basis == TimeBasis::VerifiedUtcDay {
        let start = format!("{}T00:00:00Z", row.day.date_label);
        let end = format!(
            "{}T00:00:00Z",
            date.checked_add_signed(Duration::days(1))
                .ok_or_else(|| error(ErrorCode::InvalidData, "Invalid day end"))?
        );
        if row.day.timezone.as_deref() != Some("UTC")
            || row.day.start_utc.as_deref() != Some(&start)
            || row.day.end_utc.as_deref() != Some(&end)
        {
            return Err(error(
                ErrorCode::InvalidData,
                "Verified UTC bucket must contain exact exclusive bounds",
            ));
        }
    }
    Ok(())
}

pub fn compare(account: &LedgerRow, local: &LedgerRow) -> ComparisonResult {
    let mut reasons = Vec::new();
    let mut blocked = |reason| {
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    };
    if validate_row(account).is_err() || validate_row(local).is_err() {
        blocked(ReasonCode::InconsistentComponents);
    }
    if account.ledger != LedgerKind::Account || local.ledger != LedgerKind::Windows {
        blocked(ReasonCode::SourceMismatch);
    }
    if account.source.data_mode != local.source.data_mode {
        blocked(ReasonCode::SourceMismatch);
    }
    if account.scope.provider_id != local.scope.provider_id {
        blocked(ReasonCode::ProviderMismatch);
    }
    if account.scope.product_id != local.scope.product_id {
        blocked(ReasonCode::ProductMismatch);
    }
    if account.scope.account_ref.is_none()
        || local.scope.account_ref.is_none()
        || account.scope.identity_evidence != IdentityEvidence::Verified
        || local.scope.identity_evidence != IdentityEvidence::Verified
    {
        blocked(ReasonCode::IdentityUnknown);
    } else if account.scope.account_ref != local.scope.account_ref {
        blocked(ReasonCode::AccountMismatch);
    }
    if account.day.date_label != local.day.date_label {
        blocked(ReasonCode::DateMismatch);
    }
    if account.day.basis != TimeBasis::VerifiedUtcDay
        || local.day.basis != TimeBasis::VerifiedUtcDay
        || account.day.start_utc != local.day.start_utc
        || account.day.end_utc != local.day.end_utc
    {
        blocked(ReasonCode::TimeBasisUnknown);
    }
    if account.quality.coverage != CoverageStatus::Complete
        || local.quality.coverage != CoverageStatus::Complete
    {
        blocked(ReasonCode::CoverageIncomplete);
    }
    let components = |row: &LedgerRow| {
        [
            &row.tokens.uncached_input,
            &row.tokens.cached_input,
            &row.tokens.output,
            &row.tokens.text_total,
        ]
        .map(|m| m.value.clone())
    };
    let server = components(account);
    let windows = components(local);
    if server.iter().chain(windows.iter()).any(Option::is_none) {
        blocked(ReasonCode::MissingComponent);
    }
    let mut result = ComparisonResult {
        date_label: account.day.date_label.clone(),
        status: ComparisonStatus::Blocked,
        uncached_input: None,
        cached_input: None,
        output: None,
        text_total: None,
        reason_codes: reasons,
    };
    if !result.reason_codes.is_empty() {
        return result;
    }
    let values: Vec<i64> = server
        .iter()
        .zip(windows.iter())
        .map(|(s, w)| {
            // Validation above guarantees nonnegative i64 values, so their difference fits i64.
            parse_count(s.as_deref().unwrap()).unwrap()
                - parse_count(w.as_deref().unwrap()).unwrap()
        })
        .collect();
    result.uncached_input = Some(values[0].to_string());
    result.cached_input = Some(values[1].to_string());
    result.output = Some(values[2].to_string());
    result.text_total = Some(values[3].to_string());
    result.status = if values.iter().any(|v| *v < 0) {
        ComparisonStatus::Inconsistent
    } else {
        ComparisonStatus::Unattributed
    };
    if result.status == ComparisonStatus::Inconsistent {
        result.reason_codes.push(ReasonCode::NegativeResidual);
    }
    if account.source.data_mode == DataMode::Synthetic {
        result.reason_codes.push(ReasonCode::SyntheticData);
    }
    if account.quality.freshness == FreshnessStatus::PendingBackfill {
        result.reason_codes.push(ReasonCode::PendingBackfill);
    }
    if account.quality.freshness == FreshnessStatus::Unknown {
        result.reason_codes.push(ReasonCode::FreshnessUnknown);
    }
    result.reason_codes.push(ReasonCode::CalibrationNotRun);
    result
}

pub fn rebuild_comparisons(dashboard: &mut Dashboard) {
    dashboard.comparisons.clear();
    for local in &dashboard.local_rows {
        let account = dashboard
            .account_rows
            .iter()
            .filter(|row| {
                row.day.date_label == local.day.date_label
                    && row.scope.provider_id == local.scope.provider_id
                    && row.scope.product_id == local.scope.product_id
                    && row.scope.account_ref == local.scope.account_ref
            })
            .max_by_key(|row| DateTime::parse_from_rfc3339(&row.source.collected_at).ok());
        if let Some(account) = account {
            dashboard.comparisons.push(compare(account, local));
        }
    }
}

pub fn demo(scenario: DemoScenario) -> Result<Dashboard, ApiError> {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/demo.json"))
            .map_err(|_| error(ErrorCode::Internal, "Synthetic fixture is invalid"))?;
    let key = match scenario {
        DemoScenario::Ready => "ready",
        DemoScenario::Missing => "missing",
        DemoScenario::Negative => "negative",
        DemoScenario::Empty => "empty",
    };
    let mut result: Dashboard = serde_json::from_value(fixtures[key].clone())
        .map_err(|_| error(ErrorCode::Internal, "Synthetic contract is invalid"))?;
    for row in result.account_rows.iter().chain(result.local_rows.iter()) {
        validate_row(row)?;
    }
    rebuild_comparisons(&mut result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows() -> (LedgerRow, LedgerRow) {
        let dashboard = demo(DemoScenario::Ready).unwrap();
        (
            dashboard.account_rows[0].clone(),
            dashboard.local_rows[0].clone(),
        )
    }
    #[test]
    fn integer_bounds_and_precision() {
        assert_eq!(
            parse_count("9007199254740993").unwrap(),
            9_007_199_254_740_993
        );
        assert_eq!(parse_count("9223372036854775807").unwrap(), i64::MAX);
        for invalid in ["-1", "1.0", "01", "", " 1", "9223372036854775808"] {
            assert!(parse_count(invalid).is_err());
        }
    }
    #[test]
    fn cache_counted_once_and_reasoning_is_subset() {
        let (mut account, _) = rows();
        account.tokens.text_total.value = Some("1".into());
        assert!(validate_tokens(&account.tokens).is_err());
        let (mut account, _) = rows();
        account.tokens.reasoning_output.value = Some("99999999".into());
        assert!(validate_tokens(&account.tokens).is_err());
    }
    #[test]
    fn missing_is_not_zero_and_negative_is_not_clamped() {
        let missing = demo(DemoScenario::Missing).unwrap();
        let row = missing.comparisons.last().unwrap();
        assert_eq!(row.status, ComparisonStatus::Blocked);
        assert_eq!(row.text_total, None);
        assert!(row.reason_codes.contains(&ReasonCode::MissingComponent));
        let negative = demo(DemoScenario::Negative).unwrap();
        let row = negative.comparisons.last().unwrap();
        assert_eq!(row.status, ComparisonStatus::Inconsistent);
        assert!(row.text_total.as_ref().unwrap().starts_with('-'));
    }
    #[test]
    fn provider_product_account_time_and_coverage_gates() {
        let (account, local) = rows();
        let mut changes = Vec::new();
        let mut row = local.clone();
        row.scope.provider_id = "another".into();
        changes.push((row, ReasonCode::ProviderMismatch));
        let mut row = local.clone();
        row.scope.product_id = "all-products".into();
        changes.push((row, ReasonCode::ProductMismatch));
        let mut row = local.clone();
        row.scope.account_ref = Some("other-account".into());
        changes.push((row, ReasonCode::AccountMismatch));
        let mut row = local.clone();
        row.scope.identity_evidence = IdentityEvidence::Unknown;
        changes.push((row, ReasonCode::IdentityUnknown));
        let mut row = local.clone();
        row.day.basis = TimeBasis::LocalDay;
        changes.push((row, ReasonCode::TimeBasisUnknown));
        let mut row = local.clone();
        row.quality.coverage = CoverageStatus::Partial;
        changes.push((row, ReasonCode::CoverageIncomplete));
        for (row, reason) in changes {
            let comparison = compare(&account, &row);
            assert_eq!(comparison.text_total, None);
            assert!(comparison.reason_codes.contains(&reason));
        }
    }
    #[test]
    fn latest_snapshot_and_synthetic_fixture_parity() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/demo.json")).unwrap();
        for (key, scenario) in [
            ("ready", DemoScenario::Ready),
            ("missing", DemoScenario::Missing),
            ("negative", DemoScenario::Negative),
            ("empty", DemoScenario::Empty),
        ] {
            let expected: Dashboard = serde_json::from_value(fixtures[key].clone()).unwrap();
            assert_eq!(demo(scenario).unwrap().comparisons, expected.comparisons);
        }
        let ready = demo(DemoScenario::Ready).unwrap();
        assert!(ready
            .providers
            .iter()
            .all(|provider| !provider.capabilities.calibrated_estimate
                && !provider.capabilities.incremental_collection));
        assert!(ready.comparisons.iter().all(|comparison| comparison
            .reason_codes
            .contains(&ReasonCode::CalibrationNotRun)));
    }
    #[test]
    fn unknown_quality_requires_null_with_reason_and_utc_bounds_are_exact() {
        let (mut account, _) = rows();
        account.tokens.output.quality = QualityStatus::Unknown;
        assert!(validate_row(&account).is_err());
        let (mut account, _) = rows();
        account.day.end_utc = Some("2026-10-12T00:00:00Z".into());
        assert!(validate_row(&account).is_err());
    }
}
