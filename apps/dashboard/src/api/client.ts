import { API_VERSION, ERROR_CODES, type ApiError, type Dashboard, type DashboardQuery, type SyncReport, type AccountSyncRequest, type AccountSummary } from './generated/monitor';

export interface UsageClient { readonly localView: boolean; readonly supportsLocalSync: boolean; readonly supportsAccountSync: boolean; getDashboard(query: DashboardQuery): Promise<Dashboard>; syncLocal(): Promise<SyncReport>; cancelLocalSync(): Promise<void>; setLocalRoots(roots: string[]): Promise<void>; setAccountQueries(enabled: boolean): Promise<void>; syncAccount(request: AccountSyncRequest): Promise<AccountSummary> }
const MAX = 9223372036854775807n;
const count = (value: string | null) => {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)$/.test(value) || BigInt(value) > MAX) throw new Error('invalid count');
  return BigInt(value);
};
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
      if (data.accountRows.some(row => row.ledger !== 'account' || row.source.kind !== 'server-observation' || row.scope.providerId !== 'codex' || row.scope.identityEvidence !== 'verified' || row.scope.accountRef !== data.account?.accountRef || !row.scope.accountRef || row.day.basis !== 'server-date-label' || row.day.startUtc !== null || row.day.endUtc !== null || row.quality.coverage !== 'unknown' || row.quality.freshness !== 'unknown')) throw new Error('invalid account binding');
      if (data.localRows.some(row => row.ledger !== 'windows' || row.source.kind !== 'local-log')) throw new Error('invalid local ledger');
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
      for (const metric of Object.values(row.tokens)) {
        count(metric.value);
        if ((metric.value === null) !== (metric.quality === 'unknown') || (metric.value === null && !metric.reasonCodes.length)) throw new Error('invalid quality');
      }
      const { uncachedInput, cachedInput, inclusiveInput, output, textTotal, reasoningOutput } = row.tokens;
      const u = count(uncachedInput.value), c = count(cachedInput.value), o = count(output.value), r = count(reasoningOutput.value);
      const input = count(inclusiveInput.value);
      const total = input === null || o === null ? null : input + o;
      if ((u !== null && c !== null && input !== u + c) || (c !== null && input !== null && c > input) || count(textTotal.value) !== total || (r !== null && (o === null || r > o))) throw new Error('invalid components');
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
