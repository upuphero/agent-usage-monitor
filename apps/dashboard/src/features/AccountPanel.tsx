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
  return <Notice title={t(...label)}><p>{t('账号额度由登录账号共享。Windows / Mac 的 Token 请看设备日志统计。', 'Account quota is shared by the signed-in account. See device logs for Windows / Mac Tokens.')}</p><details><summary>{t('账号查询详情', 'Account query details')}</summary><p>{t('账号历史按服务器返回的日期保存，在使用历史中单独查看。服务器的客户端分类不能确定执行设备。', 'Account history retains server date labels and appears separately in usage history. Server client categories do not identify execution devices.')}</p>
    <p className="small">{t('匿名账号', 'Anonymous account')}: {account.accountRef?.slice(-12) ?? '—'} · {t('历史采集', 'History captured')}: {formatTime(account.historyFetchedAt, timezone, locale)} · {t('额度采集', 'Quota captured')}: {formatTime(account.quotaFetchedAt, timezone, locale)} · {timezone}</p>
    {scopeMissing.length > 0 && <p>{t('最近请求中当前产品范围未返回的日期', 'Dates absent for this product scope in the latest request')}: {scopeMissing.join(', ')} · {t('缺失不补零；旧快照保留', 'Missing is not zero; prior snapshots retained')}</p>}
    {(account.historyError || account.quotaError) && <p className="small">{t('历史状态', 'History status')}: {account.historyError ?? 'OK'} · {t('额度状态', 'Quota status')}: {account.quotaError ?? '—'}</p>}
    {account.nextAllowedAt && <p className="small">{t('下次允许请求', 'Next request allowed')}: {formatTime(account.nextAllowedAt, timezone, locale)}</p>}
  </details></Notice>;
}

export interface AccountActions { onSetAccountQueries?: (enabled: boolean) => Promise<void>; onSyncAccount?: (request: AccountSyncRequest) => Promise<void>; accountBusy?: boolean }
export function AccountQuerySwitch({ data, onSetAccountQueries }: { data: Dashboard } & AccountActions) {
  const { t } = useI18n(); const [saving, setSaving] = useState(false); const [error, setError] = useState('');
  const enabled = data.account?.enabled === true;
  return <div className="setting-row"><div><h3>{t('账号查询', 'Account queries')}</h3><p className="muted">{t('开启后，启动时自动读取账号用量和额度。关闭后保留已有数据。', 'When enabled, account usage and quotas sync on startup. Turning it off keeps existing data.')}</p>{error && <p role="alert" className="small">{error}</p>}</div>
    <button className="account-query-switch" role="switch" aria-label={t('账号查询', 'Account queries')} aria-checked={enabled} disabled={!onSetAccountQueries || saving} onClick={() => {
      if (!onSetAccountQueries) return;
      setSaving(true); setError('');
      void onSetAccountQueries(!enabled).catch(failure => setError(`${t('设置未保存', 'Setting not saved')}: ${safeError(failure).code}`)).finally(() => setSaving(false));
    }}><span className="account-switch-track" aria-hidden="true"><i /></span><span>{saving ? t('保存中', 'Saving') : enabled ? t('开启', 'On') : t('关闭', 'Off')}</span></button>
  </div>;
}
export function AccountSettings({ data, onSyncAccount, accountBusy = false }: { data: Dashboard } & AccountActions) {
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
    <p className="small muted">{t('开启账号查询后，启动时自动同步最近七天。这里可以选择其他日期，手动更新账号数据。使用非公开接口，可能变化或受账号权限、服务条款限制。', 'With account queries enabled, the last seven days sync on startup. Choose other dates here to update account data manually. These private endpoints may change or be restricted by account permissions and service terms.')}</p>
    {onSyncAccount ? <>
      {!enabled && <p className="small muted">{t('账号查询已关闭。请使用上方“账号查询”开关开启。', 'Account queries are off. Use the Account queries switch above to enable them.')}</p>}
      <div className="monitor-toolbar"><label>{t('开始日期标签', 'Start date label')}<input type="date" value={startDate} disabled={busy || accountBusy} onChange={event => setStartDate(event.target.value)} /></label><label>{t('结束日期标签（包含）', 'End date label (inclusive)')}<input type="date" value={endDate} disabled={busy || accountBusy} onChange={event => setEndDate(event.target.value)} /></label><button className="button" disabled={!enabled || busy || accountBusy || !validRange} onClick={() => void run(() => onSyncAccount({ startDate, endDate }))}>{busy || accountBusy ? t('正在查询账号…', 'Querying account…') : t('同步账号数据', 'Sync account data')}</button></div>
      <p className="panel-footnote">{t('单次最多 31 个日期；每次请求至少间隔 60 秒，服务端限流可能要求更久。历史与额度状态分别记录，刷新视图只读缓存。', 'At most 31 dates per sync; requests are at least 60 seconds apart and server rate limits may require longer. History and quota statuses are separate. Refresh view reads cache only.')}</p>
    </> : <p>{t('浏览器预览只读；手动账号同步需要桌面端。', 'Browser previews are read-only; manual account synchronization requires desktop.')}</p>}
    {status && <p role="status" className="small">{status}</p>}
  </Panel>;
}
