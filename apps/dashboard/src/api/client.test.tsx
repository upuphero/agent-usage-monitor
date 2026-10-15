import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import fixtures from '../../../../tests/fixtures/demo.json';
import { parseDashboard, safeError } from './client';
import { SyntheticClient } from './transports/mock/SyntheticClient';
import { createClient } from '../app/createClient';
import { compactCount, formatCount, latestRows, percentOf } from '../components/format';
import { I18nProvider } from '../i18n/I18nContext';
import { Overview, History, Sessions, Quality, Settings, selectDay } from '../features/DashboardPages';
import { seriesPaths, seriesPoints } from '../features/TrendChart';
import { filterLocalScope } from '../app/App';

describe('independent dual ledger contract', () => {
  it.each(['ready', 'missing', 'negative', 'empty'] as const)('accepts the %s fixture without mutation', async scenario => {
    const client = new SyntheticClient();
    const dashboard = await client.getDashboard({ scenario });
    expect(dashboard.dataMode).toBe('synthetic');
    expect(dashboard.runtime.networkEnabled).toBe(false);
    expect(dashboard.providers.every(provider => !provider.capabilities.incrementalCollection && !provider.capabilities.calibratedEstimate)).toBe(true);
    dashboard.displayTimezone = 'UTC';
    expect((await client.getDashboard({ scenario })).displayTimezone).toBe('America/Phoenix');
  });
  it('rejects version drift, unsafe integers, double-counted cache and quality mismatches', () => {
    const wrong = structuredClone(fixtures.ready); wrong.apiVersion = 'future';
    expect(() => parseDashboard(wrong)).toThrowError();
    for (const value of ['-1', '01', '1.0', '9223372036854775808']) {
      const invalid = structuredClone(fixtures.ready); invalid.accountRows[0].tokens.cachedInput.value = value;
      expect(() => parseDashboard(invalid)).toThrowError();
    }
    const invalid = structuredClone(fixtures.ready); invalid.accountRows[0].tokens.textTotal.value = '1';
    expect(() => parseDashboard(invalid)).toThrowError();
    const quality = structuredClone(fixtures.ready); quality.accountRows[0].tokens.output.quality = 'unknown';
    expect(() => parseDashboard(quality)).toThrowError();
  });
  it('keeps unknown and zero distinct and preserves signed residuals', async () => {
    const missing = await new SyntheticClient().getDashboard({ scenario: 'missing' });
    expect(missing.comparisons.at(-1)?.textTotal).toBeNull();
    expect(missing.comparisons.at(-1)?.reasonCodes).toContain('MISSING_COMPONENT');
    const negative = await new SyntheticClient().getDashboard({ scenario: 'negative' });
    expect(BigInt(negative.comparisons.at(-1)!.textTotal!)).toBeLessThan(0n);
    expect(formatCount(null)).toBe('—'); expect(formatCount('0')).toBe('0');
  });
  it('formats counts above JS safe-integer range exactly', () => {
    expect(formatCount('9007199254740993', 'en-US')).toBe('9,007,199,254,740,993');
    expect(percentOf('4503599627370496', '9007199254740993')).toBe(49.99);
    expect(compactCount('1479000')).toBe('1.4M');
  });
  it('uses latest same-day snapshot without summing revisions', () => {
    const data = parseDashboard(fixtures.ready), last = selectDay(data, 'codex', '2026-10-11');
    expect(last.account?.rowId).toBe('demo-account-2026-10-11-2');
    expect(last.account?.tokens.textTotal.value).toBe('1479000');
    expect(latestRows(data.accountRows)).toHaveLength(7);
    expect(selectDay(data, 'claude-code', '2026-10-11').account).toBeUndefined();
  });
  it('leaves missing periods disconnected in trend curves', () => {
    const data = parseDashboard(fixtures.missing), points = seriesPoints(data.accountRows, data.localRows);
    expect(points.at(-1)?.account).toBeNull();
    const segments = seriesPaths([{ date: '2026-10-01', account: '1', local: '0' }, { date: '2026-10-03', account: '2', local: '0' }], 'account', '2');
    expect(segments).toHaveLength(2);
  });
  it('sanitizes transport errors and selects browser memory explicitly', async () => {
    expect(safeError(new Error('private path and token'))).toEqual({ apiVersion: '0.4.2', code: 'INTERNAL', message: 'Dashboard unavailable', retryable: false });
    expect(safeError({ code: 'STORAGE', message: 'private SQL' }).message).not.toContain('private');
    expect((await createClient(false)) instanceof SyntheticClient).toBe(true);
  });
});

