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
  NO_DATA: ['尚无数据', 'No data'], WINDOW_NOT_REPORTED: ['窗口未报告', 'Window not reported'], NETWORK_DISABLED: ['账号查询关闭', 'Account queries disabled']
};
export const reasonLabel = (reason: ReasonCode, language: Language) => reasons[reason]?.[language === 'zh' ? 0 : 1] ?? reason;
export function formatTime(value: string | null, timezone: string, locale: string): string {
  return value === null ? '—' : new Intl.DateTimeFormat(locale, { timeZone: timezone, month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value));
}
