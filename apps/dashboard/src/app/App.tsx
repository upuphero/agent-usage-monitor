import { useEffect, useMemo, useState } from 'react';
import type { ApiError, Dashboard, DemoScenario } from '../api/generated/monitor';
import { safeError } from '../api/client';
import { Icon, type IconName } from '../components/Icon';
import { formatTime } from '../components/format';
import { Notice } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';
import { History, Overview, Quality, Sessions, Settings, type Page } from '../features/DashboardPages';
import { createClient } from './createClient';

function preference(key: string, fallback: string): string { try { return localStorage.getItem(`agent-usage-monitor.${key}`) ?? fallback; } catch { return fallback; } }
function savePreference(key: string, value: string) { try { localStorage.setItem(`agent-usage-monitor.${key}`, value); } catch { /* Preferences remain usable in memory. */ } }
export function App() {
  const { t, language, setLanguage, locale } = useI18n();
  const [page, setPage] = useState<Page>('overview');
  const [scenario, setScenario] = useState<DemoScenario>('ready');
  const [provider, setProvider] = useState('codex');
  const [selectedDate, setSelectedDate] = useState('');
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(true);
  const [revision, setRevision] = useState(0);
  const [theme, setTheme] = useState(() => preference('theme', 'dark') === 'light' ? 'light' : 'dark');
  const [timezone, setTimezone] = useState(() => preference('timezone', 'America/Phoenix') === 'UTC' ? 'UTC' : 'America/Phoenix');
  const client = useMemo(() => createClient(), []);
  useEffect(() => { document.documentElement.dataset.theme = theme; savePreference('theme', theme); }, [theme]);
  useEffect(() => { savePreference('timezone', timezone); }, [timezone]);
  useEffect(() => {
    let disposed = false;
    setBusy(true); setError(null);
    client.then(transport => transport.getDashboard({ scenario })).then(result => { if (!disposed) { setData(result); setBusy(false); } }).catch(value => { if (!disposed) { setError(safeError(value)); setBusy(false); } });
    return () => { disposed = true; };
  }, [client, scenario, revision]);
  const dates = [...new Set(data?.localRows.filter(row => row.scope.providerId === provider).map(row => row.day.dateLabel) ?? [])].sort();
  const date = dates.includes(selectedDate) ? selectedDate : dates.at(-1) ?? '';
  const nav: { page: Page; icon: IconName; label: string }[] = [
    { page: 'overview', icon: 'overview', label: t('概览', 'Overview') }, { page: 'history', icon: 'chart-line', label: t('使用历史', 'Usage history') },
    { page: 'sessions', icon: 'sessions', label: t('模型与会话', 'Models & sessions') }, { page: 'quality', icon: 'shield', label: t('数据质量', 'Data quality') }, { page: 'settings', icon: 'settings', label: t('设置', 'Settings') }
  ];
  const title = nav.find(item => item.page === page)!.label;
  const latest = data?.accountRows.filter(row => row.scope.providerId === provider).sort((a, b) => Date.parse(b.source.collectedAt) - Date.parse(a.source.collectedAt))[0];
  const props = data ? { data, provider, date, timezone } : null;
  return <><a className="skip-link" href="#main">{t('跳到内容', 'Skip to content')}</a><div className="app-layout"><aside className="sidebar"><div className="brand"><span className="brand-logo"><Icon name="chart-bars" /></span><span>Agent Usage<span className="brand-caption">MONITOR</span></span></div><p className="nav-caption">{t('工作空间', 'WORKSPACE')}</p><nav aria-label={t('主导航', 'Main navigation')}>{nav.map(item => <button key={item.page} className={`nav-item ${page === item.page ? 'active' : ''}`} aria-current={page === item.page ? 'page' : undefined} onClick={() => setPage(item.page)}><Icon name={item.icon} /><span>{item.label}</span>{page === item.page && <i className="nav-active-dot" />}</button>)}</nav><div className="sidebar-bottom"><div className="local-card"><Icon name="shield" /><div>{t('合成数据预览', 'Synthetic preview')}<span className="small muted">{t('按需启动 · 真实采集关闭', 'On demand · Collection disabled')}</span></div></div><div className="sidebar-version"><span>v0.1.0</span><span>P0</span></div></div></aside><div className="main-shell"><header className="topbar"><div className="breadcrumb">Agent Usage Monitor<span>/</span>{title}</div><div className="topbar-right"><span className="platform-label">{data?.runtime.storage === 'browser-memory' ? 'BROWSER PREVIEW' : 'WINDOWS DESKTOP'}</span><div className="topbar-actions"><button className="icon-button" aria-label={t('切换主题', 'Toggle theme')} onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}><Icon name="sun" /></button><button className="icon-button language-toggle" aria-label={t('切换语言', 'Toggle language')} onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}><Icon name="language" /><span>{language === 'zh' ? 'EN' : '中文'}</span></button></div></div></header><main id="main" tabIndex={-1} className="main-content"><div className="demo-banner"><span className="demo-label">DEMO</span><div><strong>{t('全部为合成数据，非当前账号用量', 'All values are synthetic, not current account usage')}</strong><p>{t('双账本独立展示 · 真实采集未接入 · 跨设备归因未校准', 'Independent ledgers · Real collection not connected · Device attribution uncalibrated')}</p></div><label className="demo-select-label">{t('演示场景', 'Demo scenario')}<select value={scenario} onChange={event => setScenario(event.target.value as DemoScenario)}><option value="ready">{t('正常双账本', 'Dual ledgers')}</option><option value="missing">{t('缺失分量', 'Missing component')}</option><option value="negative">{t('负差值', 'Negative residual')}</option><option value="empty">{t('空数据', 'Empty data')}</option></select></label></div><div className="page-heading"><div><p className="eyebrow">USAGE INTELLIGENCE</p><h1>{title}</h1><p className="muted">{t('在一个工作空间里看清来源、范围与质量。', 'Understand sources, scope and quality in one workspace.')}</p></div><button className="button" disabled={busy} onClick={() => setRevision(value => value + 1)}><Icon name="refresh" />{t('刷新演示视图', 'Refresh demo view')}</button></div>{page !== 'settings' && <div className="monitor-toolbar"><label>{t('服务商', 'Provider')}<select aria-label={t('服务商', 'Provider')} value={provider} onChange={event => setProvider(event.target.value)}>{(data?.providers ?? [{ providerId: 'codex', displayName: 'Codex' }]).map(item => <option key={item.providerId} value={item.providerId}>{item.displayName}</option>)}</select></label>{(page === 'overview' || page === 'quality') && <label>{t('UTC 统计日期', 'UTC statistical date')}<select aria-label={t('UTC 统计日期', 'UTC statistical date')} value={date} disabled={!dates.length} onChange={event => setSelectedDate(event.target.value)}>{dates.length ? dates.map(day => <option key={day}>{day}</option>) : <option value="">{t('无日期', 'No dates')}</option>}</select></label>}<div className="account-caption"><span>{latest?.scope.accountRef ?? t('账号未知', 'Account unknown')} · {latest?.scope.productId ?? '—'}</span><span>{t('演示采集', 'Demo collection')}: {formatTime(latest?.source.collectedAt ?? null, timezone, locale)} · {timezone}</span></div></div>}{busy && <div className="small muted loading-inline" role="status">{t('正在加载演示账本…', 'Loading demo ledgers…')}</div>}{error && <Notice tone="error" title={t('账本读取失败', 'Unable to load ledgers')} action={<button className="button button-small" onClick={() => setRevision(value => value + 1)}>{t('重新读取', 'Retry read')}</button>}><p>{t('请检查应用版本与独立数据库；已有账本不会被重置。', 'Check the application version and independent database. Existing ledgers are not reset.')}</p><code>{error.code}</code></Notice>}{props && !error && <>{page === 'overview' && <Overview {...props} />}{page === 'history' && <History {...props} />}{page === 'sessions' && <Sessions {...props} />}{page === 'quality' && <Quality {...props} />}{page === 'settings' && <Settings data={props.data} timezone={timezone} setTimezone={setTimezone} />}</>}<footer className="monitor-footer"><span>{t('账号与本机数据存在重叠，不求和。', 'Account and local data overlap and are not summed.')}</span><span>Agent Usage Monitor · 0.1.0 · P0</span></footer></main></div></div></>;
}
