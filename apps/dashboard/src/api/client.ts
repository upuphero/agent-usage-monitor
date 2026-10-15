import { API_VERSION, ERROR_CODES, REASON_CODES, type ApiError, type Dashboard, type DashboardQuery, type SyncReport, type AccountSyncRequest, type AccountSummary, type TokenComponents } from './generated/monitor';

export interface UsageClient { readonly localView: boolean; readonly supportsLocalSync: boolean; readonly supportsAccountSync: boolean; getDashboard(query: DashboardQuery): Promise<Dashboard>; syncLocal(): Promise<SyncReport>; cancelLocalSync(): Promise<void>; setLocalRoots(roots: string[]): Promise<void>; setAccountQueries(enabled: boolean): Promise<void>; syncAccount(request: AccountSyncRequest): Promise<AccountSummary>; exportDeviceReport?(): Promise<string>; importDeviceReport?(reportJson: string): Promise<void>; removeDeviceReport?(deviceRef: string): Promise<void> }
const MAX = 9223372036854775807n;
const count = (value: string | null) => {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)$/.test(value) || BigInt(value) > MAX) throw new Error('invalid count');
  return BigInt(value);
};
function validateTokenComponents(tokens: TokenComponents) {
      for (const metric of Object.values(tokens)) {
        count(metric.value);
        if ((metric.value === null) !== (metric.quality === 'unknown') || (metric.value === null && !metric.reasonCodes.length)) throw new Error('invalid quality');
      }
      const { uncachedInput, cachedInput, inclusiveInput, output, textTotal, reasoningOutput } = tokens;
      const u = count(uncachedInput.value), c = count(cachedInput.value), o = count(output.value), r = count(reasoningOutput.value);
      const input = count(inclusiveInput.value);
      const total = input === null || o === null ? null : input + o;
      if ((u !== null && c !== null && input !== u + c) || (c !== null && input !== null && c > input) || count(textTotal.value) !== total || (r !== null && (o === null || r > o))) throw new Error('invalid components');
}
/** Observed and synthetic datasets remain separate; account observations are scoped to the bound login. */
export function parseDashboard(payload: unknown): Dashboard {
  const data = payload as Dashboard;
  if (data?.apiVersion !== API_VERSION) throw { apiVersion: API_VERSION, code: 'API_VERSION_UNSUPPORTED', message: 'Unsupported contract version', retryable: false } satisfies ApiError;
  try {
    if (!['synthetic','observed'].includes(data.dataMode) || !Array.isArray(data.providers) || !Array.isArray(data.comparisons) || !Array.isArray(data.models) || !Array.isArray(data.sessions)
      || typeof data.runtime.networkEnabled !== 'boolean' || typeof data.runtime.accountNoticePending !== 'boolean' || !Array.isArray(data.accountRows) || !Array.isArray(data.localRows)) throw new Error('invalid mode');
    if (data.dataMode === 'synthetic' && (data.runtime.networkEnabled || data.account !== null)) throw new Error('invalid demo');
    if (data.dataMode === 'observed') {
      if (data.comparisons.length !== 0 || !data.collector || (data.runtime.networkEnabled && !data.account?.enabled)) throw new Error('invalid observed source');
      if (data.account) {
        const account = data.account;
        const validDate = (value: unknown) => typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value) && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 10) === value;
        if (account.enabled !== data.runtime.networkEnabled || (account.accountRef !== null && (typeof account.accountRef !== 'string' || !account.accountRef))
          || !['disabled','never-synced','cached','ready','partial','auth-required','access-denied','endpoint-unavailable','rate-limited','offline','incompatible'].includes(account.state)
          || !Array.isArray(account.missingDates) || account.missingDates.length > 31 || !account.missingDates.every(validDate)
          || [account.requestedStart, account.requestedEnd].some(value => value !== null && !validDate(value))
          || [account.lastAttemptAt, account.historyFetchedAt, account.quotaFetchedAt, account.nextAllowedAt].some(value => value !== null && (typeof value !== 'string' || !Number.isFinite(Date.parse(value))))
          || [account.historyError, account.quotaError].some(value => value !== null && !(ERROR_CODES as readonly string[]).includes(value))) throw new Error('invalid account state');
      }
      const breakdown = data.collector.excludedBreakdown;
      if (!Array.isArray(breakdown) || breakdown.some(group => !(REASON_CODES as readonly string[]).includes(group.reasonCode) || !Number.isInteger(group.events) || group.events <= 0 || group.events > 4294967295)
        || new Set(breakdown.map(group => group.reasonCode)).size !== breakdown.length
        || breakdown.reduce((total,group) => total + group.events,0) !== data.collector.ambiguousEvents) throw new Error('invalid exclusion breakdown');
      if (data.accountRows.some(row => row.ledger !== 'account' || row.source.kind !== 'server-observation' || row.scope.providerId !== 'codex' || row.scope.identityEvidence !== 'verified' || row.scope.accountRef !== data.account?.accountRef || !row.scope.accountRef || row.day.basis !== 'server-date-label' || row.day.startUtc !== null || row.day.endUtc !== null || row.quality.coverage !== 'unknown' || row.quality.freshness !== 'unknown')) throw new Error('invalid account binding');
      if (data.localRows.some(row => row.ledger !== 'windows' || row.source.kind !== 'local-log')) throw new Error('invalid local ledger');
    }
    if (data.deviceUsage !== null) {
      const usage = data.deviceUsage;
      const id = (value: unknown, prefix: string) => typeof value === 'string' && new RegExp(`^${prefix}-[0-9a-f]{64}$`).test(value);
      const timestamp = (value: unknown) => typeof value === 'string' && Number.isFinite(Date.parse(value));
      const integer = (value: unknown) => typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 4294967295;
      if (data.dataMode !== 'observed' || !usage || !Array.isArray(usage.devices) || usage.devices.length > 32 || !Array.isArray(usage.daily)
        || !integer(usage.duplicateEvents) || !integer(usage.conflictEvents)) throw new Error('invalid device usage');
      const ids = new Set<string>();
      for (const item of usage.devices) {
        if (!id(item.device.deviceRef, 'device') || ids.has(item.device.deviceRef) || !['windows','macos','linux','unknown'].includes(item.device.platform)
          || !timestamp(item.generatedAt) || (item.lastSyncAt !== null && !timestamp(item.lastSyncAt))
          || !integer(item.records) || !integer(item.excludedEvents) || !integer(item.filesIndexed) || !Array.isArray(item.reasonCodes)) throw new Error('invalid device');
        ids.add(item.device.deviceRef);
      }
      const groups = new Set<string>();
      for (const row of usage.daily) {
        const group = JSON.stringify([row.deviceRef, row.scope, row.dateLabel]);
        if ((row.deviceRef !== null && !ids.has(row.deviceRef)) || row.scope.providerId !== 'codex' || !['codex-local','codex-subagent'].includes(row.scope.productId)
          || row.scope.identityEvidence !== 'unknown' || (row.scope.accountRef !== null && !id(row.scope.accountRef, 'account'))
          || !/^\d{4}-\d{2}-\d{2}$/.test(row.dateLabel) || !timestamp(row.dateLabel) || new Date(row.dateLabel).toISOString().slice(0,10) !== row.dateLabel
          || !integer(row.events) || groups.has(group)) throw new Error('invalid device day');
        groups.add(group); validateTokenComponents(row.tokens);
      }
    }
    if (data.quota) {
      if (data.quota.source.dataMode !== data.dataMode || data.quota.source.kind !== 'server-observation' || !Number.isFinite(Date.parse(data.quota.source.collectedAt))) throw new Error('invalid quota source');
      if (data.dataMode === 'observed' && (!data.account?.accountRef || data.quota.scope.accountRef !== data.account.accountRef || data.quota.scope.productId !== 'codex-account' || data.quota.scope.providerId !== 'codex' || data.quota.scope.identityEvidence !== 'verified')) throw new Error('invalid quota binding');
      if (data.quota.windows.length !== 2 || new Set(data.quota.windows.map(window => window.kind)).size !== 2) throw new Error('invalid quota windows');
      for (const window of data.quota.windows) {
        if (window.windowSeconds !== (window.kind === 'five-hour' ? 18000 : window.kind === 'weekly' ? 604800 : -1)
          || (window.usedPercent === null && !window.reasonCodes.length)
          || (window.usedPercent !== null && (typeof window.usedPercent !== 'string' || !/^(0|[1-9]\d*)(\.\d+)?$/.test(window.usedPercent) || Number(window.usedPercent) > 100))
          || (window.resetsAt !== null && !Number.isFinite(Date.parse(window.resetsAt)))) throw new Error('invalid quota');
      }
    }
    for (const row of [...data.accountRows, ...data.localRows]) {
      if (row.source.dataMode !== data.dataMode || !row.scope.providerId || !row.scope.productId || !Number.isFinite(Date.parse(row.source.collectedAt))) throw new Error('invalid scope');
      validateTokenComponents(row.tokens);
    }
    for (const comparison of data.comparisons) {
      for (const value of [comparison.uncachedInput, comparison.cachedInput, comparison.output, comparison.textTotal]) {
        if (value !== null && (typeof value !== 'string' || !/^(0|-?[1-9]\d*)$/.test(value) || BigInt(value) > MAX || BigInt(value) < -MAX)) throw new Error('invalid residual');
      }
      if (comparison.status === 'blocked' && comparison.textTotal !== null) throw new Error('invalid comparison');
    }
    return data;
  } catch {
    throw { apiVersion: API_VERSION, code: 'INVALID_DATA', message: 'Invalid dashboard contract', retryable: false } satisfies ApiError;
  }
}
export function safeError(value: unknown): ApiError {
  const error = value as Partial<ApiError>;
  if (error && typeof error.code === 'string' && (ERROR_CODES as readonly string[]).includes(error.code)) {
    return { apiVersion: API_VERSION, code: error.code as ApiError['code'], message: 'Dashboard unavailable', retryable: error.retryable === true };
  }
  return { apiVersion: API_VERSION, code: 'INTERNAL', message: 'Dashboard unavailable', retryable: false };
}
