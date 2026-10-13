use crate::{
    insert_row,
    server::{self, FetchResult},
    storage_error, Store,
};
use chrono::{DateTime, Duration, Utc};
use monitor_contracts::*;
use monitor_core::{error, validate_row};
use rusqlite::{params, OptionalExtension};

fn empty_summary(enabled: bool) -> AccountSummary {
    AccountSummary {
        enabled,
        account_ref: None,
        state: if enabled {
            AccountState::NeverSynced
        } else {
            AccountState::Disabled
        },
        last_attempt_at: None,
        history_fetched_at: None,
        quota_fetched_at: None,
        history_error: None,
        quota_error: None,
        next_allowed_at: None,
        requested_start: None,
        requested_end: None,
        missing_dates: vec![],
    }
}
impl Store {
    pub fn account_queries_enabled(&self) -> Result<bool, ApiError> {
        let value: String = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='account-query-enabled'",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        Ok(value == "true")
    }
    pub fn set_account_queries(&self, enabled: bool) -> Result<(), ApiError> {
        self.connection
            .execute(
                "UPDATE app_settings SET value=?1 WHERE key='account-query-enabled'",
                [if enabled { "true" } else { "false" }],
            )
            .map_err(storage_error)?;
        Ok(())
    }
    /// Reserve a bounded request slot before any credential file is opened.
    pub fn reserve_account_sync(
        &self,
        request: &AccountSyncRequest,
        now: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        server::validate_range(request)?;
        if !self.account_queries_enabled()? {
            return Err(error(
                ErrorCode::NetworkDisabled,
                "Enable account queries before synchronizing",
            ));
        }
        let next: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='account-next-allowed'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        if next
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|next| next > now)
        {
            return Err(error(
                ErrorCode::RateLimited,
                "Wait before requesting another account snapshot",
            ));
        }
        self.connection.execute("INSERT INTO app_settings VALUES ('account-next-allowed',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [(now + Duration::seconds(60)).to_rfc3339()]).map_err(storage_error)?;
        Ok(())
    }
    pub fn bind_account(&mut self, account_ref: &str) -> Result<AccountSummary, ApiError> {
        if !self.account_queries_enabled()? {
            return Err(error(
                ErrorCode::NetworkDisabled,
                "Account queries disabled",
            ));
        }
        if !account_ref
            .strip_prefix("account-")
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(error(ErrorCode::InvalidData, "Invalid account reference"));
        }
        let transaction = self.connection.transaction().map_err(storage_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO accounts VALUES ('codex',?1,'verified')",
                [account_ref],
            )
            .map_err(storage_error)?;
        transaction.execute("INSERT INTO app_settings VALUES ('account-active-ref',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [account_ref]).map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM app_settings WHERE key='account-auth-error'",
                [],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        let mut summary = self.account_summary()?;
        summary.account_ref = Some(account_ref.into());
        Ok(summary)
    }
    pub fn account_login_failed(&self) -> Result<(), ApiError> {
        self.connection.execute("INSERT INTO app_settings VALUES ('account-auth-error','true') ON CONFLICT(key) DO UPDATE SET value=excluded.value", []).map_err(storage_error)?;
        Ok(())
    }
    pub fn commit_account_fetch(&mut self, result: &FetchResult) -> Result<(), ApiError> {
        if !self.account_queries_enabled()? {
            return Err(error(
                ErrorCode::NetworkDisabled,
                "Account queries disabled during synchronization",
            ));
        }
        let active = self.account_summary()?.account_ref;
        if active.is_none() || result.summary.account_ref != active {
            return Err(error(
                ErrorCode::InvalidData,
                "Account changed during synchronization",
            ));
        }
        for row in &result.rows {
            validate_row(row)?;
            if row.ledger != LedgerKind::Account
                || row.source.data_mode != DataMode::Observed
                || row.scope.provider_id != "codex"
                || row.scope.account_ref != active
            {
                return Err(error(
                    ErrorCode::InvalidData,
                    "Invalid account snapshot scope",
                ));
            }
        }
        if result.quota.as_ref().is_some_and(|quota| {
            quota.scope.account_ref != active
                || quota.scope.provider_id != "codex"
                || quota.scope.product_id != "codex-account"
                || quota.source.data_mode != DataMode::Observed
        }) {
            return Err(error(
                ErrorCode::InvalidData,
                "Invalid quota snapshot scope",
            ));
        }
        let transaction = self.connection.transaction().map_err(storage_error)?;
        for row in &result.rows {
            insert_row(&transaction, row)?;
        }
        if let Some(quota) = &result.quota {
            transaction
                .execute(
                    "INSERT INTO quota_snapshots VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        quota.snapshot_id,
                        quota.scope.provider_id,
                        quota.scope.product_id,
                        quota.scope.account_ref,
                        quota.source.collected_at,
                        serde_json::to_string(quota).map_err(storage_error)?
                    ],
                )
                .map_err(storage_error)?;
        }
        transaction.execute("INSERT INTO account_reader_state VALUES ('codex',?1,?2) ON CONFLICT(provider_id,account_ref) DO UPDATE SET summary_json=excluded.summary_json", params![active, serde_json::to_string(&result.summary).map_err(storage_error)?]).map_err(storage_error)?;
        if let Some(next) = &result.summary.next_allowed_at {
            transaction.execute("INSERT INTO app_settings VALUES ('account-next-allowed',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [next]).map_err(storage_error)?;
        }
        let status = result
            .summary
            .history_error
            .or(result.summary.quota_error)
            .map(|code| serde_json::to_string(&code))
            .transpose()
            .map_err(storage_error)?
            .unwrap_or_else(|| "\"OK\"".into());
        transaction
            .execute(
                "INSERT INTO app_audit VALUES (?1,'account-sync',?2,?3)",
                params![
                    result.summary.last_attempt_at,
                    status,
                    crate::SCHEMA_VERSION
                ],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }
    pub fn account_summary(&self) -> Result<AccountSummary, ApiError> {
        let enabled = self.account_queries_enabled()?;
        let active: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='account-active-ref'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        let json: Option<String> = self.connection.query_row("SELECT summary_json FROM account_reader_state WHERE provider_id='codex' AND account_ref=?1", [&active], |row| row.get(0)).optional().map_err(storage_error)?;
        let mut summary: AccountSummary = json
            .map(|s| serde_json::from_str(&s).map_err(storage_error))
            .transpose()?
            .unwrap_or_else(|| empty_summary(enabled));
        summary.account_ref = active;
        summary.enabled = enabled;
        summary.next_allowed_at = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='account-next-allowed'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        let login_failed: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key='account-auth-error')",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if !enabled {
            summary.state = AccountState::Disabled;
        } else if login_failed {
            summary.state = AccountState::AuthRequired;
        } else if summary.state == AccountState::Disabled {
            summary.state = if summary.history_fetched_at.is_some() {
                AccountState::Cached
            } else {
                AccountState::NeverSynced
            };
        } else if summary.state == AccountState::Ready
            && summary
                .last_attempt_at
                .as_deref()
                .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
                .is_none_or(|time| Utc::now().signed_duration_since(time).num_seconds() >= 300)
        {
            summary.state = AccountState::Cached;
        }
        Ok(summary)
    }
    pub fn populate_account(&self, dashboard: &mut Dashboard) -> Result<(), ApiError> {
        let summary = self.account_summary()?;
        dashboard.runtime.network_enabled = summary.enabled;
        dashboard.providers[0].capabilities.account_ledger = true;
        if let Some(account) = &summary.account_ref {
            let mut statement = self.connection.prepare("SELECT row_json FROM server_daily_snapshots WHERE provider_id='codex' AND account_ref=?1 AND data_mode='observed' ORDER BY date_label,julianday(collected_at),row_id").map_err(storage_error)?;
            let rows = statement
                .query_map([account], |row| row.get::<_, String>(0))
                .map_err(storage_error)?;
            dashboard.account_rows = rows
                .map(|row| {
                    let row: LedgerRow = serde_json::from_str(&row.map_err(storage_error)?)
                        .map_err(storage_error)?;
                    validate_row(&row)?;
                    Ok(row)
                })
                .collect::<Result<_, ApiError>>()?;
            let json: Option<String> = self.connection.query_row("SELECT snapshot_json FROM quota_snapshots WHERE provider_id='codex' AND account_ref=?1 AND product_id='codex-account' AND json_extract(snapshot_json,'$.source.dataMode')='observed' ORDER BY julianday(collected_at) DESC,snapshot_id DESC LIMIT 1", [account], |row| row.get(0)).optional().map_err(storage_error)?;
            dashboard.quota = json
                .map(|s| serde_json::from_str(&s).map_err(storage_error))
                .transpose()?;
            if let Some(quota) = &mut dashboard.quota {
                let old = DateTime::parse_from_rfc3339(&quota.source.collected_at)
                    .map(|time| Utc::now().signed_duration_since(time).num_seconds() >= 300)
                    .unwrap_or(true);
                if !summary.enabled
                    || summary.quota_error.is_some()
                    || summary.history_error.is_some()
                    || old
                    || summary.state == AccountState::AuthRequired
                {
                    for window in &mut quota.windows {
                        if !window.reason_codes.contains(&ReasonCode::QuotaStale) {
                            window.reason_codes.push(ReasonCode::QuotaStale);
                        }
                    }
                }
            }
        }
        dashboard.account = Some(summary);
        Ok(())
    }
}
