// Deterministic synthetic data only; no real account observations or logs.
import { mkdir, writeFile } from 'node:fs/promises';
const known = value => ({ value: String(value), quality: 'known', reasonCodes: [] });
const unknown = reason => ({ value: null, quality: 'unknown', reasonCodes: [reason] });
const scope = { providerId: 'codex', productId: 'codex-desktop', accountRef: 'demo-account-a', identityEvidence: 'verified' };
const tokens = (uncached, cached, output) => ({
  uncachedInput: known(uncached), cachedInput: known(cached), inclusiveInput: known(uncached + cached),
  output: known(output), reasoningOutput: unknown('MISSING_COMPONENT'), textTotal: known(uncached + cached + output)
});
const providers = [
  { providerId: 'codex', displayName: 'Codex', state: 'synthetic-preview' },
  { providerId: 'claude-code', displayName: 'Claude Code', state: 'planned' },
  { providerId: 'antigravity', displayName: 'Antigravity', state: 'planned' }
].map(provider => ({ ...provider, capabilities: { localLedger: false, accountLedger: false, incrementalCollection: false, calibratedEstimate: false } }));
const row = (day, ledger, components, revision = '1') => ({
  rowId: `demo-${ledger}-${day}-${revision}`, ledger, scope: { ...scope },
  day: { dateLabel: day, basis: 'verified-utc-day', timezone: 'UTC', startUtc: `${day}T00:00:00Z`, endUtc: `${new Date(Date.parse(`${day}T00:00:00Z`) + 86400000).toISOString().slice(0, 10)}T00:00:00Z` },
  source: { sourceId: `demo-${ledger}`, kind: ledger === 'account' ? 'server-observation' : 'local-log', collectedAt: `2026-10-${revision === '2' ? '12' : '11'}T16:00:00Z`, dataMode: 'synthetic' },
  tokens: tokens(...components), quality: { status: 'known', coverage: 'complete', freshness: ledger === 'account' ? 'pending-backfill' : 'final', reasonCodes: ['SYNTHETIC_DATA'] }
});
const dates = Array.from({ length: 7 }, (_, index) => `2026-10-${String(index + 5).padStart(2, '0')}`);
const accounts = dates.map((day, i) => row(day, 'account', [120000 + i * 18000, 820000 + [0, 140000, 90000, 310000, 180000, 440000, 290000][i], 36000 + i * 3500]));
accounts.push(row(dates.at(-1), 'account', [238000, 1180000, 61000], '2'));
const locals = dates.map((day, i) => row(day, 'windows', [78000 + i * 10000, 590000 + [0, 120000, 60000, 170000, 80000, 190000, 140000][i], 25000 + i * 2100]));
const total = locals.reduce((sum, item) => sum + BigInt(item.tokens.textTotal.value), 0n);
const ready = {
  apiVersion: '0.4.2', dataMode: 'synthetic', displayTimezone: 'America/Phoenix', providers, collector: null, account: null, deviceUsage: null,
  accountRows: accounts, localRows: locals, comparisons: [],
  quota: { snapshotId: 'demo-quota-1', scope, source: { sourceId: 'demo-quota', kind: 'server-observation', collectedAt: '2026-10-12T16:00:00Z', dataMode: 'synthetic' }, windows: [
    { kind: 'five-hour', windowSeconds: 18000, usedPercent: null, resetsAt: null, reasonCodes: ['WINDOW_NOT_REPORTED'] },
    { kind: 'weekly', windowSeconds: 604800, usedPercent: '36.5', resetsAt: '2026-10-16T00:00:00Z', reasonCodes: ['SYNTHETIC_DATA'] }
  ] },
  models: [{ modelId: 'demo-model-alpha', scope, textTotal: known(total * 3n / 5n), sessions: 8 }, { modelId: 'demo-model-beta', scope, textTotal: known(total - total * 3n / 5n), sessions: 5 }],
  sessions: [
    { sessionRef: 'demo-session-01', modelId: 'demo-model-alpha', effort: 'high', speed: null, scope, textTotal: known('128600') },
    { sessionRef: 'demo-session-02', modelId: 'demo-model-beta', effort: null, speed: null, scope, textTotal: known('97600') }
  ],
  runtime: { appVersion: '0.4.2', schemaVersion: 4, storage: 'browser-memory', installationRef: 'synthetic-browser', networkEnabled: false, accountNoticePending: false, localCollectionEnabled: false }
};
function comparisons(data) {
  return data.localRows.map(local => {
    const account = data.accountRows.filter(item => item.day.dateLabel === local.day.dateLabel).sort((a, b) => Date.parse(b.source.collectedAt) - Date.parse(a.source.collectedAt))[0];
    const keys = ['uncachedInput', 'cachedInput', 'output', 'textTotal'];
    const missing = keys.some(key => account.tokens[key].value === null || local.tokens[key].value === null);
    const values = missing ? keys.map(() => null) : keys.map(key => (BigInt(account.tokens[key].value) - BigInt(local.tokens[key].value)).toString());
    const negative = values.some(value => value !== null && BigInt(value) < 0n);
    return { scope: structuredClone(account.scope), accountRowId: account.rowId, localRowId: local.rowId, dateLabel: local.day.dateLabel, status: missing ? 'blocked' : negative ? 'inconsistent' : 'unattributed', ...Object.fromEntries(keys.map((key, i) => [key, values[i]])), reasonCodes: missing ? ['MISSING_COMPONENT'] : [...(negative ? ['NEGATIVE_RESIDUAL'] : []), 'SYNTHETIC_DATA', 'PENDING_BACKFILL', 'CALIBRATION_NOT_RUN'] };
  });
}
const missing = structuredClone(ready);
for (const account of missing.accountRows.filter(item => item.day.dateLabel === dates.at(-1))) {
  for (const key of ['cachedInput', 'inclusiveInput', 'textTotal']) account.tokens[key] = unknown('MISSING_COMPONENT');
  account.quality.status = 'unknown'; account.quality.reasonCodes.push('MISSING_COMPONENT');
}
const negative = structuredClone(ready);
negative.localRows.at(-1).tokens = tokens(338000, 1580000, 91000);
const empty = { ...structuredClone(ready), accountRows: [], localRows: [], quota: null, models: [], sessions: [] };
const fixtures = { ready, missing, negative, empty };
for (const data of Object.values(fixtures)) {
  data.comparisons = comparisons(data);
  const localTotal = data.localRows.reduce((sum, row) => sum + BigInt(row.tokens.textTotal.value), 0n);
  if (data.models.length) {
    data.models[0].textTotal = known(localTotal * 3n / 5n);
    data.models[1].textTotal = known(localTotal - localTotal * 3n / 5n);
    data.sessions = data.models.flatMap((model, modelIndex) => Array.from({ length: model.sessions }, (_, index) => ({
      sessionRef: `demo-session-${modelIndex}-${String(index + 1).padStart(2, '0')}`, modelId: model.modelId,
      effort: index % 2 === 0 ? 'high' : null, speed: null, scope: structuredClone(scope),
      textTotal: known(BigInt(model.textTotal.value) / BigInt(model.sessions) + (index === model.sessions - 1 ? BigInt(model.textTotal.value) % BigInt(model.sessions) : 0n))
    })));
  }
}
const local = structuredClone(ready);
local.dataMode = 'observed'; local.accountRows = []; local.comparisons = []; local.quota = null;
local.runtime.storage = 'monitor.sqlite3'; local.runtime.installationRef = 'fixture-installation'; local.runtime.localCollectionEnabled = true;
local.localRows.forEach(row => { row.source.dataMode = 'observed'; row.scope.productId = 'codex-local'; row.scope.identityEvidence = 'unknown'; row.quality.status = 'partial'; row.quality.coverage = 'partial'; row.quality.freshness = 'unknown'; row.quality.reasonCodes = ['IDENTITY_UNKNOWN', 'COVERAGE_INCOMPLETE']; });
for (const row of [...local.models,...local.sessions]) { row.scope.productId = 'codex-local'; row.scope.identityEvidence = 'unknown'; }
local.providers[0].state = 'local-partial'; local.providers[0].capabilities.localLedger = true; local.providers[0].capabilities.incrementalCollection = true;
local.collector = { state: 'idle', lastSyncAt: '2026-10-12T16:00:00Z', filesIndexed: 13, retainedSources: 0, snapshots: 30, acceptedEvents: 25, ambiguousEvents: 5, sessionsTotal: 13, excludedBreakdown: [{ reasonCode: 'COUNTER_GAP', events: 5 }],
  reasonCodes: ['IDENTITY_UNKNOWN','COVERAGE_INCOMPLETE'],
  lastRun: { filesFound: 13, filesParsed: 1, filesUnchanged: 12, filesRebuilt: 0, newSnapshots: 2, duplicateSnapshots: 0, bytesParsed: '2048', bytesVerified: '8192', partialTails: 0, reasonCodes: [] } };
fixtures.local = local; // Native-contract example for tests only; SyntheticClient never returns it.
await mkdir(new URL('../tests/fixtures/', import.meta.url), { recursive: true });
await writeFile(new URL('../tests/fixtures/demo.json', import.meta.url), `${JSON.stringify(fixtures, null, 2)}\n`);
console.log('Wrote synthetic demo fixtures.');
