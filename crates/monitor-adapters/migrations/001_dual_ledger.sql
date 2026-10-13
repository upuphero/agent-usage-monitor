CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL);
CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE accounts (
    provider_id TEXT NOT NULL, account_ref TEXT NOT NULL,
    identity_evidence TEXT NOT NULL CHECK (identity_evidence IN ('verified','unknown')),
    PRIMARY KEY (provider_id, account_ref)
);
CREATE TABLE source_checkpoints (
    provider_id TEXT NOT NULL, source_id TEXT NOT NULL, account_ref TEXT,
    file_identity TEXT NOT NULL, byte_offset INTEGER NOT NULL CHECK (byte_offset >= 0),
    parser_version TEXT NOT NULL, updated_at TEXT NOT NULL,
    PRIMARY KEY (provider_id, source_id),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE TABLE local_usage_events (
    provider_id TEXT NOT NULL, event_key TEXT NOT NULL, product_id TEXT NOT NULL,
    account_ref TEXT, installation_ref TEXT NOT NULL, source_id TEXT NOT NULL,
    source_offset INTEGER NOT NULL CHECK (source_offset >= 0), thread_ref TEXT NOT NULL,
    ts_utc TEXT NOT NULL, model_id TEXT, effort TEXT, speed TEXT,
    uncached_input INTEGER CHECK (uncached_input >= 0), cached_input INTEGER CHECK (cached_input >= 0),
    output INTEGER CHECK (output >= 0), quality_json TEXT NOT NULL CHECK (json_valid(quality_json)),
    PRIMARY KEY (provider_id, event_key),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE INDEX local_events_thread_time ON local_usage_events(provider_id, thread_ref, ts_utc);
CREATE INDEX local_events_source_offset ON local_usage_events(provider_id, source_id, source_offset);
CREATE TABLE local_daily (
    row_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT,
    date_label TEXT NOT NULL, collected_at TEXT NOT NULL,
    data_mode TEXT NOT NULL CHECK (data_mode IN ('synthetic','observed')),
    uncached_input INTEGER CHECK (uncached_input >= 0), cached_input INTEGER CHECK (cached_input >= 0),
    inclusive_input INTEGER CHECK (inclusive_input >= 0), output INTEGER CHECK (output >= 0),
    reasoning_output INTEGER CHECK (reasoning_output >= 0 AND reasoning_output <= output),
    text_total INTEGER CHECK (text_total >= 0), row_json TEXT NOT NULL CHECK (json_valid(row_json)),
    CHECK ((inclusive_input IS NULL AND (uncached_input IS NULL OR cached_input IS NULL)) OR
           (inclusive_input IS NOT NULL AND uncached_input IS NOT NULL AND cached_input IS NOT NULL AND inclusive_input = uncached_input + cached_input)),
    CHECK ((text_total IS NULL AND (inclusive_input IS NULL OR output IS NULL)) OR
           (text_total IS NOT NULL AND inclusive_input IS NOT NULL AND output IS NOT NULL AND text_total = inclusive_input + output)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE INDEX local_daily_account_date ON local_daily(provider_id, account_ref, product_id, date_label);
CREATE TABLE server_daily_snapshots (
    row_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT,
    date_label TEXT NOT NULL, collected_at TEXT NOT NULL,
    data_mode TEXT NOT NULL CHECK (data_mode IN ('synthetic','observed')),
    uncached_input INTEGER CHECK (uncached_input >= 0), cached_input INTEGER CHECK (cached_input >= 0),
    inclusive_input INTEGER CHECK (inclusive_input >= 0), output INTEGER CHECK (output >= 0),
    reasoning_output INTEGER CHECK (reasoning_output >= 0 AND reasoning_output <= output),
    text_total INTEGER CHECK (text_total >= 0), row_json TEXT NOT NULL CHECK (json_valid(row_json)),
    CHECK ((inclusive_input IS NULL AND (uncached_input IS NULL OR cached_input IS NULL)) OR
           (inclusive_input IS NOT NULL AND uncached_input IS NOT NULL AND cached_input IS NOT NULL AND inclusive_input = uncached_input + cached_input)),
    CHECK ((text_total IS NULL AND (inclusive_input IS NULL OR output IS NULL)) OR
           (text_total IS NOT NULL AND inclusive_input IS NOT NULL AND output IS NOT NULL AND text_total = inclusive_input + output)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE INDEX server_snapshots_account_date ON server_daily_snapshots(provider_id, account_ref, product_id, date_label, collected_at);
CREATE TRIGGER server_snapshots_no_update BEFORE UPDATE ON server_daily_snapshots BEGIN SELECT RAISE(ABORT, 'Snapshots are append only'); END;
CREATE TABLE quota_snapshots (
    snapshot_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT,
    collected_at TEXT NOT NULL, snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE TRIGGER quota_snapshots_no_update BEFORE UPDATE ON quota_snapshots BEGIN SELECT RAISE(ABORT, 'Snapshots are append only'); END;
CREATE TABLE calibration_runs (
    run_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT NOT NULL,
    experiment_type TEXT NOT NULL, date_label TEXT NOT NULL, calibration_version TEXT NOT NULL,
    gate_status TEXT NOT NULL, actual_error_json TEXT CHECK (json_valid(actual_error_json)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE TABLE attribution_results (
    result_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT NOT NULL,
    date_label TEXT NOT NULL, status TEXT NOT NULL, calibration_version TEXT,
    uncached_input INTEGER, cached_input INTEGER, output INTEGER, text_total INTEGER,
    reason_codes_json TEXT NOT NULL CHECK (json_valid(reason_codes_json)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE TABLE app_audit (event_time TEXT NOT NULL, event_type TEXT NOT NULL, safe_status_code TEXT NOT NULL, schema_version INTEGER NOT NULL);
INSERT INTO schema_migrations VALUES (1, 'p0-dual-ledger');
