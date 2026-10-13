import { useState } from 'react';
import type { AccountSummary, AccountSyncRequest, Dashboard } from '../api/generated/monitor';
import { formatTime } from '../components/format';
import { Notice, Panel } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';
import { safeError } from '../api/client';

export function AccountStatus({ data, timezone }: { data: Dashboard; timezone: string }) {
  const { t, locale } = useI18n(); const account = data.account;
  if (data.dataMode !== 'observed' || !account) return null;
  const states: Record<AccountSummary['state'], [string, string]> = {
    disabled: ['账号查询关闭', 'Account queries disabled'], 'never-synced': ['尚未同步账号', 'Account not synchronized'],
    cached: ['显示账号缓存', 'Showing account cache'], ready: ['账号快照已采集', 'Account snapshots captured'],
    partial: ['账号同步部分完成', 'Account sync partially complete'], 'auth-required': ['请通过官方 Codex 登录', 'Sign in using official Codex'],
    'access-denied': ['账号接口无访问权限', 'Account endpoint access denied'], 'endpoint-unavailable': ['账号接口不可用', 'Account endpoint unavailable'],
    'rate-limited': ['账号请求受限，请稍后手动重试', 'Account requests limited; retry manually later'], offline: ['账号请求离线，显示已有缓存', 'Account offline; showing cached observations'],
    incompatible: ['账号响应格式不兼容', 'Account response incompatible']
  };
  const label = states[account.state] ?? states.incompatible;
  const rangeStart = Date.parse(account.requestedStart ?? ''), rangeEnd = Date.parse(account.requestedEnd ?? '');
  const successfulRange = account.historyError === null && account.historyFetchedAt !== null && Number.isFinite(rangeStart) && Number.isFinite(rangeEnd) && rangeEnd >= rangeStart && rangeEnd - rangeStart < 31 * 86400000;
  const returnedDates = new Set(data.accountRows.filter(row => Date.parse(row.source.collectedAt) === Date.parse(account.historyFetchedAt!)).map(row => row.day.dateLabel));
  const scopeMissing = successfulRange ? Array.from({ length: (rangeEnd - rangeStart) / 86400000 + 1 }, (_, index) => new Date(rangeStart + index * 86400000).toISOString().slice(0, 10)).filter(date => !returnedDates.has(date)) : account.missingDates;
  return <Notice title={t(...label)}><p>{t('服务端日期标签、产品覆盖和回填水位尚未验证；客户端应用类型不能区分 Windows 与 Mac。', 'Server date labels, product coverage and backfill finality are unverified. Client application types cannot distinguish Windows from Mac.')}</p>
    <p className="small">{t('匿名账号', 'Anonymous account')}: {account.accountRef?.slice(-12) ?? '—'} · {t('历史采集', 'History captured')}: {formatTime(account.historyFetchedAt, timezone, locale)} · {t('额度采集', 'Quota captured')}: {formatTime(account.quotaFetchedAt, timezone, locale)} · {timezone}</p>
    {scopeMissing.length > 0 && <p>{t('最近请求中当前产品范围未返回的日期', 'Dates absent for this product scope in the latest request')}: {scopeMissing.join(', ')} · {t('缺失不补零；旧快照保留', 'Missing is not zero; prior snapshots retained')}</p>}
    {(account.historyError || account.quotaError) && <p className="small">{t('历史状态', 'History status')}: {account.historyError ?? 'OK'} · {t('额度状态', 'Quota status')}: {account.quotaError ?? '—'}</p>}
    {account.nextAllowedAt && <p className="small">{t('下次允许请求', 'Next request allowed')}: {formatTime(account.nextAllowedAt, timezone, locale)}</p>}
  </Notice>;
}

export interface AccountActions { onSetAccountQueries?: (enabled: boolean) => Promise<void>; onSyncAccount?: (request: AccountSyncRequest) => Promise<void> }
export function AccountSettings({ data, onSetAccountQueries, onSyncAccount }: { data: Dashboard } & AccountActions) {
  const { t } = useI18n(); const today = new Date().toISOString().slice(0, 10);
  const [startDate, setStartDate] = useState(() => new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10));
  const [endDate, setEndDate] = useState(today); const [busy, setBusy] = useState(false); const [status, setStatus] = useState('');
  const enabled = data.account?.enabled === true;
  const span = (Date.parse(endDate) - Date.parse(startDate)) / 86400000;
  const validRange = Number.isInteger(span) && span >= 0 && span < 31;
  async function run(action: () => Promise<void>) {
    setBusy(true); setStatus('');
    try { await action(); setStatus(t('操作已完成，请查看账号同步状态', 'Operation completed; review account sync status')); }
    catch (failure) { setStatus(`${t('操作未完成，缓存保留', 'Operation incomplete; cache retained')}: ${safeError(failure).code}`); }
    finally { setBusy(false); }
  }
  return <Panel title={t('账号历史与额度', 'Account history and quotas')}>
    <p className="small muted">{t('启用后，点击“同步账号快照”会在内存中读取官方 Codex 已有登录并向 chatgpt.com 查询历史和额度。使用非公开接口，可能变化或受账号权限、服务条款限制。默认关闭；不会自动刷新登录或定时请求。', 'After enabling, Sync account snapshots reads the existing official Codex login in memory and queries chatgpt.com for history and quotas. These private endpoints may change or be restricted by account permissions and service terms. Disabled by default; no login refresh or scheduled requests.')}</p>
    {onSetAccountQueries && onSyncAccount ? <>
      <label className="setting-row"><span>{t('允许手动查询账号历史与额度', 'Allow manual account history and quota queries')}</span><input type="checkbox" checked={enabled} disabled={busy} onChange={event => { const checked = event.target.checked; void run(() => onSetAccountQueries(checked)); }} /></label>
      <div className="monitor-toolbar"><label>{t('开始日期标签', 'Start date label')}<input type="date" value={startDate} disabled={busy} onChange={event => setStartDate(event.target.value)} /></label><label>{t('结束日期标签（包含）', 'End date label (inclusive)')}<input type="date" value={endDate} disabled={busy} onChange={event => setEndDate(event.target.value)} /></label><button className="button" disabled={!enabled || busy || !validRange} onClick={() => void run(() => onSyncAccount({ startDate, endDate }))}>{busy ? t('操作进行中…', 'Working…') : t('同步账号快照', 'Sync account snapshots')}</button></div>
      <p className="panel-footnote">{t('单次最多 31 个日期；每次请求至少间隔 60 秒，服务端限流可能要求更久。历史与额度状态分别记录，刷新视图只读缓存。', 'At most 31 dates per sync; requests are at least 60 seconds apart and server rate limits may require longer. History and quota statuses are separate. Refresh view reads cache only.')}</p>
    </> : <p>{t('浏览器预览只读；手动账号同步需要桌面端。', 'Browser previews are read-only; manual account synchronization requires desktop.')}</p>}
    {status && <p role="status" className="small">{status}</p>}
  </Panel>;
}
