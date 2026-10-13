import { API_VERSION, ERROR_CODES, type ApiError, type Dashboard, type DashboardQuery } from './generated/monitor';

export interface UsageClient { getDashboard(query: DashboardQuery): Promise<Dashboard> }
const MAX = 9223372036854775807n;
const count = (value: string | null) => {
  if (value === null) return null;
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)$/.test(value) || BigInt(value) > MAX) throw new Error('invalid count');
  return BigInt(value);
};
/** IPC v0.1 accepts the synthetic P0 contract only. Errors stay separate from demo data. */
export function parseDashboard(payload: unknown): Dashboard {
  const data = payload as Dashboard;
  if (data?.apiVersion !== API_VERSION) throw { apiVersion: API_VERSION, code: 'API_VERSION_UNSUPPORTED', message: 'Unsupported contract version', retryable: false } satisfies ApiError;
  try {
    if (data.dataMode !== 'synthetic' || !Array.isArray(data.providers) || !Array.isArray(data.comparisons) || !Array.isArray(data.models) || !Array.isArray(data.sessions)
      || data.runtime.networkEnabled !== false || data.runtime.localCollectionEnabled !== false) throw new Error('invalid mode');
    for (const row of [...data.accountRows, ...data.localRows]) {
      if (row.source.dataMode !== 'synthetic' || !row.scope.providerId || !row.scope.productId) throw new Error('invalid scope');
      for (const metric of Object.values(row.tokens)) {
        count(metric.value);
        if ((metric.value === null) !== (metric.quality === 'unknown') || (metric.value === null && !metric.reasonCodes.length)) throw new Error('invalid quality');
      }
      const { uncachedInput, cachedInput, inclusiveInput, output, textTotal, reasoningOutput } = row.tokens;
      const u = count(uncachedInput.value), c = count(cachedInput.value), o = count(output.value), r = count(reasoningOutput.value);
      const input = u === null || c === null ? null : u + c;
      const total = input === null || o === null ? null : input + o;
      if (count(inclusiveInput.value) !== input || count(textTotal.value) !== total || (r !== null && (o === null || r > o))) throw new Error('invalid components');
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