describe('synthetic UI integration', () => {
  const data = parseDashboard(fixtures.ready);
  const props = { data, provider: 'codex', date: '2026-10-11', timezone: 'America/Phoenix' };
  const render = (element: React.ReactNode, language: 'zh' | 'en' = 'zh') => renderToStaticMarkup(<I18nProvider initialLanguage={language}>{element}</I18nProvider>);
  it('separates quota from local Tokens and removes account comparison cards', () => {
    const html = render(<Overview {...props} />);
    expect(html).toContain('账号额度'); expect(html).toContain('Windows 已观测部分');
    expect(html).not.toContain('5 小时窗口'); expect(html).not.toContain('未归因诊断差值'); expect(html).not.toContain('series-account');
    expect(html).not.toContain('总用量');
  });
  it('renders missing and inconsistent scenarios without inventing values', () => {
    const missing = render(<Overview {...props} data={parseDashboard(fixtures.missing)} />);
    expect(missing).toContain('本机 Token 组成'); expect(missing).not.toContain('减法已阻断');
    const negative = render(<Overview {...props} data={parseDashboard(fixtures.negative)} />);
    expect(negative).not.toContain('负差值'); expect(negative).not.toContain('未归因诊断差值');
  });
  it('renders history revisions, local-only sessions, quality and independent settings', () => {
    expect(render(<History {...props} />)).toContain('同日旧版本保留');
    expect(render(<Sessions {...props} />)).toContain('远程模型未知');
    expect(render(<Quality {...props} />)).toContain('CALIBRATION_NOT_RUN');
    expect(render(<Settings data={data} timezone="UTC" setTimezone={() => {}} />)).toContain('dev.agentusage.monitor');
  });
  it('shows no data for planned providers and supports English', () => {
    expect(render(<Overview {...props} provider="claude-code" />)).toContain('当前范围尚无数据');
    expect(render(<Overview {...props} />, 'en')).toContain('Account quota');
  });
});

describe('observed local ledger interface', () => {
  const data = parseDashboard(fixtures.local);
  const props = { data, provider: 'codex', date: '2026-10-11', timezone: 'America/Phoenix' };
  const render = (element: React.ReactNode) => renderToStaticMarkup(<I18nProvider initialLanguage="zh">{element}</I18nProvider>);
  it('accepts a real local contract with account and quota unavailable', () => {
    expect(data.dataMode).toBe('observed'); expect(data.runtime.localCollectionEnabled).toBe(true);
    expect(data.accountRows).toHaveLength(0); expect(data.quota).toBeNull(); expect(data.comparisons).toHaveLength(0);
    expect(data.providers[0].capabilities.incrementalCollection).toBe(true);
  });
  it('rejects mixed data modes and account values in the local-only stage', () => {
    const mixed = structuredClone(fixtures.local); mixed.localRows[0].source.dataMode = 'synthetic';
    expect(() => parseDashboard(mixed)).toThrowError();
    const account = structuredClone(fixtures.local) as unknown as Record<string, unknown>; account.accountRows = fixtures.ready.accountRows;
    expect(() => parseDashboard(account)).toThrowError();
  });
  it('renders observed portions, unknown account data and no synthetic provenance', () => {
    const html = render(<Overview {...props} />);
    expect(html).toContain('Windows 已观测部分'); expect(html).toContain('账号额度尚无快照'); expect(html).toContain('覆盖未核实');
    expect(html).not.toContain('合成服务端观测'); expect(html).not.toContain('合成本地日志');
  });
  it('keeps account and child products separate while preserving their own statistics', () => {
    const extended = structuredClone(data); const other = structuredClone(data.localRows[0]);
    other.scope.accountRef = 'another-account'; other.rowId = 'other-account'; extended.localRows.push(other);
    const child = structuredClone(data.localRows[0]); child.scope.productId = 'codex-subagent'; child.rowId = 'child'; extended.localRows.push(child);
    expect(filterLocalScope(extended, 'demo-account-a', 'codex-local').localRows).toHaveLength(7);
    expect(filterLocalScope(extended, 'demo-account-a', 'codex-subagent').localRows).toHaveLength(1);
    expect(filterLocalScope(extended, 'another-account', 'codex-local').localRows).toHaveLength(1);
  });
  it('allows a known inclusive input and total when cache decomposition is missing', () => {
    const missing = structuredClone(data); const tokens = missing.localRows[0].tokens;
    tokens.cachedInput = { value: null, quality: 'unknown', reasonCodes: ['MISSING_COMPONENT'] };
    tokens.uncachedInput = { value: null, quality: 'unknown', reasonCodes: ['MISSING_COMPONENT'] };
    expect(parseDashboard(missing).localRows[0].tokens.textTotal.value).not.toBeNull();
  });
  it('shows real daily rows, counter diagnostics and source configuration guidance', () => {
    expect(render(<History {...props} />)).toContain('本机 UTC 日账本');
    expect(render(<Quality {...props} />)).toContain('完整性检查字节');
    expect(render(<Sessions {...props} />)).toContain('turn_context');
    expect(render(<Settings data={data} timezone="UTC" setTimezone={() => {}} />)).toContain('session-cleanup-archive');
  });
  it('browser demonstration cannot return the observed-contract fixture', async () => {
    await expect(new SyntheticClient().getDashboard({ scenario: 'local' })).rejects.toThrow();
  });
});
