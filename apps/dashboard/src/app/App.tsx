import { useEffect, useMemo, useRef, useState } from 'react';
import type { ApiError, Dashboard, DemoScenario, AccountSyncRequest } from '../api/generated/monitor';
import { safeError, type UsageClient } from '../api/client';
import { Icon, type IconName } from '../components/Icon';
import { formatTime } from '../components/format';
import { Notice } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';
import { History, Overview, Quality, Sessions, Settings, type Page } from '../features/DashboardPages';
import { createClient, isDesktopRuntime, isLocalReportPreview } from './createClient';
import { DeviceTransfer, combineLocalDays } from '../features/DeviceUsage';
import { AccountNotice } from '../features/AccountNotice';

function preference(key: string, fallback: string): string { try { return localStorage.getItem(`agent-usage-monitor.${key}`) ?? fallback; } catch { return fallback; } }
function savePreference(key: string, value: string) { try { localStorage.setItem(`agent-usage-monitor.${key}`, value); } catch { /* Preferences remain usable in memory. */ } }
export function filterLocalScope(data: Dashboard, account: string, product: string): Dashboard {
  const inScope = (row: { scope: { accountRef: string | null; productId: string } }) => (account === '*' || (row.scope.accountRef ?? 'unmarked') === account) && row.scope.productId === product;
  return { ...data, localRows: data.localRows.filter(inScope), models: data.models.filter(inScope), sessions: data.sessions.filter(inScope), deviceUsage: data.deviceUsage ? { ...data.deviceUsage, daily: data.deviceUsage.daily.filter(inScope) } : null };
}
export function App({ usageClient }: { usageClient?: UsageClient } = {}) {
  const { t, language, setLanguage, locale } = useI18n();
  const [page, setPage] = useState<Page>('overview');
  const [scenario, setScenario] = useState<DemoScenario>(() => usageClient?.localView || isDesktopRuntime() || isLocalReportPreview() ? 'local' : 'ready');
  const [provider, setProvider] = useState('codex');
  const [account, setAccount] = useState('*');
  const [product, setProduct] = useState('codex-local');
  const [accountProduct, setAccountProduct] = useState('codex-account');
  const [accountBusy, setAccountBusy] = useState(false);
  const [selectedDate, setSelectedDate] = useState('');
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [transport, setTransport] = useState<UsageClient | null>(null);
  const [busy, setBusy] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [revision, setRevision] = useState(0);
  const [theme, setTheme] = useState(() => preference('theme', 'dark') === 'light' ? 'light' : 'dark');
  const [timezone, setTimezone] = useState(() => preference('timezone', 'America/Phoenix') === 'UTC' ? 'UTC' : 'America/Phoenix');
  const client = useMemo(() => usageClient ? Promise.resolve(usageClient) : createClient(), [usageClient]);
  const autoStarted = useRef(false);
  const autoAccountStarted = useRef(false);
  useEffect(() => { document.documentElement.dataset.theme = theme; savePreference('theme', theme); }, [theme]);
  useEffect(() => { savePreference('timezone', timezone); }, [timezone]);
  useEffect(() => {
    let disposed = false;
    setBusy(true); setError(null);
    client.then(async value => {
      if (!disposed) setTransport(value);
      const loaded = await value.getDashboard({ scenario });
      if (!disposed) { setData(loaded); setBusy(false); }
    }).catch(value => { if (!disposed) { setError(safeError(value)); setBusy(false); } });
    return () => { disposed = true; };
  }, [client, scenario, revision]);
  async function synchronize(value: UsageClient) {
    setSyncing(true); setError(null);
    try { await value.syncLocal(); }
    catch (failure) { setError(safeError(failure)); }
    finally {
      try { setData(await value.getDashboard({ scenario: 'local' })); } catch (failure) { setError(safeError(failure)); }
      setSyncing(false);
    }
  }
  async function accountOperation(action: (value: UsageClient) => Promise<unknown>) {
    if (!transport) return;
    setAccountBusy(true);
    try { await action(transport); }
    finally { try { setData(await transport.getDashboard({ scenario: 'local' })); } finally { setAccountBusy(false); } }
  }
  async function configureAccountQueries(enabled: boolean) {
    if (!transport) return;
    setError(null);
    await transport.setAccountQueries(enabled);
    setData(await transport.getDashboard({ scenario: 'local' }));
  }
  useEffect(() => {
    if (data?.account?.enabled === false) autoAccountStarted.current = false;
    if (!transport?.supportsAccountSync || scenario !== 'local' || data?.dataMode !== 'observed' || !data.account?.enabled || data.runtime.accountNoticePending || accountBusy || autoAccountStarted.current) return;
    autoAccountStarted.current = true;
    if (data.account.nextAllowedAt && Date.parse(data.account.nextAllowedAt) > Date.now()) return;
    const request = { startDate: new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10), endDate: new Date().toISOString().slice(0, 10) };
    void accountOperation(value => value.syncAccount(request)).catch(failure => {
      const safe = safeError(failure);
      if (safe.code !== 'NETWORK_DISABLED' && safe.code !== 'CANCELLED') setError(safe);
    });
  }, [transport, scenario, data?.dataMode, data?.account?.enabled, data?.runtime.accountNoticePending, accountBusy]);
  useEffect(() => {
    if (transport?.supportsLocalSync && data?.dataMode === 'observed' && scenario === 'local' && !autoStarted.current) {
      autoStarted.current = true; void synchronize(transport);
    }
  }, [transport, data?.dataMode, scenario]);
  useEffect(() => () => { if (transport?.supportsLocalSync) void transport.cancelLocalSync(); }, [transport]);
  const observed = data?.dataMode === 'observed';
  const localScopes = [...(data?.localRows ?? []), ...(data?.deviceUsage?.daily ?? [])].filter(row => row.scope.providerId === provider);
  const localAccounts = [...new Set(localScopes.map(row => row.scope.accountRef ?? 'unmarked'))];
  const selectedAccount = account === '*' || localAccounts.includes(account) ? account : '*';
  const localProducts = [...new Set(localScopes.filter(row => selectedAccount === '*' || (row.scope.accountRef ?? 'unmarked') === selectedAccount).map(row => row.scope.productId))];
  const selectedProduct = localProducts.includes(product) ? product : localProducts[0] ?? 'codex-local';
  const accountProducts = [...new Set(data?.accountRows.filter(row => row.scope.providerId === provider).map(row => row.scope.productId) ?? [])];
  const selectedAccountProduct = accountProducts.includes(accountProduct) ? accountProduct : accountProducts[0] ?? 'codex-account';
  const localScope = data ? filterLocalScope(data, selectedAccount, selectedProduct) : null;
  const scoped = data ? observed && localScope ? { ...localScope, localRows: selectedAccount === '*' ? combineLocalDays(localScope.localRows) : localScope.localRows, accountRows: data.accountRows.filter(row => row.scope.productId === selectedAccountProduct) } : data : null;
  const dates = [...new Set([...(scoped?.localRows ?? []), ...(scoped?.deviceUsage?.daily.map(row => ({ scope: row.scope, day: { dateLabel: row.dateLabel } })) ?? [])].filter(row => row.scope.providerId === provider).map(row => row.day.dateLabel))].sort();
  const date = dates.includes(selectedDate) ? selectedDate : dates.at(-1) ?? '';
  const nav: { page: Page; icon: IconName; label: string }[] = [
    { page: 'overview', icon: 'overview', label: t('概览', 'Overview') }, { page: 'history', icon: 'chart-line', label: t('使用历史', 'Usage history') },
    { page: 'sessions', icon: 'sessions', label: t('模型与会话', 'Models & sessions') }, { page: 'quality', icon: 'shield', label: t('数据质量', 'Data quality') }, { page: 'settings', icon: 'settings', label: t('设置', 'Settings') }
  ];
  const title = nav.find(item => item.page === page)!.label;
  const collectionTime = observed ? data?.collector?.lastSyncAt ?? null : data?.accountRows.map(row => row.source.collectedAt).sort().at(-1) ?? null;
  const props = scoped ? { data: scoped, provider, date, timezone } : null;
  return <>
    {observed && data?.runtime.accountNoticePending && transport?.supportsAccountSync && <AccountNotice onContinue={configureAccountQueries} />}
    <a className="skip-link" href="#main">{t('跳到内容', 'Skip to content')}</a>
    <div className="app-layout">
      <aside className="sidebar">
        <div className="brand"><span className="brand-logo"><Icon name="chart-bars" /></span><span>Agent Usage<span className="brand-caption">MONITOR</span></span></div>
        <p className="nav-caption">{t('工作空间', 'WORKSPACE')}</p>
        <nav aria-label={t('主导航', 'Main navigation')}>{nav.map(item => <button key={item.page} aria-label={item.label} title={item.label} className={`nav-item ${page === item.page ? 'active' : ''}`} aria-current={page === item.page ? 'page' : undefined} onClick={() => setPage(item.page)}><Icon name={item.icon} /><span>{item.label}</span>{page === item.page && <i className="nav-active-dot" />}</button>)}</nav>
        <div className="sidebar-bottom"><div className="local-card"><Icon name="shield" /><div>{observed ? t('Windows 本机日志', 'Windows local logs') : t('合成数据预览', 'Synthetic preview')}<span className="small muted">{data?.account?.enabled ? t('启动时同步账号', 'Account sync on startup') : t('按需启动 · 账号查询关闭', 'On demand · Account queries disabled')}</span></div></div><div className="sidebar-version"><span>v0.4.0</span><span>LOGS</span></div></div>
      </aside>
      <div className="main-shell">
        <header className="topbar"><div className="breadcrumb">Agent Usage Monitor<span>/</span>{title}</div><div className="topbar-right"><span className="platform-label">{isDesktopRuntime() ? 'WINDOWS DESKTOP' : 'BROWSER PREVIEW'}</span><div className="topbar-actions"><button className="icon-button" aria-label={t('切换主题', 'Toggle theme')} onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}><Icon name="sun" /></button><button className="icon-button language-toggle" aria-label={t('切换语言', 'Toggle language')} onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}><Icon name="language" /><span>{language === 'zh' ? 'EN' : '中文'}</span></button></div></div></header>
        <main id="main" tabIndex={-1} className="main-content">
          <div className="demo-banner"><span className="demo-label">{observed ? 'LOCAL' : 'DEMO'}</span><div><strong>{observed ? t('账号额度与设备日志分别统计', 'Account quota and device logs are tracked separately') : t('全部为合成数据，非当前账号用量', 'All values are synthetic, not current account usage')}</strong><p>{observed ? t('账号额度单独查询 · Windows / Mac 各自采集日志 · 汇总只合并日志记录', 'Account quota is queried separately · Windows / Mac collect their own logs · Only log records are merged') : t('双账本合成样本 · 切换“本机账本”查看真实日志', 'Synthetic dual ledgers · Switch to local ledger for real logs')}</p></div><label className="demo-select-label">{t('数据来源', 'Data source')}<select value={scenario} disabled={syncing || accountBusy} onChange={event => setScenario(event.target.value as DemoScenario)}><option value="local" disabled={!transport?.localView}>{t('本机账本', 'Local ledger')}</option><option value="ready">{t('正常双账本演示', 'Dual ledger demo')}</option><option value="missing">{t('缺失分量演示', 'Missing component demo')}</option><option value="negative">{t('负差值演示', 'Negative residual demo')}</option><option value="empty">{t('空数据演示', 'Empty demo')}</option></select></label></div>
          <div className="page-heading"><div><p className="eyebrow">USAGE INTELLIGENCE</p><h1>{title}</h1><p className="muted">{t('在一个工作空间里看清来源、范围与质量。', 'Understand sources, scope and quality in one workspace.')}</p></div><div className="button-group"><button className="button" disabled={busy || syncing} onClick={() => setRevision(value => value + 1)}><Icon name="refresh" />{t('刷新视图', 'Refresh view')}</button>{observed && transport?.supportsLocalSync && <button className="button" disabled={busy || syncing} onClick={() => void synchronize(transport)}>{t('同步本机日志', 'Sync local logs')}</button>}{syncing && <button className="button" onClick={() => void transport?.cancelLocalSync()}>{t('取消同步', 'Cancel sync')}</button>}</div></div>
          {page !== 'settings' && <div className="monitor-toolbar"><label>{t('服务商', 'Provider')}<select aria-label={t('服务商', 'Provider')} value={provider} onChange={event => setProvider(event.target.value)}>{(data?.providers ?? [{ providerId: 'codex', displayName: 'Codex' }]).map(item => <option key={item.providerId} value={item.providerId}>{item.displayName}</option>)}</select></label>
            {observed && <><label>{t('日志账号范围', 'Logged account scope')}<select aria-label={t('日志账号范围', 'Logged account scope')} value={selectedAccount} disabled={!localAccounts.length} onChange={event => setAccount(event.target.value)}>{localAccounts.length ? [<option key="all" value="*">{t('所有日志账号（未核实）', 'All logged accounts (unverified)')}</option>, ...localAccounts.map(value => <option key={value} value={value}>{value === 'unmarked' ? t('未标记账号', 'Unmarked account') : `${t('日志账号', 'Logged account')} ${value.slice(-12)}`}</option>)] : <option value="unmarked">{t('未知', 'Unknown')}</option>}</select></label><label>{t('执行范围', 'Execution scope')}<select aria-label={t('执行范围', 'Execution scope')} value={selectedProduct} disabled={!localProducts.length} onChange={event => setProduct(event.target.value)}>{localProducts.length ? localProducts.map(value => <option value={value} key={value}>{value === 'codex-subagent' ? t('子代理（单独统计）', 'Subagents (separate)') : t('主线程日志', 'Main thread logs')}</option>) : <option value="codex-local">{t('主线程日志', 'Main thread logs')}</option>}</select></label></>}
            {observed && page === 'history' && accountProducts.length > 0 && <label>{t('账号服务端产品范围', 'Account server product scope')}<select aria-label={t('账号服务端产品范围', 'Account server product scope')} value={selectedAccountProduct} onChange={event => setAccountProduct(event.target.value)}>{accountProducts.map(value => <option key={value} value={value}>{value === 'codex-account' ? t('账号总范围', 'Account total scope') : value}</option>)}</select></label>}{(page === 'overview' || page === 'quality' || page === 'history') && <label>{t('日志 UTC 日期', 'Log UTC date')}<select aria-label={t('日志 UTC 日期', 'Log UTC date')} value={date} disabled={!dates.length} onChange={event => setSelectedDate(event.target.value)}>{dates.length ? dates.map(day => <option key={day}>{day}</option>) : <option value="">{t('无日期', 'No dates')}</option>}</select></label>}
            <div className="account-caption"><span>{observed ? t('账号归属未核实', 'Account identity unverified') : 'demo-account-a'} · {selectedProduct}</span><span>{observed ? t('上次本地检查', 'Last local check') : t('演示采集', 'Demo collection')}: {formatTime(collectionTime, timezone, locale)} · {timezone}</span></div>
          </div>}
          {(busy || syncing || accountBusy) && <div className="small muted loading-inline" role="status">{accountBusy ? t('正在查询账号用量和额度…', 'Querying account usage and quotas…') : syncing ? t('正在增量同步；已缓存账本仍可查看…', 'Incremental sync in progress; cached ledgers remain available…') : t('正在读取缓存账本…', 'Reading cached ledgers…')}</div>}
          {error && <Notice tone={error.code === 'CANCELLED' ? 'warning' : 'error'} title={error.code === 'CANCELLED' ? t('同步已取消', 'Synchronization cancelled') : t('账本操作未完成', 'Ledger operation incomplete')}><p>{t('已提交的本机事件与检查点保留，可再次同步继续处理。', 'Committed events and checkpoints are retained. Synchronize again to resume.')}</p><code>{error.code}</code></Notice>}
          {props && <>{page === 'overview' && <Overview {...props} />}{page === 'history' && <History {...props} />}{page === 'sessions' && <Sessions {...props} />}{page === 'quality' && <Quality {...props} />}{page === 'settings' && observed && <DeviceTransfer data={data!} client={transport ?? undefined} onChanged={async () => { if (transport) setData(await transport.getDashboard({ scenario: 'local' })); }} />}{page === 'settings' && <Settings data={props.data} timezone={timezone} setTimezone={setTimezone} onSetRoots={transport?.supportsLocalSync ? roots => transport.setLocalRoots(roots) : undefined} onSetAccountQueries={observed && transport?.supportsAccountSync ? configureAccountQueries : undefined} onSyncAccount={observed && transport?.supportsAccountSync ? (request: AccountSyncRequest) => accountOperation(value => value.syncAccount(request)) : undefined} accountBusy={accountBusy} />}</>}
          <footer className="monitor-footer"><span>{t('账号额度独立显示；设备 Token 来自各自日志。', 'Account quota is separate; device Tokens come from each device’s logs.')}</span><span>Agent Usage Monitor · 0.4.0</span></footer>
        </main>
      </div>
    </div>
  </>;
}
