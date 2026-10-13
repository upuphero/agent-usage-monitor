import { API_VERSION, ERROR_CODES, type ApiError, type Dashboard, type DashboardQuery, type SyncReport } from './generated/monitor';

export interface UsageClient { readonly localView: boolean; readonly supportsLocalSync: boolean; getDashboard(query: DashboardQuery): Promise<Dashboard>; syncLocal(): Promise<SyncReport>; cancelLocalSync(): Promise<void>; setLocalRoots(roots: string[]): Promise<void> }
const MAX = 9223372036854775807n;
const count = (value: string | null) => {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)$/.test(value) || BigInt(value) > MAX) throw new Error('invalid count');
  return BigInt(value);
};
/** Observed and synthetic datasets remain separate. Stage A accepts no account ledger or quotas. */
export function parseDashboard(payload: unknown): Dashboard {
  const data = payload as Dashboard;
  if (data?.apiVersion !== API_VERSION) throw { apiVersion: API_VERSION, code: 'API_VERSION_UNSUPPORTED', message: 'Unsupported contract version', retryable: false } satisfies ApiError;
  try {
    if (!['synthetic','observed'].includes(data.dataMode) || !Array.isArray(data.providers) || !Array.isArray(data.comparisons) || !Array.isArray(data.models) || !Array.isArray(data.sessions)
      || data.runtime.networkEnabled !== false) throw new Error('invalid mode');
    if (data.dataMode === 'observed' && (data.accountRows.length !== 0 || data.quota !== null || data.comparisons.length !== 0 || !data.collector)) throw new Error('unconnected account source');
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
