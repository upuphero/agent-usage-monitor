// Chart geometry and interaction adapted from the existing TrendChart component.
import { useId, useState } from 'react';
import type { LedgerRow } from '../api/generated/monitor';
import { compactCount, formatCount, latestRows, percentOf } from '../components/format';
import { EmptyState, Panel } from '../components/ui';
import { Icon } from '../components/Icon';
import { useI18n } from '../i18n/I18nContext';

type Point = { date: string; account: string | null; local: string | null };
export function seriesPoints(account: LedgerRow[], local: LedgerRow[]): Point[] {
  const dates = [...new Set([...account, ...local].map(row => row.day.dateLabel))].sort();
  const servers = latestRows(account);
  return dates.map(date => {
    const accounts = servers.filter(row => row.day.dateLabel === date), locals = local.filter(row => row.day.dateLabel === date);
    return { date, account: accounts.length === 1 ? accounts[0].tokens.textTotal.value : null, local: locals.length === 1 ? locals[0].tokens.textTotal.value : null };
  });
}
export function seriesPaths(points: Point[], key: 'account' | 'local', maximum: string) {
  const segments: string[] = [];
  let previous: Point | null = null;
  for (let i = 0; i < points.length; i++) {
    const point = points[i], value = point[key];
    if (value === null) { previous = null; continue; }
    const x = points.length === 1 ? 500 : 20 + i / (points.length - 1) * 960;
    const y = 220 - percentOf(value, maximum) * 2.2;
    if (!previous || Date.parse(point.date) - Date.parse(previous.date) !== 86400000) segments.push(`M ${x} ${y}`);
    else segments[segments.length - 1] += ` L ${x} ${y}`;
    previous = point;
  }
  return segments;
}
export function TrendChart({ account, local, observed = false }: { account: LedgerRow[]; local: LedgerRow[]; observed?: boolean }) {
  const { t, locale } = useI18n();
  const [mode, setMode] = useState<'line' | 'bars'>('line');
  const [active, setActive] = useState<number | null>(null);
  const id = useId();
  const points = seriesPoints(account, local);
  const maximum = points.flatMap(point => [point.account, point.local]).reduce<string>((max, value) => value !== null && BigInt(value) > BigInt(max) ? value : max, '0');
  const selected = active === null ? null : points[active];
  const x = (i: number) => points.length === 1 ? 500 : 20 + i / (points.length - 1) * 960;
  return <Panel title={!account.length ? t('本机 UTC 日趋势', 'Local UTC daily trend') : observed ? account.length ? t('账号与本机独立趋势', 'Separate account and local trends') : t('本机 UTC 日趋势', 'Local UTC daily trend') : t('双账本趋势', 'Dual ledger trend')} eyebrow={observed ? account.length ? t('服务端日期标签未验证 · 本机 UTC', 'Server labels unverified · Local UTC') : t('已观测部分 · UTC', 'Observed portion · UTC') : t('最近七个演示日 · UTC', 'Seven demo days · UTC')} className="trend-panel" action={<div className="chart-switch" role="group" aria-label={t('图表类型', 'Chart type')}><button aria-pressed={mode === 'line'} onClick={() => setMode('line')}><Icon name="chart-line" />{t('曲线', 'Line')}</button><button aria-pressed={mode === 'bars'} onClick={() => setMode('bars')}><Icon name="chart-bars" />{t('柱状', 'Bars')}</button></div>}>
    <div className="chart-legend">{account.length > 0 && <span><i className="chart-dot" />{t('账号服务端', 'Account server')}</span>}<span><i className="chart-dot color-2" />{t('Windows 本机', 'Windows local')}</span></div>
    {!points.length ? <EmptyState /> : <><div className="trend-surface" onPointerLeave={() => setActive(null)}><div className="trend-y-axis" aria-hidden="true">{[4, 3, 2, 1, 0].map(tick => <span key={tick} style={{ top: `${100 - tick * 25}%` }}>{compactCount((BigInt(maximum) * BigInt(tick) / 4n).toString())}</span>)}</div><div className="trend-plot"><svg className="trend-svg" viewBox="0 -8 1000 236" preserveAspectRatio="none" role="img" aria-label={account.length ? t('账号与本机两条独立序列', 'Separate account and local series') : t('Windows 本机 Token 趋势', 'Windows local Token trend')}>
      {[0, 55, 110, 165, 220].map(y => <line key={y} x1="0" x2="1000" y1={y} y2={y} className="chart-grid-line" />)}
      {(['account', 'local'] as const).map((key, series) => mode === 'line' ? seriesPaths(points, key, maximum).map((path, i) => <path key={`${key}-${i}`} d={path} className={`chart-curve series-${key}`} vectorEffect="non-scaling-stroke" />) : points.map((point, i) => point[key] === null ? null : <rect key={`${key}-${i}`} x={x(i) - 16 + series * 17} y={220 - percentOf(point[key], maximum) * 2.2} width="14" height={percentOf(point[key], maximum) * 2.2} rx="2" className={`chart-column series-${key}`} />))}
    </svg><div className="chart-hit-targets">{points.map((point, i) => <button key={point.date} aria-label={`${point.date} · ${account.length ? `${t('账号', 'Account')} ${formatCount(point.account, locale)} · ` : ''}Windows ${formatCount(point.local, locale)}`} aria-describedby={active === i ? id : undefined} style={{ left: `${x(i) / 10}%`, width: `${90 / points.length}%` }} onPointerEnter={() => setActive(i)} onFocus={() => setActive(i)} onBlur={() => setActive(null)} onClick={() => setActive(i)} onKeyDown={event => { if (event.key === 'Escape') setActive(null); if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') { event.preventDefault(); const next = Math.max(0, Math.min(points.length - 1, i + (event.key === 'ArrowLeft' ? -1 : 1))); (event.currentTarget.parentElement?.children[next] as HTMLButtonElement)?.focus(); } }} />)}</div><div className="trend-x-axis" aria-hidden="true">{points.filter((_, i) => i === 0 || i === points.length - 1 || i % Math.max(1, Math.ceil(points.length / 7)) === 0).map(point => <span key={point.date} style={{ left: `${x(points.indexOf(point)) / 10}%` }}>{point.date.slice(5)}</span>)}</div>{selected && <div className="chart-tooltip" id={id} role="tooltip" style={{ left: `clamp(110px, ${x(active!) / 10}%, calc(100% - 110px))` }}><span>{selected.date} {observed && account.length ? t('日期标签', 'date label') : 'UTC'}</span>{account.length > 0 && <strong>{t('账号', 'Account')}: {formatCount(selected.account, locale)}</strong>}<strong>Windows: {formatCount(selected.local, locale)}</strong><span className="small muted">{observed ? t('独立观测 · 缺失不补零', 'Separate observations · Missing is not zero') : t('合成数据 · 缺失不补零', 'Synthetic · Missing is not zero')}</span></div>}</div></div><p className="chart-hint">{t('悬停、聚焦或用方向键查看完整数值；组成图使用上方所选日期。', 'Hover, focus or use arrow keys for exact counts. Composition uses the selected date above.')}</p></>}
  </Panel>;
}
