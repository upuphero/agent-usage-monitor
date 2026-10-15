import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import fixtures from '../../../../tests/fixtures/demo.json';
import { API_VERSION, type Dashboard } from './generated/monitor';
import { parseDashboard } from './client';
import { I18nProvider } from '../i18n/I18nContext';
import { History, Overview, Quality, Settings, selectDay } from '../features/DashboardPages';
import { AccountSettings, AccountQuerySwitch } from '../features/AccountPanel';
import { AccountNotice } from '../features/AccountNotice';
import { latestRows } from '../components/format';
import { seriesPoints } from '../features/TrendChart';
import { SyntheticClient } from './transports/mock/SyntheticClient';

function dashboard(): Dashboard {
  const demo = parseDashboard(fixtures.ready);
  const data = structuredClone(fixtures.local) as Dashboard;
  data.account = { enabled: true, accountRef: 'synthetic-bound-account', state: 'offline', lastAttemptAt: '2026-10-12T16:00:00Z', historyFetchedAt: '2026-10-11T16:00:00Z', quotaFetchedAt: '2026-10-11T16:00:00Z', historyError: 'OFFLINE', quotaError: null, nextAllowedAt: '2026-10-12T16:01:00Z', requestedStart: '2026-10-08', requestedEnd: '2026-10-11', missingDates: ['2026-10-11'] };
  data.runtime.networkEnabled = true;
  data.accountRows = structuredClone(demo.accountRows).map(row => ({ ...row, scope: { ...row.scope, accountRef: data.account!.accountRef, productId: 'codex-account' }, day: { ...row.day, basis: 'server-date-label', timezone: null, startUtc: null, endUtc: null }, source: { ...row.source, dataMode: 'observed' }, quality: { status: 'partial', coverage: 'unknown', freshness: 'unknown', reasonCodes: ['TIME_BASIS_UNKNOWN', 'PRODUCT_SCOPE_UNVERIFIED', 'FRESHNESS_UNKNOWN'] } }));
  data.quota = structuredClone(demo.quota);
  data.quota!.scope.accountRef = data.account.accountRef; data.quota!.scope.productId = 'codex-account';
  data.quota!.source.dataMode = 'observed'; data.quota!.windows.forEach(window => window.reasonCodes.push('QUOTA_STALE'));
  return data;
}
const render = (node: React.ReactNode) => renderToStaticMarkup(<I18nProvider initialLanguage="zh">{node}</I18nProvider>);

