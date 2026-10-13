use crate::{
    collector::{Checkpoint, ParsedSource},
    insert_row, storage_error, Store, SCHEMA_VERSION,
};
use chrono::{Duration, NaiveDate, Utc};
use monitor_contracts::*;
use monitor_core::{
    error,
    local::{add_reason, aggregate, opaque_id, project_thread, LocalEvent, Snapshot},
};
use rusqlite::{params, OptionalExtension, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}
fn json<T: serde::Serialize>(value: &T) -> Result<String, ApiError> {
    serde_json::to_string(value).map_err(storage_error)
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, ApiError> {
    serde_json::from_str(value).map_err(storage_error)
}
type DayScope = (String, Option<String>, String);
type ModelKey = (Option<String>, String, Option<String>);
type SessionKey = (
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);
type ThreadProjection = (Vec<Snapshot>, bool, BTreeSet<String>);
const ACTIVE:&str="EXISTS(SELECT 1 FROM local_source_snapshots links WHERE links.event_key=r.event_key AND links.active=1)";

fn thread_snapshots(
    transaction: &Transaction<'_>,
    thread: &str,
) -> Result<ThreadProjection, ApiError> {
    let mut statement=transaction.prepare(&format!("SELECT snapshot_json,conflict,metadata_digest FROM local_raw_snapshots r WHERE thread_ref=?1 AND {ACTIVE}")).map_err(storage_error)?;
    let records = statement
        .query_map([thread], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, bool>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(storage_error)?;
    let mut snapshots = Vec::new();
    let mut conflict = false;
    let mut metadata = BTreeSet::new();
    let mut ownership = BTreeSet::new();
    for record in records {
        let (text, issue, meta) = record.map_err(storage_error)?;
        let snapshot: Snapshot = decode(&text)?;
        if monitor_core::local::utc(&snapshot.at).is_none() {
            return Err(error(ErrorCode::Storage, "Invalid cached observation time"));
        }
        conflict |= issue;
        metadata.insert(meta);
        let eligible="json_extract(snapshot_json,'$.metadata.imported')=0 AND (json_extract(snapshot_json,'$.metadata.created_at') IS NULL OR at_utc>=json_extract(snapshot_json,'$.metadata.created_at'))";
        let owners:i64=transaction.query_row(&format!("SELECT count(DISTINCT thread_ref) FROM local_raw_snapshots r WHERE (ownership_key=?1 OR (?2 IS NOT NULL AND response_ref=?2)) AND {eligible} AND {ACTIVE}"),params![snapshot.ownership_key,snapshot.response_ref],|row|row.get(0)).map_err(storage_error)?;
        if owners > 1 {
            ownership.insert(snapshot.ownership_key.clone());
        }
        snapshots.push(snapshot);
    }
    Ok((snapshots, conflict || metadata.len() > 1, ownership))
}
fn load_events(
    transaction: &Transaction<'_>,
    scope: &DayScope,
) -> Result<Vec<LocalEvent>, ApiError> {
    let mut statement=transaction.prepare("SELECT event_json FROM local_usage_events WHERE provider_id='codex' AND product_id=?1 AND account_ref IS ?2 AND substr(ts_utc,1,10)=?3 AND event_json IS NOT NULL").map_err(storage_error)?;
    let records = statement
        .query_map(params![scope.0, scope.1, scope.2], |row| {
            row.get::<_, String>(0)
        })
        .map_err(storage_error)?;
    records
        .map(|row| decode(&row.map_err(storage_error)?))
        .collect()
}
fn refresh_day(transaction: &Transaction<'_>, scope: &DayScope) -> Result<(), ApiError> {
    let events = load_events(transaction, scope)?;
    let row_id = opaque_id("local-day", &json(scope)?);
    transaction
        .execute(
            "DELETE FROM local_daily WHERE row_id=?1 AND data_mode='observed'",
            [&row_id],
        )
        .map_err(storage_error)?;
    if events.is_empty() {
        return Ok(());
    }
    let mut reasons = Vec::new();
    for event in &events {
        for reason in &event.reason_codes {
            add_reason(&mut reasons, *reason);
        }
    }
    let tokens = aggregate(&events.iter().collect::<Vec<_>>())?;
    if tokens.cached_input.value.is_none() || tokens.uncached_input.value.is_none() {
        add_reason(&mut reasons, ReasonCode::MissingComponent);
    }
    let date = NaiveDate::parse_from_str(&scope.2, "%Y-%m-%d").map_err(storage_error)?;
    let end = date.checked_add_signed(Duration::days(1)).ok_or_else(|| {
        error(
            ErrorCode::InvalidData,
            "Local day is outside supported range",
        )
    })?;
    let row = LedgerRow {
        row_id,
        ledger: LedgerKind::Windows,
        scope: Scope {
            provider_id: "codex".into(),
            product_id: scope.0.clone(),
            account_ref: scope.1.clone(),
            identity_evidence: IdentityEvidence::Unknown,
        },
        day: DayBucket {
            date_label: scope.2.clone(),
            basis: TimeBasis::VerifiedUtcDay,
            timezone: Some("UTC".into()),
            start_utc: Some(format!("{}T00:00:00Z", scope.2)),
            end_utc: Some(format!("{end}T00:00:00Z")),
        },
        source: Source {
            source_id: "local-event-ledger".into(),
            kind: SourceKind::LocalLog,
            collected_at: now(),
            data_mode: DataMode::Observed,
        },
        quality: Quality {
            status: if tokens.text_total.value.is_none() {
                QualityStatus::Unknown
            } else {
                QualityStatus::Partial
            },
            coverage: CoverageStatus::Partial,
            freshness: FreshnessStatus::Unknown,
            reason_codes: reasons,
        },
        tokens,
    };
    insert_row(transaction, &row)
}
fn rebuild_threads(
    transaction: &Transaction<'_>,
    threads: &BTreeSet<String>,
) -> Result<(), ApiError> {
    let mut dates = BTreeSet::new();
    for thread in threads {
        {
            let mut statement=transaction.prepare("SELECT DISTINCT product_id,account_ref,substr(ts_utc,1,10) FROM local_usage_events WHERE provider_id='codex' AND thread_ref=?1").map_err(storage_error)?;
            let old = statement
                .query_map([thread], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(storage_error)?;
            for scope in old {
                dates.insert(scope.map_err(storage_error)?);
            }
        }
        transaction
            .execute(
                "DELETE FROM local_usage_events WHERE provider_id='codex' AND thread_ref=?1",
                [thread],
            )
            .map_err(storage_error)?;
        let (snapshots, conflict, ownership) = thread_snapshots(transaction, thread)?;
        for (ordinal, event) in project_thread(snapshots, &ownership, conflict)
            .into_iter()
            .enumerate()
        {
            let metadata = &event.metadata;
            if let Some(account) = &metadata.account_ref {
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO accounts VALUES ('codex',?1,'unknown')",
                        [account],
                    )
                    .map_err(storage_error)?;
            }
            dates.insert((
                metadata.product().to_string(),
                metadata.account_ref.clone(),
                event.date.clone(),
            ));
            transaction.execute("INSERT INTO local_usage_events(provider_id,event_key,product_id,account_ref,installation_ref,source_id,source_offset,thread_ref,ts_utc,model_id,effort,speed,uncached_input,cached_input,output,quality_json,event_json,allocated) VALUES ('codex',?1,?2,?3,(SELECT value FROM app_meta WHERE key='installation-ref'),?4,?5,?4,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![
                event.key,metadata.product(),metadata.account_ref,metadata.thread_ref,i64::try_from(ordinal).map_err(storage_error)?,event.at,event.model_id,event.effort,event.speed,
                event.counts.input.zip(event.counts.cached).map(|(i,c)|i-c),event.counts.cached,event.counts.output,json(&event.reason_codes)?,json(&event)?,event.allocated]).map_err(storage_error)?;
        }
    }
    for scope in dates {
        refresh_day(transaction, &scope)?;
    }
    Ok(())
}

impl Store {
    pub(crate) fn checkpoint(&self, source: &str) -> Result<Option<Checkpoint>, ApiError> {
        self.connection.query_row("SELECT source_id,root_ref,parser_version,byte_offset,file_size,modified_tag,prefix_digest,head_digest,tail_digest,state_json FROM local_streams WHERE source_id=?1",[source],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,u64>(3)?,row.get::<_,u64>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,String>(9)?)))
            .optional().map_err(storage_error)?.map(|(source_id,root_ref,parser_version,offset,size,modified,digest,head,tail,state)|Ok(Checkpoint {source_id,root_ref,parser_version,offset,size,modified,digest,head,tail,state:decode(&state)?})).transpose()
    }
    pub(crate) fn commit_source(&mut self, parsed: ParsedSource) -> Result<(u32, u32), ApiError> {
        let cp = parsed.checkpoint;
        let transaction = self.connection.transaction().map_err(storage_error)?;
        let mut threads = BTreeSet::new();
        let mut new = 0u32;
        let mut duplicate = 0u32;
        let old_generation: Option<i64> = transaction
            .query_row(
                "SELECT generation FROM local_streams WHERE source_id=?1",
                [&cp.source_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        let generation = old_generation.unwrap_or(1) + i64::from(parsed.rebuilt);
        if parsed.rebuilt {
            let mut statement=transaction.prepare("SELECT DISTINCT thread_ref FROM local_raw_snapshots r JOIN local_source_snapshots links ON links.event_key=r.event_key WHERE links.source_id=?1 AND links.active=1").map_err(storage_error)?;
            for thread in statement
                .query_map([&cp.source_id], |row| row.get::<_, String>(0))
                .map_err(storage_error)?
            {
                threads.insert(thread.map_err(storage_error)?);
            }
            drop(statement);
            transaction
                .execute(
                    "UPDATE local_source_snapshots SET active=0 WHERE source_id=?1",
                    [&cp.source_id],
                )
                .map_err(storage_error)?;
        }
        transaction.execute("INSERT INTO local_streams(source_id,root_ref,parser_version,byte_offset,file_size,modified_tag,prefix_digest,head_digest,tail_digest,state_json,present,issues_json,generation,alias_of) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11,?12,NULL) ON CONFLICT(source_id) DO UPDATE SET root_ref=excluded.root_ref,parser_version=excluded.parser_version,byte_offset=excluded.byte_offset,file_size=excluded.file_size,modified_tag=excluded.modified_tag,prefix_digest=excluded.prefix_digest,head_digest=excluded.head_digest,tail_digest=excluded.tail_digest,state_json=excluded.state_json,present=1,issues_json=excluded.issues_json,generation=excluded.generation,alias_of=NULL",params![cp.source_id,cp.root_ref,cp.parser_version,i64::try_from(cp.offset).map_err(storage_error)?,i64::try_from(cp.size).map_err(storage_error)?,cp.modified,cp.digest,cp.head,cp.tail,json(&cp.state)?,json(&cp.state.reason_codes)?,generation]).map_err(storage_error)?;
        for snapshot in parsed.snapshots {
            threads.insert(snapshot.metadata.thread_ref.clone());
            let text = json(&snapshot)?;
            let metadata_digest = opaque_id("metadata", &json(&snapshot.metadata)?);
            let old: Option<String> = transaction
                .query_row(
                    "SELECT snapshot_json FROM local_raw_snapshots WHERE event_key=?1",
                    [&snapshot.key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(storage_error)?;
            match old {
                None => {
                    transaction
                        .execute(
                            "INSERT INTO local_raw_snapshots VALUES (?1,?2,?3,?4,?5,?6,?7,0)",
                            params![
                                snapshot.key,
                                snapshot.metadata.thread_ref,
                                snapshot.ownership_key,
                                metadata_digest,
                                snapshot.response_ref,
                                snapshot.at,
                                text
                            ],
                        )
                        .map_err(storage_error)?;
                    new += 1;
                }
                Some(old) => {
                    duplicate += 1;
                    if old != text {
                        let active:i64=transaction.query_row("SELECT count(*) FROM local_source_snapshots WHERE event_key=?1 AND active=1",[&snapshot.key],|row|row.get(0)).map_err(storage_error)?;
                        if active == 0 {
                            transaction.execute("UPDATE local_raw_snapshots SET metadata_digest=?2,snapshot_json=?3,conflict=0 WHERE event_key=?1",params![snapshot.key,metadata_digest,text]).map_err(storage_error)?;
                        } else {
                            transaction
                                .execute(
                                    "UPDATE local_raw_snapshots SET conflict=1 WHERE event_key=?1",
                                    [&snapshot.key],
                                )
                                .map_err(storage_error)?;
                        }
                    }
                }
            }
            transaction.execute("INSERT INTO local_source_snapshots VALUES (?1,?2,1,?3) ON CONFLICT(source_id,event_key) DO UPDATE SET active=1,generation=excluded.generation",params![cp.source_id,snapshot.key,generation]).map_err(storage_error)?;
            let mut owners=transaction.prepare("SELECT DISTINCT thread_ref FROM local_raw_snapshots WHERE ownership_key=?1 OR (?2 IS NOT NULL AND response_ref=?2)").map_err(storage_error)?;
            for thread in owners
                .query_map(
                    params![snapshot.ownership_key, snapshot.response_ref],
                    |row| row.get::<_, String>(0),
                )
                .map_err(storage_error)?
            {
                threads.insert(thread.map_err(storage_error)?);
            }
        }
        if let Some(account) = cp
            .state
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.account_ref.as_ref())
        {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO accounts VALUES ('codex',?1,'unknown')",
                    [account],
                )
                .map_err(storage_error)?;
        }
        transaction.execute("INSERT INTO source_checkpoints VALUES ('codex',?1,?2,?3,?4,?5,?6) ON CONFLICT(provider_id,source_id) DO UPDATE SET account_ref=excluded.account_ref,file_identity=excluded.file_identity,byte_offset=excluded.byte_offset,parser_version=excluded.parser_version,updated_at=excluded.updated_at",params![cp.source_id,cp.state.metadata.as_ref().and_then(|metadata|metadata.account_ref.as_ref()),cp.digest,i64::try_from(cp.offset).map_err(storage_error)?,cp.parser_version,now()]).map_err(storage_error)?;
        rebuild_threads(&transaction, &threads)?;
        transaction.commit().map_err(storage_error)?;
        Ok((new, duplicate))
    }
    pub(crate) fn mark_present(&mut self, source: &str) -> Result<(), ApiError> {
        let transaction = self.connection.transaction().map_err(storage_error)?;
        transaction
            .execute(
                "UPDATE local_streams SET present=1,alias_of=NULL WHERE source_id=?1",
                [source],
            )
            .map_err(storage_error)?;
        transaction.execute("UPDATE local_source_snapshots SET active=1 WHERE source_id=?1 AND generation=(SELECT generation FROM local_streams WHERE source_id=?1)",[source]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }
    pub(crate) fn mark_missing(&mut self, present: &BTreeSet<String>) -> Result<(), ApiError> {
        let mut statement = self
            .connection
            .prepare("SELECT source_id FROM local_streams")
            .map_err(storage_error)?;
        let sources: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .map_err(storage_error)?
            .collect::<Result<_, _>>()
            .map_err(storage_error)?;
        drop(statement);
        for source in sources {
            if present.contains(&source) {
                continue;
            }
            let alias:Option<String>=self.connection.query_row("SELECT other.source_id FROM local_streams old JOIN local_streams other ON old.prefix_digest=other.prefix_digest AND old.byte_offset=other.byte_offset AND old.parser_version=other.parser_version WHERE old.source_id=?1 AND other.source_id!=?1 AND other.present=1 LIMIT 1",[&source],|row|row.get(0)).optional().map_err(storage_error)?;
            self.connection
                .execute(
                    "UPDATE local_streams SET present=0,alias_of=?2 WHERE source_id=?1",
                    params![source, alias],
                )
                .map_err(storage_error)?;
            // Retain historical events when a source disappears. Aliases share the same event keys.
        }
        Ok(())
    }
    pub(crate) fn record_sync_state(
        &self,
        state: SyncState,
        report: Option<&SyncReport>,
    ) -> Result<(), ApiError> {
        self.connection.execute("INSERT INTO app_meta VALUES ('local-sync-state',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json(&state)?]).map_err(storage_error)?;
        if let Some(report) = report {
            self.connection.execute("INSERT INTO app_meta VALUES ('local-sync-report',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json(report)?]).map_err(storage_error)?;
        }
        Ok(())
    }
    pub(crate) fn finish_sync(&self, report: &SyncReport) -> Result<(), ApiError> {
        self.record_sync_state(SyncState::Idle, Some(report))?;
        self.connection.execute("INSERT INTO app_meta VALUES ('local-last-sync',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[now()]).map_err(storage_error)?;
        Ok(())
    }
    fn meta(&self, key: &str) -> Result<Option<String>, ApiError> {
        self.connection
            .query_row("SELECT value FROM app_meta WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(storage_error)
    }
    pub fn additional_roots(&self) -> Result<Vec<PathBuf>, ApiError> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='local-additional-roots'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        value
            .map(|value| {
                decode::<Vec<String>>(&value)
                    .map(|roots| roots.into_iter().map(PathBuf::from).collect())
            })
            .unwrap_or_else(|| Ok(Vec::new()))
    }
    pub fn set_additional_roots(&self, roots: Vec<String>) -> Result<(), ApiError> {
        if roots.len() > 16 {
            return Err(error(
                ErrorCode::InvalidDirectory,
                "Too many additional roots",
            ));
        }
        let mut validated = Vec::new();
        for root in roots {
            let path = Path::new(&root);
            if !path.is_absolute() || !path.is_dir() {
                return Err(error(
                    ErrorCode::InvalidDirectory,
                    "A root must be an existing absolute directory",
                ));
            }
            validated.push(
                path.canonicalize()
                    .map_err(storage_error)?
                    .to_string_lossy()
                    .to_string(),
            );
        }
        validated.sort();
        validated.dedup();
        self.connection.execute("INSERT INTO app_settings VALUES ('local-additional-roots',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json(&validated)?]).map_err(storage_error)?;
        Ok(())
    }
    pub fn local_dashboard(&self) -> Result<Dashboard, ApiError> {
        let mut rows_statement=self.connection.prepare("SELECT row_json FROM local_daily WHERE provider_id='codex' AND data_mode='observed' ORDER BY date_label,account_ref,product_id").map_err(storage_error)?;
        let local_rows = rows_statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| {
                let row: LedgerRow = decode(&row.map_err(storage_error)?)?;
                monitor_core::validate_row(&row)?;
                Ok(row)
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        let mut events_statement=self.connection.prepare("SELECT event_json FROM local_usage_events WHERE provider_id='codex' AND event_json IS NOT NULL ORDER BY ts_utc DESC,event_key").map_err(storage_error)?;
        let events = events_statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| decode::<LocalEvent>(&row.map_err(storage_error)?))
            .collect::<Result<Vec<_>, ApiError>>()?;
        let mut model_groups: BTreeMap<ModelKey, Vec<&LocalEvent>> = BTreeMap::new();
        let mut session_groups: BTreeMap<SessionKey, Vec<&LocalEvent>> = BTreeMap::new();
        for event in &events {
            let product = event.metadata.product().to_string();
            let account = event.metadata.account_ref.clone();
            model_groups
                .entry((account.clone(), product.clone(), event.model_id.clone()))
                .or_default()
                .push(event);
            session_groups
                .entry((
                    event.metadata.thread_ref.clone(),
                    account,
                    product,
                    event.model_id.clone(),
                    event.effort.clone(),
                    event.speed.clone(),
                ))
                .or_default()
                .push(event);
        }
        let scope = |account: Option<String>, product: String| Scope {
            provider_id: "codex".into(),
            product_id: product,
            account_ref: account,
            identity_evidence: IdentityEvidence::Unknown,
        };
        let mut models = Vec::new();
        for ((account, product, model), group) in model_groups {
            models.push(LocalModel {
                model_id: model.unwrap_or_else(|| "unknown".into()),
                scope: scope(account, product),
                text_total: aggregate(&group)?.text_total,
                sessions: group
                    .iter()
                    .map(|event| &event.metadata.thread_ref)
                    .collect::<BTreeSet<_>>()
                    .len()
                    .try_into()
                    .map_err(storage_error)?,
            });
        }
        let mut sessions = Vec::new();
        for ((thread, account, product, model, effort, speed), group) in session_groups {
            sessions.push(LocalSession {
                session_ref: opaque_id(
                    "session-group",
                    &json(&(&thread, &model, &effort, &speed))?,
                ),
                model_id: model,
                effort,
                speed,
                scope: scope(account, product),
                text_total: aggregate(&group)?.text_total,
            });
        }
        let mut reasons = vec![
            ReasonCode::IdentityUnknown,
            ReasonCode::CoverageIncomplete,
            ReasonCode::ProductScopeUnverified,
            ReasonCode::AccountLedgerUnavailable,
            ReasonCode::CalibrationNotRun,
        ];
        let mut issues = self
            .connection
            .prepare("SELECT issues_json FROM local_streams")
            .map_err(storage_error)?;
        for row in issues
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
        {
            for reason in decode::<Vec<ReasonCode>>(&row.map_err(storage_error)?)? {
                add_reason(&mut reasons, reason);
            }
        }
        for event in &events {
            for reason in &event.reason_codes {
                add_reason(&mut reasons, *reason);
            }
        }
        let count = |sql: &str| {
            self.connection
                .query_row(sql, [], |row| row.get::<_, u32>(0))
                .map_err(storage_error)
        };
        let retained =
            count("SELECT count(*) FROM local_streams WHERE present=0 AND alias_of IS NULL")?;
        if retained > 0 {
            add_reason(&mut reasons, ReasonCode::SourceMissing);
        }
        let state = self
            .meta("local-sync-state")?
            .map(|value| decode(&value))
            .transpose()?
            .unwrap_or(SyncState::NeverSynced);
        let last_run = self
            .meta("local-sync-report")?
            .map(|value| decode::<SyncReport>(&value))
            .transpose()?;
        if let Some(report) = &last_run {
            for reason in &report.reason_codes {
                add_reason(&mut reasons, *reason);
            }
        }
        let collector = CollectorSummary {
            state,
            last_sync_at: self.meta("local-last-sync")?,
            files_indexed: count("SELECT count(*) FROM local_streams WHERE present=1")?,
            retained_sources: retained,
            snapshots: count(&format!(
                "SELECT count(*) FROM local_raw_snapshots r WHERE {ACTIVE}"
            ))?,
            accepted_events: count("SELECT count(*) FROM local_usage_events WHERE allocated=1")?,
            ambiguous_events: count("SELECT count(*) FROM local_usage_events WHERE allocated=0")?,
            sessions_total: events
                .iter()
                .map(|event| &event.metadata.thread_ref)
                .collect::<BTreeSet<_>>()
                .len()
                .try_into()
                .map_err(storage_error)?,
            reason_codes: reasons,
            last_run,
        };
        let has_data = !local_rows.is_empty();
        Ok(Dashboard {
            api_version: API_VERSION.into(),
            data_mode: DataMode::Observed,
            display_timezone: "America/Phoenix".into(),
            providers: vec![
                Provider {
                    provider_id: "codex".into(),
                    display_name: "Codex".into(),
                    state: if has_data {
                        ProviderState::LocalPartial
                    } else {
                        ProviderState::NotDetected
                    },
                    capabilities: ProviderCapabilities {
                        local_ledger: true,
                        account_ledger: false,
                        incremental_collection: true,
                        calibrated_estimate: false,
                    },
                },
                Provider {
                    provider_id: "claude-code".into(),
                    display_name: "Claude Code".into(),
                    state: ProviderState::Planned,
                    capabilities: ProviderCapabilities {
                        local_ledger: false,
                        account_ledger: false,
                        incremental_collection: false,
                        calibrated_estimate: false,
                    },
                },
                Provider {
                    provider_id: "antigravity".into(),
                    display_name: "Antigravity".into(),
                    state: ProviderState::Planned,
                    capabilities: ProviderCapabilities {
                        local_ledger: false,
                        account_ledger: false,
                        incremental_collection: false,
                        calibrated_estimate: false,
                    },
                },
            ],
            local_rows,
            account_rows: Vec::new(),
            comparisons: Vec::new(),
            quota: None,
            models,
            sessions,
            runtime: RuntimeInfo {
                app_version: env!("CARGO_PKG_VERSION").into(),
                schema_version: SCHEMA_VERSION,
                storage: "monitor.sqlite3".into(),
                installation_ref: self
                    .meta("installation-ref")?
                    .ok_or_else(|| error(ErrorCode::Storage, "Installation identity missing"))?,
                network_enabled: false,
                local_collection_enabled: true,
            },
            collector: Some(collector),
            account: None,
        })
    }
}
