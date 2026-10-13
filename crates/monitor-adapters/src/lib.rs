use monitor_contracts::*;
use monitor_core::{demo, error, parse_count, rebuild_comparisons, validate_row};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;
const MIGRATION: &str = include_str!("../migrations/001_dual_ledger.sql");
pub struct Store {
    connection: Connection,
}

fn storage_error(_: impl std::fmt::Display) -> ApiError {
    // Never pass filesystem paths, SQL payloads or exception bodies to IPC.
    error(ErrorCode::Storage, "Local database operation failed")
}

fn migrate(connection: &mut Connection, migration: &str) -> Result<(), ApiError> {
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(storage_error)?;
    if version > SCHEMA_VERSION {
        return Err(error(
            ErrorCode::StorageSchemaNewer,
            "Database requires a newer application version",
        ));
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute_batch(migration)
        .map_err(storage_error)?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)
}

fn insert_row(transaction: &Transaction<'_>, row: &LedgerRow) -> Result<(), ApiError> {
    validate_row(row)?;
    if let Some(account) = &row.scope.account_ref {
        let identity = match row.scope.identity_evidence {
            IdentityEvidence::Verified => "verified",
            IdentityEvidence::Unknown => "unknown",
        };
        transaction
            .execute(
                "INSERT OR IGNORE INTO accounts VALUES (?1, ?2, ?3)",
                params![row.scope.provider_id, account, identity],
            )
            .map_err(storage_error)?;
    }
    let table = match row.ledger {
        LedgerKind::Account => "server_daily_snapshots",
        LedgerKind::Windows => "local_daily",
    };
    let number = |metric: &Metric<String>| -> Result<Option<i64>, ApiError> {
        metric.value.as_deref().map(parse_count).transpose()
    };
    let json = serde_json::to_string(row).map_err(storage_error)?;
    // Table name comes exclusively from the enum above; all data is parameterized.
    let sql =
        format!("INSERT INTO {table} VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)");
    transaction
        .execute(
            &sql,
            params![
                row.row_id,
                row.scope.provider_id,
                row.scope.product_id,
                row.scope.account_ref,
                row.day.date_label,
                row.source.collected_at,
                match row.source.data_mode {
                    DataMode::Synthetic => "synthetic",
                    DataMode::Observed => "observed",
                },
                number(&row.tokens.uncached_input)?,
                number(&row.tokens.cached_input)?,
                number(&row.tokens.inclusive_input)?,
                number(&row.tokens.output)?,
                number(&row.tokens.reasoning_output)?,
                number(&row.tokens.text_total)?,
                json
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, ApiError> {
        let connection = Connection::open(path).map_err(storage_error)?;
        Self::initialize(connection)
    }
    fn initialize(mut connection: Connection) -> Result<Self, ApiError> {
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(storage_error)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(3))
            .map_err(storage_error)?;
        migrate(&mut connection, MIGRATION)?;
        Ok(Self { connection })
    }
    pub fn seed_synthetic(&mut self) -> Result<(), ApiError> {
        let seeded: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM app_meta WHERE key='p0-fixture-version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        if seeded.is_some() {
            return Ok(());
        }
        let fixture = demo(DemoScenario::Ready)?;
        let transaction = self.connection.transaction().map_err(storage_error)?;
        for row in fixture.account_rows.iter().chain(fixture.local_rows.iter()) {
            insert_row(&transaction, row)?;
        }
        if let Some(quota) = &fixture.quota {
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
        transaction
            .execute("INSERT INTO app_meta VALUES ('p0-fixture-version','1')", [])
            .map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO app_meta VALUES ('installation-ref',?1)",
                [uuid::Uuid::new_v4().to_string()],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO app_settings VALUES ('account-query-enabled','false')",
                [],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }
    pub fn append_rows(&mut self, rows: &[LedgerRow]) -> Result<(), ApiError> {
        let transaction = self.connection.transaction().map_err(storage_error)?;
        for row in rows {
            insert_row(&transaction, row)?;
        }
        transaction.commit().map_err(storage_error)
    }
    pub fn rows(
        &self,
        ledger: LedgerKind,
        provider_id: &str,
        account_ref: &str,
        product_id: &str,
    ) -> Result<Vec<LedgerRow>, ApiError> {
        let table = match ledger {
            LedgerKind::Account => "server_daily_snapshots",
            LedgerKind::Windows => "local_daily",
        };
        let sql = format!("SELECT row_json FROM {table} WHERE provider_id=?1 AND account_ref=?2 AND product_id=?3 AND data_mode='synthetic' ORDER BY date_label, julianday(collected_at), row_id");
        let mut statement = self.connection.prepare(&sql).map_err(storage_error)?;
        let rows = statement
            .query_map(params![provider_id, account_ref, product_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(storage_error)?;
        rows.map(|row| {
            let row: LedgerRow =
                serde_json::from_str(&row.map_err(storage_error)?).map_err(storage_error)?;
            validate_row(&row)?;
            Ok(row)
        })
        .collect()
    }
    pub fn dashboard(&self, scenario: DemoScenario) -> Result<Dashboard, ApiError> {
        let mut dashboard = demo(scenario)?;
        if scenario == DemoScenario::Ready {
            dashboard.account_rows = self.rows(
                LedgerKind::Account,
                "codex",
                "demo-account-a",
                "codex-desktop",
            )?;
            dashboard.local_rows = self.rows(
                LedgerKind::Windows,
                "codex",
                "demo-account-a",
                "codex-desktop",
            )?;
            let quota: Option<String> = self.connection.query_row("SELECT snapshot_json FROM quota_snapshots WHERE provider_id='codex' AND account_ref='demo-account-a' AND product_id='codex-desktop' ORDER BY julianday(collected_at) DESC LIMIT 1", [], |row| row.get(0)).optional().map_err(storage_error)?;
            dashboard.quota = quota
                .map(|json| serde_json::from_str(&json).map_err(storage_error))
                .transpose()?;
            rebuild_comparisons(&mut dashboard);
        }
        dashboard.runtime.storage = "monitor.sqlite3".into();
        dashboard.runtime.installation_ref = self
            .connection
            .query_row(
                "SELECT value FROM app_meta WHERE key='installation-ref'",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        dashboard.runtime.schema_version = SCHEMA_VERSION;
        Ok(dashboard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> Store {
        let mut store = Store::initialize(Connection::open_in_memory().unwrap()).unwrap();
        store.seed_synthetic().unwrap();
        store
    }
    #[test]
    fn fresh_migration_reopen_and_seed_are_idempotent() {
        let mut store = store();
        store.seed_synthetic().unwrap();
        migrate(&mut store.connection, MIGRATION).unwrap();
        assert_eq!(
            store
                .rows(
                    LedgerKind::Account,
                    "codex",
                    "demo-account-a",
                    "codex-desktop"
                )
                .unwrap()
                .len(),
            8
        );
        let version: u32 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM calibration_runs", [], |row| row
                    .get::<_, i32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn migration_failure_rolls_back_and_newer_schema_is_preserved() {
        let mut connection = Connection::open_in_memory().unwrap();
        assert!(migrate(
            &mut connection,
            "CREATE TABLE temporary_marker(id INTEGER); INVALID SQL;"
        )
        .is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='temporary_marker'",
                    [],
                    |row| row.get::<_, i32>(0)
                )
                .unwrap(),
            0
        );
        connection.pragma_update(None, "user_version", 2).unwrap();
        assert_eq!(
            migrate(&mut connection, MIGRATION).unwrap_err().code,
            ErrorCode::StorageSchemaNewer
        );
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn append_preserves_snapshots_and_duplicate_batch_is_atomic() {
        let mut store = store();
        let original = store
            .rows(
                LedgerKind::Account,
                "codex",
                "demo-account-a",
                "codex-desktop",
            )
            .unwrap();
        let mut new = original[0].clone();
        new.row_id = "new-snapshot".into();
        new.source.collected_at = "2026-10-13T00:00:00Z".into();
        store.append_rows(&[new.clone()]).unwrap();
        assert_eq!(
            store
                .rows(
                    LedgerKind::Account,
                    "codex",
                    "demo-account-a",
                    "codex-desktop"
                )
                .unwrap()
                .len(),
            9
        );
        assert!(store
            .connection
            .execute(
                "UPDATE server_daily_snapshots SET collected_at='changed'",
                []
            )
            .is_err());
        let mut another = new.clone();
        another.row_id = "should-rollback".into();
        assert!(store.append_rows(&[another, new]).is_err());
        assert_eq!(
            store
                .rows(
                    LedgerKind::Account,
                    "codex",
                    "demo-account-a",
                    "codex-desktop"
                )
                .unwrap()
                .len(),
            9
        );
    }
    #[test]
    fn provider_account_product_and_synthetic_scope_isolated() {
        let mut store = store();
        let mut row = demo(DemoScenario::Ready).unwrap().account_rows[0].clone();
        row.row_id = "other-account".into();
        row.scope.account_ref = Some("demo-account-b".into());
        store.append_rows(&[row.clone()]).unwrap();
        row.row_id = "other-provider".into();
        row.scope.provider_id = "future-provider".into();
        store.append_rows(&[row.clone()]).unwrap();
        row.row_id = "other-product".into();
        row.scope.product_id = "all-products".into();
        store.append_rows(&[row.clone()]).unwrap();
        row.row_id = "observed-separate".into();
        row.source.data_mode = DataMode::Observed;
        store.append_rows(&[row]).unwrap();
        assert_eq!(
            store
                .rows(
                    LedgerKind::Account,
                    "codex",
                    "demo-account-a",
                    "codex-desktop"
                )
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            store
                .rows(
                    LedgerKind::Account,
                    "codex",
                    "demo-account-b",
                    "codex-desktop"
                )
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn database_constraints_and_whitelist_contract() {
        let mut store = store();
        let mut row = demo(DemoScenario::Ready).unwrap().account_rows[0].clone();
        row.row_id = "invalid".into();
        row.tokens.cached_input.value = Some("-1".into());
        assert!(store.append_rows(&[row]).is_err());
        let mut json =
            serde_json::to_value(demo(DemoScenario::Ready).unwrap().account_rows[0].clone())
                .unwrap();
        json["authorization"] = "forbidden".into();
        assert!(serde_json::from_value::<LedgerRow>(json).is_err());
        assert!(store
            .connection
            .execute("UPDATE local_daily SET cached_input=-1", [])
            .is_err());
    }
    #[test]
    fn native_cached_dashboard_matches_browser_contract() {
        let store = store();
        let dashboard = store.dashboard(DemoScenario::Ready).unwrap();
        assert_eq!(
            dashboard.comparisons,
            demo(DemoScenario::Ready).unwrap().comparisons
        );
        assert_eq!(dashboard.runtime.storage, "monitor.sqlite3");
        assert!(!dashboard.runtime.network_enabled && !dashboard.runtime.local_collection_enabled);
    }
}
