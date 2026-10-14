import { useEffect, useRef, useState } from 'react';
import { useI18n } from '../i18n/I18nContext';
import { safeError } from '../api/client';

export function AccountNotice({ onContinue }: { onContinue: (enabled: boolean) => Promise<void> }) {
  const { t } = useI18n(); const dialog = useRef<HTMLDialogElement>(null);
  const continueButton = useRef<HTMLButtonElement>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  useEffect(() => { if (dialog.current && !dialog.current.open) { dialog.current.showModal(); continueButton.current?.focus(); } }, []);
  async function finish(enabled: boolean) {
    setBusy(true); setError('');
    try { await onContinue(enabled); }
    catch (failure) { setError(`${t('设置未保存，请重试', 'Settings not saved; try again')}: ${safeError(failure).code}`); setBusy(false); }
  }
  return <dialog ref={dialog} className="account-notice-dialog" aria-labelledby="account-notice-title" onCancel={event => { event.preventDefault(); if (!busy) void finish(true); }}>
    <h2 id="account-notice-title">{t('自动读取账号用量', 'Read account usage automatically')}</h2>
    <p>{t('账号查询默认开启。启动软件时，会使用你在官方 Codex 的现有登录，向 chatgpt.com 查询账号用量和额度；数据只保存在这台电脑。', 'Account queries are enabled by default. On startup, the app uses your existing official Codex login to request usage and quotas from chatgpt.com. Data stays on this computer.')}</p>
    <p>{t('不想查询账号，可以现在关闭，也可以随时在设置里的“账号查询”开关关闭。关闭后仍能查看本机统计和已有缓存。', 'You can turn this off now or later using Account queries in Settings. Local statistics and existing cache remain available.')}</p>
    <p className="small muted">{t('使用非公开接口，可能变化或受账号权限、服务条款限制。不会自行刷新登录，也不会在软件关闭后继续采样。', 'These private endpoints may change or be restricted by account permissions and service terms. The app does not refresh your login or collect data after it closes.')}</p>
    <div className="button-group"><button className="button" disabled={busy} onClick={() => void finish(false)}>{t('关闭账号查询', 'Turn account queries off')}</button><button ref={continueButton} className="button account-continue" disabled={busy} onClick={() => void finish(true)}>{busy ? t('正在保存…', 'Saving…') : t('开始使用（保持开启）', 'Continue with queries enabled')}</button></div>
    {error && <p role="alert" className="small">{error}</p>}
  </dialog>;
}
