-- Keep both existing ledgers and all prior snapshots. Only safe sync metadata is added.
CREATE TABLE account_reader_state (
    provider_id TEXT NOT NULL, account_ref TEXT NOT NULL,
    summary_json TEXT NOT NULL CHECK (json_valid(summary_json)),
    PRIMARY KEY (provider_id, account_ref),
    FOREIGN KEY (provider_id, account_ref) REFERENCES accounts(provider_id, account_ref)
);
CREATE INDEX quota_scope_time ON quota_snapshots(provider_id, account_ref, product_id, collected_at);
INSERT INTO schema_migrations VALUES (3, 'account-readonly-reader');
