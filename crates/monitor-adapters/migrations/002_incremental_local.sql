-- Preserve P0 demo rows, while allowing known inclusive input with unknown cache components.
DROP INDEX local_daily_account_date;
ALTER TABLE local_daily RENAME TO local_daily_v1;
CREATE TABLE local_daily (
    row_id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, product_id TEXT NOT NULL, account_ref TEXT,
    date_label TEXT NOT NULL, collected_at TEXT NOT NULL,
    data_mode TEXT NOT NULL CHECK (data_mode IN ('synthetic','observed')),
    uncached_input INTEGER CHECK (uncached_input >= 0), cached_input INTEGER CHECK (cached_input >= 0),
    inclusive_input INTEGER CHECK (inclusive_input >= 0), output INTEGER CHECK (output >= 0),
    reasoning_output INTEGER CHECK (reasoning_output >= 0 AND reasoning_output <= output),
    text_total INTEGER CHECK (text_total >= 0), row_json TEXT NOT NULL CHECK (json_valid(row_json)),
    CHECK (uncached_input IS NULL OR cached_input IS NULL OR (inclusive_input IS NOT NULL AND inclusive_input = uncached_input + cached_input)),
    CHECK (cached_input IS NULL OR inclusive_input IS NULL OR cached_input <= inclusive_input),
    CHECK ((text_total IS NULL AND (inclusive_input IS NULL OR output IS NULL)) OR
           (text_total IS NOT NULL AND inclusive_input IS NOT NULL AND output IS NOT NULL AND text_total = inclusive_input + output)),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
INSERT INTO local_daily SELECT * FROM local_daily_v1;
DROP TABLE local_daily_v1;
CREATE INDEX local_daily_account_date ON local_daily(provider_id, account_ref, product_id, date_label);
ALTER TABLE local_usage_events ADD COLUMN event_json TEXT CHECK (json_valid(event_json));
ALTER TABLE local_usage_events ADD COLUMN allocated INTEGER NOT NULL DEFAULT 0 CHECK (allocated IN (0,1));
CREATE INDEX local_events_scope ON local_usage_events(provider_id, account_ref, product_id, ts_utc);
CREATE TABLE local_streams (
    source_id TEXT PRIMARY KEY, root_ref TEXT NOT NULL, parser_version TEXT NOT NULL,
    byte_offset INTEGER NOT NULL CHECK (byte_offset>=0), file_size INTEGER NOT NULL CHECK (file_size>=0),
    modified_tag TEXT NOT NULL, prefix_digest TEXT NOT NULL, head_digest TEXT NOT NULL, tail_digest TEXT NOT NULL,
    state_json TEXT NOT NULL CHECK (json_valid(state_json)), present INTEGER NOT NULL CHECK (present IN (0,1)),
    issues_json TEXT NOT NULL CHECK (json_valid(issues_json)), generation INTEGER NOT NULL DEFAULT 1, alias_of TEXT
);
CREATE TABLE local_raw_snapshots (
    event_key TEXT PRIMARY KEY, thread_ref TEXT NOT NULL, ownership_key TEXT NOT NULL,
    metadata_digest TEXT NOT NULL, response_ref TEXT, at_utc TEXT NOT NULL,
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)), conflict INTEGER NOT NULL DEFAULT 0 CHECK (conflict IN (0,1))
);
CREATE INDEX raw_snapshots_thread ON local_raw_snapshots(thread_ref, at_utc, event_key);
CREATE INDEX raw_snapshots_ownership ON local_raw_snapshots(ownership_key, thread_ref);
CREATE INDEX raw_snapshots_response ON local_raw_snapshots(response_ref, thread_ref);
CREATE TABLE local_source_snapshots (
    source_id TEXT NOT NULL, event_key TEXT NOT NULL, active INTEGER NOT NULL CHECK (active IN (0,1)), generation INTEGER NOT NULL,
    PRIMARY KEY(source_id,event_key),
    FOREIGN KEY(source_id) REFERENCES local_streams(source_id),
    FOREIGN KEY(event_key) REFERENCES local_raw_snapshots(event_key)
);
CREATE INDEX local_snapshot_active ON local_source_snapshots(event_key, active);
INSERT INTO schema_migrations VALUES (2, 'incremental-local-ledger');
