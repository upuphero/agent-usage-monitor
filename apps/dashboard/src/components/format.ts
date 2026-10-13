import type { LedgerRow, ReasonCode } from '../api/generated/monitor';
import type { Language } from '../i18n/messages';

export function formatCount(value: string | null, locale = 'zh-CN'): string {
  return value === null ? '—' : BigInt(value).toLocaleString(locale);
}
export function compactCount(value: string | null): string {
  if (value === null) return '—';
  const count = BigInt(value), abs = count < 0n ? -count : count, sign = count < 0n ? '-' : '';
  for (const [threshold, suffix] of [[1000000000n, 'B'], [1000000n, 'M'], [1000n, 'K']] as const) {
    if (abs >= threshold) return `${sign}${abs / threshold}.${abs % threshold * 10n / threshold}${suffix}`;
  }
  return count.toString();
}
export function percentOf(value: string, maximum: string): number {
  const max = BigInt(maximum), count = BigInt(value);
  return max === 0n ? 0 : Number(count * 10000n / max) / 100;
}
export function latestRows(rows: LedgerRow[]): LedgerRow[] {
  const latest = new Map<string, LedgerRow>();
  for (const row of rows) {
    const key = JSON.stringify([row.scope.providerId, row.scope.productId, row.scope.accountRef, row.day.dateLabel]);
    const previous = latest.get(key);
    if (!previous || Date.parse(row.source.collectedAt) > Date.parse(previous.source.collectedAt)) latest.set(key, row);
  }
  return [...latest.values()].sort((a, b) => a.day.dateLabel.localeCompare(b.day.dateLabel));
}
const reasons: Record<ReasonCode, [string, string]> = {
  SYNTHETIC_DATA: ['合成数据', 'Synthetic data'], CALIBRATION_NOT_RUN: ['真实校准未执行', 'Real calibration not run'],
  MISSING_COMPONENT: ['分量未提供', 'Component missing'], IDENTITY_UNKNOWN: ['账号身份未知', 'Account identity unknown'],
  ACCOUNT_MISMATCH: ['账号不匹配', 'Account mismatch'], PROVIDER_MISMATCH: ['服务商不匹配', 'Provider mismatch'],
  PRODUCT_MISMATCH: ['产品范围不匹配', 'Product mismatch'], TIME_BASIS_UNKNOWN: ['日界线未验证', 'Day boundary unverified'],
  DATE_MISMATCH: ['日期不匹配', 'Date mismatch'], COVERAGE_INCOMPLETE: ['本地覆盖不完整', 'Incomplete local coverage'],
  SOURCE_MISMATCH: ['来源不匹配', 'Source mismatch'], PENDING_BACKFILL: ['等待服务端回填', 'Pending server backfill'],
  FRESHNESS_UNKNOWN: ['回填水位未知', 'Freshness unknown'], NEGATIVE_RESIDUAL: ['存在负差值', 'Negative residual'],
  INVALID_TOKEN: ['Token 数值无效', 'Invalid Token value'], INCONSISTENT_COMPONENTS: ['分量口径矛盾', 'Inconsistent components'],
  NO_DATA: ['尚无数据', 'No data'], WINDOW_NOT_REPORTED: ['窗口未报告', 'Window not reported'], NETWORK_DISABLED: ['账号查询关闭', 'Account queries disabled'],
  ACCOUNT_LEDGER_UNAVAILABLE: ['账号账本未接入', 'Account ledger not connected'], THREAD_IDENTITY_MISSING: ['线程身份缺失', 'Missing thread identity'],
  INITIAL_COUNTER_BASELINE: ['初始余额仅作为基线', 'Initial balance is baseline only'], COUNTER_RESET: ['计数回退；后续 epoch 未计入', 'Counter reset; later epoch excluded'],
  COUNTER_GAP: ['累计增量与响应不符', 'Counter delta and response disagree'], REPEATED_COUNTER_CONFLICT: ['重复累计状态存在矛盾', 'Conflicting repeated counter'],
  INHERITED_HISTORY: ['继承历史边界未核实', 'Inherited history boundary unverified'], CHILD_SCOPE_UNVERIFIED: ['子代理单独统计', 'Subagents reported separately'],
  CROSS_DAY_INTERVAL: ['跨 UTC 日区间未分摊', 'Cross-day interval not allocated'], EVENT_OWNERSHIP_AMBIGUOUS: ['执行归属有歧义', 'Execution ownership ambiguous'],
  METADATA_CONFLICT: ['线程元数据矛盾', 'Conflicting thread metadata'], INVALID_RECORD: ['存在无效日志记录', 'Invalid log records'], RECORD_TOO_LARGE: ['超大记录已跳过', 'Oversized record skipped'],
  SOURCE_REWRITTEN: ['来源重写后已重建', 'Rewritten source rebuilt'], SOURCE_MISSING: ['保留已消失来源的历史', 'History retained from missing source'],
  SOURCE_CHANGED_DURING_READ: ['来源读取期间变化或不可读', 'Source changed or unavailable'], DISCOVERY_INCOMPLETE: ['文件发现不完整', 'Incomplete file discovery'],
  IMPORTED_SOURCE: ['非 Windows 来源已排除', 'Imported non-Windows source excluded'], PRODUCT_SCOPE_UNVERIFIED: ['本地产品范围未核实', 'Local product scope unverified'],
  MODEL_UNKNOWN: ['模型配置未知', 'Model configuration unknown'], SESSION_LIST_LIMITED: ['会话列表已分页', 'Session list paginated']
};
export const reasonLabel = (reason: ReasonCode, language: Language) => reasons[reason]?.[language === 'zh' ? 0 : 1] ?? reason;
export function formatTime(value: string | null, timezone: string, locale: string): string {
  return value === null ? '—' : new Intl.DateTimeFormat(locale, { timeZone: timezone, month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value));
}
