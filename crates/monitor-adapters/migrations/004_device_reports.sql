CREATE TABLE device_reports (
    device_ref TEXT PRIMARY KEY,
    generated_at TEXT NOT NULL,
    report_json TEXT NOT NULL CHECK (json_valid(report_json))
);
INSERT INTO schema_migrations VALUES (4, 'device-log-reports');