describe('observed account cache and UI', () => {
  it('accepts bound observations and retains all historical revisions', () => {
    const data = parseDashboard(dashboard());
    expect(data.apiVersion).toBe(API_VERSION); expect(data.accountRows).toHaveLength(8);
    expect(latestRows(data.accountRows)).toHaveLength(7);
    expect(seriesPoints(data.accountRows, data.localRows).at(-1)?.account).toBe('1479000');
    expect(data.comparisons).toHaveLength(0);
  });
  it('does not choose an arbitrary product when several scopes share the same date', () => {
    const data = dashboard(); const other = structuredClone(data.accountRows.at(-1)!); other.scope.productId = 'codex-desktop'; other.rowId = 'different-product';
    data.accountRows.push(other);
    expect(selectDay(data, 'codex', '2026-10-11').account).toBeUndefined();
    expect(seriesPoints(data.accountRows, data.localRows).at(-1)?.account).toBeNull();
  });
  it('rejects account and quota mixing, fake UTC finality, and observed residuals', () => {
    for (const mutate of [
      (data: Dashboard) => { data.accountRows[0].scope.accountRef = 'another-account'; },
      (data: Dashboard) => { data.quota!.scope.accountRef = 'another-account'; },
      (data: Dashboard) => { data.accountRows[0].day.basis = 'verified-utc-day'; },
      (data: Dashboard) => { data.accountRows[0].quality.freshness = 'final'; },
      (data: Dashboard) => { data.accountRows[0].source.dataMode = 'synthetic'; },
      (data: Dashboard) => { data.quota!.windows[0].usedPercent = '101'; },
      (data: Dashboard) => { data.quota!.windows[0].windowSeconds = 604800; },
      (data: Dashboard) => { data.account!.historyFetchedAt = 'invalid-private-response'; },
      (data: Dashboard) => { data.account!.missingDates = ['2026-02-30']; },
      (data: Dashboard) => { data.comparisons = structuredClone(parseDashboard(fixtures.ready).comparisons); },
    ]) {
      const data = dashboard(); mutate(data); expect(() => parseDashboard(data)).toThrowError();
    }
  });
  it('permits disabled queries with retained cache and no automatic network consent', () => {
    const data = dashboard(); data.account!.enabled = false; data.account!.state = 'disabled'; data.runtime.networkEnabled = false;
    expect(parseDashboard(data).accountRows).toHaveLength(8);
    expect(render(<Settings data={data} timezone="UTC" setTimezone={() => {}} />)).toContain('账号查询关闭');
  });
  it('shows stale quota, missing dates and unverified date labels without hiding history', () => {
    const data = parseDashboard(dashboard()); const props = { data, provider: 'codex', date: '2026-10-11', timezone: 'America/Phoenix' };
    const overview = render(<Overview {...props} />);
    expect(overview).toContain('36.5%'); expect(overview).toContain('过期缓存'); expect(overview).not.toContain('5 小时窗口');
    expect(overview).toContain('OFFLINE'); expect(overview).toContain('缺失不补零'); expect(overview).not.toContain('口径未对齐'); expect(overview).not.toContain('series-account');
    expect(overview).not.toContain('合成服务端观测'); expect(overview).not.toContain('账号额度尚无快照');
    const history = render(<History {...props} />);
    expect(history).toContain('账号追加快照'); expect(history).toContain('同日旧版本保留'); expect(history).toContain('server-date-label');
    expect(history).toContain('本机 UTC 日账本');
    expect(render(<Quality {...props} />)).toContain('有效增量事件');
  });
  it('has explicit desktop consent and date bounds; browser cannot query accounts', async () => {
    const data = dashboard(); data.account!.enabled = false;
    const settings = render(<AccountSettings data={data} onSetAccountQueries={async () => {}} onSyncAccount={async () => {}} />);
    expect(settings).toContain('上方“账号查询”开关'); expect(settings).toContain('最多 31'); expect(settings).toContain('非公开接口'); expect(settings).toContain('disabled');
    expect(new SyntheticClient().supportsAccountSync).toBe(false);
    await expect(new SyntheticClient().syncAccount()).rejects.toThrow();
    await expect(new SyntheticClient().setAccountQueries()).rejects.toThrow();
  });
  it('places an enabled switch on the account-query row and keeps it usable during a sync', () => {
    const data = dashboard();
    const html = render(<Settings data={data} timezone="UTC" setTimezone={() => {}} accountBusy onSetAccountQueries={async () => {}} onSyncAccount={async () => {}} />);
    expect(html).toContain('role="switch"'); expect(html).toContain('aria-checked="true"');
    expect(html).toContain('aria-label="账号查询"'); expect(html).not.toContain('type="checkbox"');
    const control = render(<AccountQuerySwitch data={data} onSetAccountQueries={async () => {}} accountBusy />);
    expect(control).not.toContain('disabled');
  });
  it('explains the default and provides an immediate off option in the first-use notice', () => {
    const html = render(<AccountNotice onContinue={async () => {}} />);
    expect(html).toContain('账号查询默认开启'); expect(html).toContain('chatgpt.com');
    expect(html).toContain('关闭账号查询'); expect(html).toContain('开始使用（保持开启）');
  });
  it('hides a missing five-hour window and lets the returned weekly window fill the row', () => {
    const data = dashboard(); data.account!.state = 'partial'; data.account!.historyError = null;
    const props = { data: parseDashboard(data), provider: 'codex', date: '2026-10-11', timezone: 'UTC' };
    const html = render(<Overview {...props} />);
    expect(html).not.toContain('5 小时窗口'); expect(html).not.toContain('服务端未提供此窗口');
    expect(html).toContain('quota-single'); expect(html).toContain('每周窗口');
    expect(html).toContain('额度已更新，账号历史有日期待补齐');
    expect(html).toContain('36.5%'); expect(html).not.toContain('账号同步部分完成');
  });
  it('shows a real zero five-hour window, and shows no invented cards before data arrives', () => {
    const data = dashboard(); data.quota!.windows[0].usedPercent = '0'; data.quota!.windows[0].reasonCodes = [];
    const props = { data: parseDashboard(data), provider: 'codex', date: '2026-10-11', timezone: 'UTC' };
    const both = render(<Overview {...props} />);
    expect(both).toContain('5 小时窗口'); expect(both).toContain('0%'); expect(both).not.toContain('quota-single');
    data.quota = null;
    const absent = render(<Overview {...props} data={parseDashboard(data)} />);
    expect(absent).not.toContain('5 小时窗口'); expect(absent).not.toContain('每周窗口'); expect(absent).toContain('账号额度尚无快照');
  });
});
