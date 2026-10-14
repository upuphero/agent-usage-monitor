// Executes the actual migration offline, also on hosts without an MSVC compiler.
import { DatabaseSync } from 'node:sqlite';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
const migration = readFileSync(new URL('../crates/monitor-adapters/migrations/001_dual_ledger.sql', import.meta.url), 'utf8');
const localMigration = readFileSync(new URL('../crates/monitor-adapters/migrations/002_incremental_local.sql', import.meta.url), 'utf8');
const accountMigration = readFileSync(new URL('../crates/monitor-adapters/migrations/003_account_reader.sql', import.meta.url), 'utf8');
const deviceMigration = readFileSync(new URL('../crates/monitor-adapters/migrations/004_device_reports.sql', import.meta.url), 'utf8');
test('SQLite migration creates the isolated dual-ledger schema transactionally', () => {
  const db = new DatabaseSync(':memory:');
  try {
    db.exec('PRAGMA foreign_keys=ON; BEGIN;'); db.exec(migration); db.exec(localMigration); db.exec(accountMigration); db.exec(deviceMigration); db.exec('PRAGMA user_version=4; COMMIT;');
    assert.equal(db.prepare('PRAGMA user_version').get().user_version, 4);
    for (const table of ['accounts', 'source_checkpoints', 'local_usage_events', 'local_daily', 'server_daily_snapshots', 'quota_snapshots', 'calibration_runs', 'attribution_results', 'app_settings', 'app_audit', 'local_streams', 'local_raw_snapshots', 'local_source_snapshots', 'account_reader_state', 'device_reports']) {
      assert.equal(db.prepare('SELECT count(*) AS count FROM sqlite_master WHERE type=? AND name=?').get('table', table).count, 1);
    }
    db.exec("INSERT INTO accounts VALUES ('codex','demo','verified');");
    const insert = db.prepare('INSERT INTO server_daily_snapshots VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)');
    const values = ['one', 'codex', 'codex-desktop', 'demo', '2026-10-01', '2026-10-02T00:00:00Z', 'synthetic', 3, 4, 7, 2, null, 9, '{}'];
    insert.run(...values); insert.run(...values.map((value, index) => index === 0 ? 'two' : value));
    assert.equal(db.prepare('SELECT count(*) AS count FROM server_daily_snapshots').get().count, 2);
    assert.throws(() => db.exec("UPDATE server_daily_snapshots SET text_total=9"));
    assert.throws(() => insert.run(...values.map((value, index) => index === 0 ? 'bad' : index === 8 ? -1 : value)));
    assert.throws(() => insert.run(...values.map((value, index) => index === 0 ? 'bad-account' : index === 3 ? 'unknown-account' : value)));
  } finally { db.close(); }
});
test('SQLite DDL failure preserves the pre-migration database', () => {
  const db = new DatabaseSync(':memory:');
  try {
    db.exec('CREATE TABLE existing_ledger(value INTEGER); INSERT INTO existing_ledger VALUES (7); BEGIN;');
    assert.throws(() => db.exec(`${migration}\n${localMigration}\n${accountMigration}\nINVALID SQL;`)); db.exec('ROLLBACK;');
    assert.equal(db.prepare('SELECT value FROM existing_ledger').get().value, 7);
    assert.equal(db.prepare("SELECT count(*) AS count FROM sqlite_master WHERE name='accounts'").get().count, 0);
    assert.equal(db.prepare('PRAGMA user_version').get().user_version, 0);
  } finally { db.close(); }
});
test('Schema 2 upgrade keeps both ledgers, quotas and checkpoint metadata unchanged', () => {
  const db = new DatabaseSync(':memory:');
  try {
    db.exec('PRAGMA foreign_keys=ON;'); db.exec(migration); db.exec(localMigration);
    db.exec("PRAGMA user_version=2; INSERT INTO accounts VALUES ('codex','account-a','verified'); INSERT INTO app_meta VALUES ('installation-ref','same-install');");
    db.prepare('INSERT INTO server_daily_snapshots VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)').run('before','codex','codex-account','account-a','2026-10-01','2026-10-02T00:00:00Z','observed',3,4,7,2,null,9,'{}');
    db.prepare('INSERT INTO quota_snapshots VALUES (?,?,?,?,?,?)').run('quota','codex','codex-account','account-a','2026-10-02T00:00:00Z','{}');
    db.exec('BEGIN;'); db.exec(accountMigration); db.exec('PRAGMA user_version=3; COMMIT;');
    assert.equal(db.prepare('SELECT text_total FROM server_daily_snapshots').get().text_total, 9);
    assert.equal(db.prepare('SELECT snapshot_id FROM quota_snapshots').get().snapshot_id, 'quota');
    assert.equal(db.prepare("SELECT value FROM app_meta WHERE key='installation-ref'").get().value, 'same-install');
    assert.equal(db.prepare('SELECT count(*) AS count FROM schema_migrations').get().count, 3);
    db.prepare('INSERT INTO account_reader_state VALUES (?,?,?)').run('codex','account-a','{}');
    assert.throws(() => db.prepare('INSERT INTO account_reader_state VALUES (?,?,?)').run('codex','account-b','{}'));
    assert.throws(() => db.exec("UPDATE server_daily_snapshots SET text_total=9"));
    assert.throws(() => db.exec("UPDATE quota_snapshots SET snapshot_json='{}'"));
  } finally { db.close(); }
});
test('Schema 1 upgrade preserves cached data and admits known total with missing cache', () => {
  const db = new DatabaseSync(':memory:');
  try {
    db.exec(migration); db.exec('PRAGMA user_version=1;');
    db.exec("INSERT INTO app_meta VALUES ('installation-ref','preserved'); INSERT INTO accounts VALUES ('codex','demo','unknown');");
    db.prepare('INSERT INTO local_daily VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)').run('demo','codex','codex-desktop','demo','2026-10-01','2026-10-02T00:00:00Z','synthetic',3,4,7,2,null,9,'{}');
    db.exec('BEGIN;'); db.exec(localMigration); db.exec('PRAGMA user_version=2; COMMIT;');
    assert.equal(db.prepare("SELECT value FROM app_meta WHERE key='installation-ref'").get().value, 'preserved');
    assert.equal(db.prepare("SELECT text_total FROM local_daily WHERE row_id='demo'").get().text_total, 9);
    db.prepare('INSERT INTO local_daily VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)').run('partial','codex','codex-local',null,'2026-10-01','2026-10-02T00:00:00Z','observed',null,null,7,2,null,9,'{}');
    assert.equal(db.prepare("SELECT text_total FROM local_daily WHERE row_id='partial'").get().text_total, 9);
    assert.equal(db.prepare('SELECT count(*) AS count FROM schema_migrations').get().count, 2);
  } finally { db.close(); }
});

test('Schema 3 device migration preserves login preferences, cached histories and installation identity', () => {
  const db = new DatabaseSync(':memory:');
  try {
    db.exec(migration); db.exec(localMigration); db.exec(accountMigration);
    db.exec("INSERT INTO app_meta VALUES ('installation-ref','preserved-device'); INSERT INTO app_settings VALUES ('account-query-enabled','false'); PRAGMA user_version=3;");
    db.exec('BEGIN;'); db.exec(deviceMigration); db.exec('PRAGMA user_version=4; COMMIT;');
    assert.equal(db.prepare("SELECT value FROM app_meta WHERE key='installation-ref'").get().value,'preserved-device');
    assert.equal(db.prepare("SELECT value FROM app_settings WHERE key='account-query-enabled'").get().value,'false');
    assert.equal(db.prepare('SELECT count(*) AS count FROM schema_migrations').get().count,4);
    db.prepare('INSERT INTO device_reports VALUES (?,?,?)').run('device','2026-10-01T00:00:00Z','{}');
    assert.throws(() => db.prepare('INSERT INTO device_reports VALUES (?,?,?)').run('bad','date','invalid-json'));
  } finally { db.close(); }
});
