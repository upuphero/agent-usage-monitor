import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import fixtures from '../../../../tests/fixtures/demo.json';
import type { Dashboard, DeviceDay, DeviceUsage } from './generated/monitor';
import { parseDashboard } from './client';
import { Overview, Quality } from '../features/DashboardPages';
import { combineLocalDays, DeviceTransfer, DeviceUsagePanel, sumDeviceMetric } from '../features/DeviceUsage';
import { filterLocalScope } from '../app/App';
import { I18nProvider } from '../i18n/I18nContext';
const windows = `device-${'a'.repeat(64)}`, mac = `device-${'b'.repeat(64)}`;
const render = (node: React.ReactNode) => renderToStaticMarkup(<I18nProvider initialLanguage="zh">{node}</I18nProvider>);
function data(): Dashboard {
  const result = structuredClone(fixtures.local) as Dashboard;
  const summary = (deviceRef: string, platform: 'windows' | 'macos') => ({ device: { deviceRef, platform }, generatedAt: '2026-10-12T16:00:00Z', lastSyncAt: '2026-10-12T16:00:00Z', filesIndexed: 1, records: 1, excludedEvents: 2, reasonCodes: ['COVERAGE_INCOMPLETE'] as const });
  const day = (deviceRef: string | null, total: string): DeviceDay => {
    const tokens = structuredClone(result.localRows[0].tokens);
    for (const key of ['uncachedInput','cachedInput','inclusiveInput','output','textTotal'] as const) tokens[key].value = key === 'uncachedInput' || key === 'inclusiveInput' || key === 'textTotal' ? total : '0';
    return { deviceRef, dateLabel: '2026-10-11', scope: { providerId: 'codex', productId: 'codex-local', accountRef: null, identityEvidence: 'unknown' }, tokens, events: 1 };
  };
  result.deviceUsage = { devices: [summary(windows,'windows'), summary(mac,'macos')], daily: [day(windows,'26'),day(mac,'26'),day(null,'26')], duplicateEvents: 1, conflictEvents: 0 } as unknown as DeviceUsage;
  return result;
}
describe('device reports and separate account quota', () => {
  it('shows actual report values and their deduplicated union without account arithmetic', () => {
    const dashboard = parseDashboard(data());
    const html = render(<Overview data={dashboard} provider="codex" date="2026-10-11" timezone="UTC" />);
    expect(html).toContain('账号额度'); expect(html).toContain('Windows Token'); expect(html).toContain('Mac Token'); expect(html).toContain('设备日志去重汇总');
    expect(html).not.toContain('未归因诊断差值'); expect(html).not.toContain('设备归因尚未开放'); expect(html).not.toContain('series-account');
    expect(sumDeviceMetric(dashboard.deviceUsage!.daily.filter(row => row.deviceRef === null), 'textTotal')).toBe('26');
  });
  it('shows Mac unconnected and missing selected dates without inventing zero', () => {
    const dashboard = data(); dashboard.deviceUsage!.devices.pop(); dashboard.deviceUsage!.daily = dashboard.deviceUsage!.daily.filter(row => row.deviceRef !== mac);
    const html = render(<DeviceUsagePanel data={dashboard} date="2026-10-10" timezone="UTC" />);
    expect(html).toContain('未接入'); expect(html).toContain('所选日期没有可计入记录');
    expect(sumDeviceMetric([], 'textTotal')).toBeNull();
  });
  it('preserves account/product scope and exact counts above the JS safe range', () => {
    const dashboard = data(); const child = structuredClone(dashboard.deviceUsage!.daily[0]); child.scope.productId = 'codex-subagent'; dashboard.deviceUsage!.daily.push(child);
    expect(filterLocalScope(dashboard,'unmarked','codex-local').deviceUsage!.daily).toHaveLength(3);
    expect(filterLocalScope(dashboard,'unmarked','codex-subagent').deviceUsage!.daily).toHaveLength(1);
    const row = structuredClone(child); row.tokens.textTotal.value = '9007199254740993';
    expect(sumDeviceMetric([row], 'textTotal')).toBe('9007199254740993');
    row.tokens.textTotal.value = null; expect(sumDeviceMetric([row], 'textTotal')).toBeNull();
  });
  it('rejects unsafe report projections and inconsistent token components', () => {
    for (const mutate of [
      (d: Dashboard) => { d.deviceUsage!.devices[0].device.deviceRef = 'private-hostname'; },
      (d: Dashboard) => { d.deviceUsage!.daily[0].tokens.textTotal.value = '99'; },
      (d: Dashboard) => { d.deviceUsage!.daily[0].dateLabel = '2026-02-30'; },
      (d: Dashboard) => { d.deviceUsage!.daily[0].deviceRef = `device-${'c'.repeat(64)}`; },
      (d: Dashboard) => { d.deviceUsage!.daily.push(structuredClone(d.deviceUsage!.daily[0])); },
    ]) { const invalid = data(); mutate(invalid); expect(() => parseDashboard(invalid)).toThrowError(); }
  });
  it('offers export/import and explains a standalone Mac collector', () => {
    const html = render(<DeviceTransfer data={data()} onChanged={async () => {}} />);
    expect(html).toContain('导出本机报告'); expect(html).toContain('导入设备报告'); expect(html).toContain('./monitor-collector collect'); expect(html).toContain('移除导入报告');
    expect(html).not.toContain('auth.json');
  });
  it('combines multiple local account scopes without losing exact day totals', () => {
    const rows = structuredClone(fixtures.local.localRows.slice(0,1)) as Dashboard['localRows'];
    const other = structuredClone(rows[0]); other.scope.accountRef = 'another-account'; rows.push(other);
    const combined = combineLocalDays(rows);
    expect(combined).toHaveLength(1); expect(combined[0].scope.accountRef).toBeNull();
    expect(combined[0].tokens.textTotal.value).toBe((BigInt(rows[0].tokens.textTotal.value!) * 2n).toString());
  });
  it('explains exclusions as all-history primary counts whose sum must match the total', () => {
    const dashboard = parseDashboard(data());
    const html = render(<Quality data={dashboard} provider="codex" date="2026-10-11" timezone="UTC" />);
    expect(html).toContain('未计入记录的主要原因'); expect(html).toContain('统计全部历史记录，非所选当天');
    expect(html).toContain('每条记录只归入一个主要原因'); expect(html).toContain('累计增量与响应不符');
    expect(html).toContain('无法确认中间用量'); expect(html).toContain('采集提示（包含历史记录）');
    const inflated = data(); inflated.collector!.excludedBreakdown[0].events += 1;
    expect(() => parseDashboard(inflated)).toThrowError();
    const repeated = data(); repeated.collector!.excludedBreakdown.push(structuredClone(repeated.collector!.excludedBreakdown[0]));
    expect(() => parseDashboard(repeated)).toThrowError();
  });
});
