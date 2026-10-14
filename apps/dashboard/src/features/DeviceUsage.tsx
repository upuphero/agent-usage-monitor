import { useRef, useState } from 'react';
import type { Dashboard, DeviceDay, DevicePlatform, TokenComponents, LedgerRow, Metric } from '../api/generated/monitor';
import type { UsageClient } from '../api/client';
import { safeError } from '../api/client';
import { formatCount, formatTime } from '../components/format';
import { MetricCard, Panel, Notice } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';

export function sumDeviceMetric(rows: DeviceDay[], key: keyof TokenComponents): string | null {
  if (!rows.length || rows.some(row => row.tokens[key].value === null)) return null;
  const sum = rows.reduce((sum, row) => sum + BigInt(row.tokens[key].value!), 0n);
  return sum <= 9223372036854775807n ? sum.toString() : null;
}
export function combineLocalDays(rows: LedgerRow[]): LedgerRow[] {
  const groups = new Map<string, LedgerRow[]>();
  for (const row of rows) {
    const group = `${row.scope.providerId}/${row.scope.productId}/${row.day.dateLabel}`;
    groups.set(group, [...(groups.get(group) ?? []), row]);
  }
  return [...groups.values()].map(group => {
    const metric = (key: keyof TokenComponents): Metric<string> => {
      const values = group.map(row => row.tokens[key].value);
      if (values.some(value => value === null)) return { value: null, quality: 'unknown', reasonCodes: ['MISSING_COMPONENT'] };
      const value = values.reduce<bigint>((sum, value) => sum + BigInt(value!), 0n);
      return value > 9223372036854775807n ? { value: null, quality: 'unknown', reasonCodes: ['INVALID_TOKEN'] } : { value: value.toString(), quality: 'partial', reasonCodes: ['COVERAGE_INCOMPLETE'] };
    };
    return { ...group[0], scope: { ...group[0].scope, accountRef: null, identityEvidence: 'unknown' },
      tokens: { uncachedInput: metric('uncachedInput'), cachedInput: metric('cachedInput'), inclusiveInput: metric('inclusiveInput'), output: metric('output'), reasoningOutput: metric('reasoningOutput'), textTotal: metric('textTotal') } };
  });
}
export const platformLabel = (platform: DevicePlatform) => platform === 'macos' ? 'Mac' : platform === 'windows' ? 'Windows' : platform === 'linux' ? 'Linux' : 'Unknown';
export function DeviceUsagePanel({ data, date, timezone }: { data: Dashboard; date: string; timezone: string }) {
  const { t, locale } = useI18n();
  const usage = data.deviceUsage;
  if (!usage) return null;
  const daily = usage.daily.filter(row => row.dateLabel === date);
  const platformRows = (platform: DevicePlatform) => {
    const ids = new Set(usage.devices.filter(item => item.device.platform === platform).map(item => item.device.deviceRef));
    return daily.filter(row => row.deviceRef !== null && ids.has(row.deviceRef));
  };
  return <Panel title={t('设备 Token', 'Device Tokens')} eyebrow={`${date || '—'} · UTC`}>
    <p className="small muted">{t('Windows 和 Mac 各自读取日志。这里统计日志中可计入的 Token；账号额度单独显示。', 'Windows and Mac read their own logs. These are countable log Tokens; account quota is displayed separately.')}</p>
    <div className="metrics-grid monitor-metrics">{(['windows', 'macos'] as const).map(platform => {
      const present = usage.devices.some(item => item.device.platform === platform && item.lastSyncAt !== null);
      const rows = platformRows(platform);
      return <MetricCard key={platform} label={`${platformLabel(platform)} Token`} value={sumDeviceMetric(rows, 'textTotal')} accent={platform === 'windows'} status={present ? t('已采集日志', 'Logs collected') : t('未接入', 'Not connected')} note={present ? rows.length ? t('日志所覆盖部分', 'Portion covered by logs') : t('所选日期没有可计入记录', 'No countable records on this date') : platform === 'macos' ? t('在 Mac 采集后导入报告', 'Collect on Mac, then import the report') : t('同步 Windows 本机日志', 'Sync Windows local logs')} />;
    })}<MetricCard label={t('设备日志去重汇总', 'Deduplicated device logs')} value={sumDeviceMetric(daily.filter(row => row.deviceRef === null), 'textTotal')} status={t('各设备报告中的已观测部分', 'Observed portion of device reports')} note={t('相同记录只计一次；冲突记录不计入', 'Identical records count once; conflicting records are excluded')} /></div>
    <div className="table-scroll"><table><thead><tr>{[t('设备', 'Device'), t('最近采集', 'Last collection'), t('报告记录', 'Report records'), t('未计入记录', 'Excluded records')].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{usage.devices.map(item => <tr key={item.device.deviceRef}><td>{platformLabel(item.device.platform)} <span className="small muted">{item.device.deviceRef.slice(-8)}</span></td><td>{formatTime(item.lastSyncAt, timezone, locale)}</td><td>{formatCount(String(item.records), locale)}</td><td>{formatCount(String(item.excludedEvents), locale)}</td></tr>)}</tbody></table></div>
    <p className="panel-footnote">{t('报告范围内跨设备重复记录', 'Cross-device duplicate records in reports')}: {usage.duplicateEvents} · {t('冲突记录', 'Conflicting records')}: {usage.conflictEvents}. {t('复制过的日志可能出现在两台设备中，因此汇总可能小于设备数字之和。设备标签表示采集地点，不能证明执行地点。', 'Copied logs may appear on both devices, so the union can be smaller than the sum of device figures. Device labels identify collection, not execution.')}</p>
  </Panel>;
}
export function DeviceTransfer({ data, client, onChanged }: { data: Dashboard; client?: UsageClient; onChanged: () => Promise<void> }) {
  const { t } = useI18n();
  const fileInput = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [failed, setFailed] = useState(false);
  async function operation(action: () => Promise<void>, message: string) {
    setBusy(true); setStatus(''); setFailed(false);
    try { await action(); await onChanged(); setStatus(message); }
    catch (failure) { setFailed(true); setStatus(`${t('报告操作失败，已有记录保留', 'Report operation failed; existing records retained')} · ${safeError(failure).code}`); }
    finally { setBusy(false); }
  }
  return <Panel title={t('Windows / Mac 日志汇总', 'Windows / Mac log collection')}>
    <p>{t('Windows 点击“同步本机日志”。Mac 下载 Release 中对应芯片的采集器，运行下面的命令，再把生成的 JSON 文件复制到这里导入。每次导入会更新该设备报告，不会重复追加。', 'Sync local logs on Windows. Download the collector for your Mac chip from the Release, run the command below, then copy and import the JSON here. Each import updates that device report without adding another copy.')}</p>
    <pre className="collector-command"><code>./monitor-collector collect --output "mac-usage-$(date +%Y%m%d-%H%M%S).json"</code></pre>
    <p className="small muted">{t('采集器需要在终端运行；报告不含对话正文、工具输入或登录信息。设备身份保存在采集器缓存里，更新时请保留该缓存。报告包含用量信息，请自行传送，不会自动上传。', 'Run the collector in Terminal. Reports omit conversation text, tool inputs and login information. Keep its cache when upgrading to preserve device identity. Reports contain usage information; transfer them yourself. They are never uploaded automatically.')}</p>
    <div className="button-group"><button className="button" disabled={busy || !client?.exportDeviceReport} onClick={() => void operation(async () => {
      const json = await client!.exportDeviceReport!();
      const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      const link = document.createElement('a'); link.href = url; link.download = `device-usage-${new Date().toISOString().replaceAll(':', '-')}.json`; link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    }, t('本机统计报告已导出', 'Local report exported'))}>{t('导出本机报告', 'Export local report')}</button><button className="button" disabled={busy || !client?.importDeviceReport} onClick={() => fileInput.current?.click()}>{t('导入设备报告', 'Import device report')}</button></div>
    <input ref={fileInput} type="file" accept=".json,application/json" hidden aria-label={t('选择设备报告', 'Choose device report')} onChange={event => {
      const file = event.target.files?.[0]; event.target.value = '';
      if (file) void operation(async () => {
        if (file.size > 32 * 1024 * 1024) throw { code: 'INVALID_DATA' };
        await client!.importDeviceReport!(await file.text());
      }, t('设备报告已更新', 'Device report updated'));
    }} />
    {(data.deviceUsage?.devices ?? []).filter(item => item.device.deviceRef !== data.deviceUsage?.devices[0]?.device.deviceRef).map(item => <div className="setting-row" key={item.device.deviceRef}><span>{platformLabel(item.device.platform)} · {item.device.deviceRef.slice(-8)}</span><button className="button button-small" disabled={busy || !client?.removeDeviceReport} onClick={() => void operation(() => client!.removeDeviceReport!(item.device.deviceRef), t('已移除导入报告，可重新导入', 'Imported report removed; it can be imported again'))}>{t('移除导入报告', 'Remove imported report')}</button></div>)}
    {status && <Notice tone={failed ? 'error' : 'info'}>{status}</Notice>}
  </Panel>;
}
