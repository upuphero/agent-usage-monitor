use crate::{storage_error, Store};
use monitor_contracts::*;
use monitor_core::{
    devices::*,
    error,
    local::{aggregate, opaque_id, LocalEvent},
};
use rusqlite::{params, OptionalExtension};

impl Store {
    pub fn export_device_report(&self) -> Result<DeviceReport, ApiError> {
        let identity: String = self
            .connection
            .query_row(
                "SELECT value FROM app_meta WHERE key='installation-ref'",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        let meta = |key: &str| -> Result<Option<String>, ApiError> {
            self.connection
                .query_row("SELECT value FROM app_meta WHERE key=?1", [key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(storage_error)
        };
        let mut statement = self.connection.prepare("SELECT event_json FROM local_usage_events WHERE provider_id='codex' AND allocated=1 AND event_json IS NOT NULL ORDER BY event_key LIMIT 50001").map_err(storage_error)?;
        let records = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?;
        let mut events = Vec::new();
        for record in records {
            if events.len() == MAX_EVENTS {
                return Err(error(
                    ErrorCode::InvalidData,
                    "Device report exceeds record limit",
                ));
            }
            let event: LocalEvent =
                serde_json::from_str(&record.map_err(storage_error)?).map_err(storage_error)?;
            events.push(DeviceEvent {
                event_ref: event.key.clone(),
                at: event.at.clone(),
                scope: Scope {
                    provider_id: "codex".into(),
                    product_id: event.metadata.product().into(),
                    account_ref: event.metadata.account_ref.clone(),
                    identity_evidence: IdentityEvidence::Unknown,
                },
                tokens: aggregate(&[&event])?,
            });
        }
        let count = |sql| {
            self.connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(storage_error)
        };
        let mut reason_codes = vec![ReasonCode::CoverageIncomplete, ReasonCode::IdentityUnknown];
        if let Some(raw) = meta("local-sync-report")? {
            let report: SyncReport = serde_json::from_str(&raw).map_err(storage_error)?;
            for reason in report.reason_codes {
                monitor_core::local::add_reason(&mut reason_codes, reason);
            }
        }
        let report = DeviceReport {
            format: REPORT_FORMAT.into(),
            schema_version: REPORT_VERSION,
            device: DeviceIdentity {
                device_ref: opaque_id("device", &identity),
                platform: native_platform(),
            },
            generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            last_sync_at: [meta("local-last-sync")?, meta("local-last-observed-at")?]
                .into_iter()
                .flatten()
                .max(),
            files_indexed: count("SELECT count(*) FROM local_streams WHERE present=1")?,
            excluded_events: count("SELECT count(*) FROM local_usage_events WHERE allocated=0")?,
            reason_codes,
            events,
        };
        validate_report(&report)?;
        Ok(report)
    }
    pub fn device_usage(&self) -> Result<DeviceUsage, ApiError> {
        let mut reports = vec![self.export_device_report()?];
        let mut statement = self
            .connection
            .prepare("SELECT report_json FROM device_reports ORDER BY device_ref")
            .map_err(storage_error)?;
        for row in statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
        {
            reports.push(decode_report(row.map_err(storage_error)?.as_bytes())?);
        }
        merge_reports(&reports)
    }
    pub fn import_device_report(&mut self, bytes: &[u8]) -> Result<(), ApiError> {
        let report = decode_report(bytes)?;
        let own = self.export_device_report()?;
        if report.device.device_ref == own.device.device_ref {
            return Err(error(
                ErrorCode::InvalidData,
                "This device is collected locally; import another device",
            ));
        }
        let encoded = serde_json::to_string(&report).map_err(storage_error)?;
        let transaction = self.connection.transaction().map_err(storage_error)?;
        let mut reports = vec![own];
        let mut existing = None;
        {
            let mut statement = transaction
                .prepare("SELECT report_json FROM device_reports")
                .map_err(storage_error)?;
            for row in statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(storage_error)?
            {
                let text = row.map_err(storage_error)?;
                let cached = decode_report(text.as_bytes())?;
                if cached.device.device_ref == report.device.device_ref {
                    existing = Some((cached, text));
                } else {
                    reports.push(cached);
                }
            }
        }
        if let Some((previous, text)) = existing {
            let old_time = monitor_core::local::utc(&previous.generated_at);
            let new_time = monitor_core::local::utc(&report.generated_at);
            if previous.device.platform != report.device.platform
                || new_time < old_time
                || (new_time == old_time && text != encoded)
            {
                return Err(error(
                    ErrorCode::InvalidData,
                    "An older or conflicting device snapshot cannot replace this report",
                ));
            }
            if text == encoded {
                return Ok(());
            }
        }
        reports.push(report.clone());
        merge_reports(&reports)?; // Check all identities, totals and overflow before replacing anything.
        transaction.execute("INSERT INTO device_reports VALUES (?1,?2,?3) ON CONFLICT(device_ref) DO UPDATE SET generated_at=excluded.generated_at,report_json=excluded.report_json", params![report.device.device_ref, report.generated_at, encoded]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }
    pub fn remove_device_report(&self, device_ref: &str) -> Result<(), ApiError> {
        if !hashed_ref(device_ref, "device-") {
            return Err(error(ErrorCode::InvalidQuery, "Invalid device identity"));
        }
        self.connection
            .execute(
                "DELETE FROM device_reports WHERE device_ref=?1",
                [device_ref],
            )
            .map_err(storage_error)?;
        Ok(())
    }
}
