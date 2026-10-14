import type { Dashboard, LedgerRow } from '../api/generated/monitor';
import { formatCount, formatTime, latestRows, percentOf } from '../components/format';
import { EmptyState, MetricCard, Notice, Panel, ReasonList } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';
import { TrendChart } from './TrendChart';
import { useState } from 'react';
import { DeviceUsagePanel } from './DeviceUsage';
import { AccountSettings, AccountStatus, AccountQuerySwitch, type AccountActions } from './AccountPanel';

export type Page = 'overview' | 'history' | 'sessions' | 'quality' | 'settings';
export interface PageProps { data: Dashboard; provider: string; date: string; timezone: string }
const scopeRows = (rows: LedgerRow[], provider: string) => rows.filter(row => row.scope.providerId === provider);
export function selectDay(data: Dashboard, provider: string, date: string) {
  const accounts = latestRows(scopeRows(data.accountRows, provider)).filter(row => row.day.dateLabel === date);
  const locals = scopeRows(data.localRows, provider).filter(row => row.day.dateLabel === date);
  const account = accounts.length === 1 ? accounts[0] : undefined;
  const local = locals.length === 1 ? locals[0] : undefined;
  const comparison = local && account ? data.comparisons.find(item => item.dateLabel === date && item.scope.providerId === provider && item.accountRowId === account.rowId && item.localRowId === local.rowId) : undefined;
  return { account, local, comparison };
}
function Quota({ data, provider, timezone }: PageProps) {
  const { t, locale } = useI18n();
  const quota = data.quota?.scope.providerId === provider ? data.quota : null;
  return <div className="quota-strip">{(['five-hour', 'weekly'] as const).map(kind => {
    const window = quota?.windows.find(item => item.kind === kind);
    return <div className="quota-item" key={kind}><span className="small muted">{kind === 'five-hour' ? t('5 小时窗口', '5-hour window') : t('每周窗口', 'Weekly window')}</span><strong>{!quota && data.dataMode === 'observed' ? t('账号额度尚无快照', 'No account quota snapshot') : window?.usedPercent == null ? t('未报告', 'Not reported') : `${window.usedPercent}% ${t('已用', 'used')}`}</strong><span className="small muted">{window?.resetsAt ? `${data.dataMode === 'observed' ? t('快照重置', 'Snapshot reset') : t('演示重置', 'Demo reset')}: ${formatTime(window.resetsAt, timezone, locale)} · ${timezone}` : t('重置时间未知', 'Reset time unknown')}</span>{quota && data.dataMode === 'observed' && <span className="small muted">{window?.reasonCodes.includes('QUOTA_STALE') ? t('过期缓存', 'Stale cache') : t('采集快照', 'Captured snapshot')} · {formatTime(quota.source.collectedAt, timezone, locale)}</span>}</div>;
  })}</div>;
}
function Composition({ row, local = false }: { row?: LedgerRow; local?: boolean }) {
  const { t, locale } = useI18n();
  const parts = [
    ['uncachedInput', t('未缓存输入', 'Uncached input')], ['cachedInput', t('缓存输入', 'Cached input')], ['output', t('输出', 'Output')]
  ] as const;
  const total = row?.tokens.textTotal.value ?? null;
  return <Panel title={local ? t('本机 Token 组成', 'Local Token composition') : t('账号 Token 组成', 'Account Token composition')} eyebrow={t('缓存只计一次', 'Cache counted once')}>
    {!row ? <EmptyState /> : <><div className="composition-track" aria-hidden="true">{parts.map(([key], i) => <span key={key} className={`composition-part color-${i}`} style={{ width: `${total === null || row.tokens[key].value === null ? 0 : percentOf(row.tokens[key].value, total)}%` }} />)}</div><div className="distribution-list">{parts.map(([key, label], i) => <div className="distribution-label" key={key}><i className={`chart-dot color-${i}`} /><span>{label}</span><span className="distribution-value">{formatCount(row.tokens[key].value, locale)}</span></div>)}</div><p className="panel-footnote">{t('文本总量 = 未缓存输入 + 缓存输入 + 输出；reasoning 是输出子集。', 'Text total = uncached input + cached input + output. Reasoning is a subset of output.')}</p></>}
  </Panel>;
}
export function Overview(props: PageProps) {
  const { t } = useI18n();
  const { local } = selectDay(props.data, props.provider, props.date);
  const observed = props.data.dataMode === 'observed';
  return <div className="page-stack">
    <Panel title={t('账号额度', 'Account quota')}><p className="small muted">{t('登录账号在各设备间共享的额度。百分比和重置时间来自账号查询，与下面的日志 Token 分开统计。', 'Quota shared by the signed-in account across devices. Percentages and resets come from account queries, separately from log Tokens below.')}</p><Quota {...props} /><AccountStatus data={props.data} timezone={props.timezone} /></Panel>
    {props.provider === 'codex' && props.data.deviceUsage ? <DeviceUsagePanel data={props.data} date={props.date} timezone={props.timezone} /> : <MetricCard label={t('Windows 已观测部分', 'Windows observed portion')} value={local?.tokens.textTotal.value ?? null} accent status={t('本地日志 · 覆盖未核实', 'Local logs · Coverage unverified')} note={`${props.date || '—'} UTC`} />}
    <div className="monitor-overview-grid"><TrendChart account={[]} local={scopeRows(props.data.localRows, props.provider)} observed={observed} /><Composition row={local} local /></div>
    <Notice title={t('统计范围', 'What is counted')}>{t('本机 Token 来自日志所覆盖的记录。缺失或无法核对的记录不计入；账号额度不能换算成 Token，也不用于计算另一台设备的用量。图表悬停只查看该点，右侧组成对应上方所选 UTC 日期。', 'Local Tokens come from available log records. Missing or unverifiable records are excluded. Account quota is not converted to Tokens or another device’s usage. Hover inspects a chart point; composition follows the selected UTC date above.')}</Notice>
  </div>;
}
export function History(props: PageProps) {
  const { t, locale } = useI18n();
  const account = scopeRows(props.data.accountRows, props.provider), local = scopeRows(props.data.localRows, props.provider);
  return <div className="page-stack">{props.provider === 'codex' && props.data.deviceUsage && <DeviceUsagePanel data={props.data} date={props.date} timezone={props.timezone} />}<TrendChart account={[]} local={local} observed={props.data.dataMode === 'observed'} /><details className="account-history-details"><summary>{t('账号历史记录（独立来源）', 'Account history (separate source)')}</summary><Panel title={t('账号追加快照', 'Append-only account snapshots')} eyebrow={t('同日旧版本保留', 'Earlier revisions retained')}>
    {!account.length ? <EmptyState /> : <div className="table-scroll"><table><thead><tr>{[t('统计日期', 'Date label'), t('采集时间', 'Collected at'), t('未缓存输入', 'Uncached input'), t('缓存输入', 'Cached input'), t('输出', 'Output'), t('文本总量', 'Text total')].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{account.map(row => <tr key={row.rowId}><td>{row.day.dateLabel}<span className="cell-subtitle">{row.day.basis}</span></td><td>{formatTime(row.source.collectedAt, props.timezone, locale)}<span className="cell-subtitle">{props.timezone}</span></td>{(['uncachedInput', 'cachedInput', 'output', 'textTotal'] as const).map(key => <td key={key} className="numeric">{formatCount(row.tokens[key].value, locale)}</td>)}</tr>)}</tbody></table></div>}
    <p className="panel-footnote">{props.data.dataMode === 'observed' ? t('每个日期和产品范围使用最新快照；旧版本保留且不相加。服务端日期标签尚未验证为 UTC 日。', 'Each date and product scope uses its latest snapshot; prior revisions are retained and never summed. Server labels are not verified UTC days.') : t('趋势使用每个日期最新快照；快照版本不相加。演示日期不代表当前账号用量。', 'The trend uses the latest snapshot for each date; revisions are not summed. Demo dates are not current account usage.')}</p>
  </Panel></details>{props.data.dataMode === 'observed' && <><AccountStatus data={props.data} timezone={props.timezone} /><Panel title={t('本机 UTC 日账本', 'Local UTC daily ledger')}><DailyTable rows={local} locale={locale} /><p className="panel-footnote">{t('已验证增量按 UTC 日归档。缺失与歧义区间保留原因，不按累计总量或平均分摊填补。', 'Validated increments are bucketed by UTC day. Missing and ambiguous intervals are explained, never filled from cumulative totals or averages.')}</p></Panel></>}</div>;
}
export function Sessions(props: PageProps) {
  const { t, locale } = useI18n();
  const models = props.data.models.filter(row => row.scope.providerId === props.provider), sessions = props.data.sessions.filter(row => row.scope.providerId === props.provider);
  const [visible, setVisible] = useState(100);
  const observed = props.data.dataMode === 'observed';
  const maximum = models.reduce((max, model) => model.textTotal.value !== null && BigInt(model.textTotal.value) > BigInt(max) ? model.textTotal.value : max, '0');
  return <div className="page-stack"><Notice title={t('仅 Windows 本机', 'Windows local only')}>{observed ? t('模型与 effort 来自 turn_context 配置，speed 缺失保持未知。子代理、账号分别筛选，不代表远程模型或已校准的完整消耗。', 'Model and effort come from turn_context configuration; absent speed remains unknown. Accounts and subagents are scoped separately, without remote model or calibrated completeness claims.') : t('模型与会话来自七个演示日的合成本地统计。远程模型未知。', 'Models and sessions come from synthetic local statistics across seven demo days. Remote models are unknown.')}</Notice><Panel title={observed ? t('本机模型配置统计', 'Local model configuration statistics') : t('本机模型', 'Local models')}>
    {!models.length ? <EmptyState /> : <div className="distribution-list">{models.map((model, i) => <div key={model.modelId}><div className="distribution-label"><i className={`chart-dot color-${i}`} /><span>{model.modelId === 'unknown' ? t('未知模型', 'Unknown model') : model.modelId}</span><span className="distribution-value">{formatCount(model.textTotal.value, locale)}</span></div><div className="bar-track"><div className={`bar-fill color-${i}`} style={{ width: `${model.textTotal.value === null ? 0 : percentOf(model.textTotal.value, maximum)}%` }} /></div><span className="small muted">{model.sessions} {observed ? t('个日志线程', 'logged threads') : t('个合成会话', 'synthetic sessions')}</span></div>)}</div>}
  </Panel><Panel title={t('本机会话', 'Local sessions')}>
    {!sessions.length ? <EmptyState /> : <><div className="table-scroll"><table><thead><tr>{[t('会话/配置段', 'Session/configuration segment'), t('模型', 'Model'), 'Effort', 'Speed', 'Token'].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{sessions.slice(0, visible).map(session => <tr key={session.sessionRef}><td title={session.sessionRef}>{observed ? session.sessionRef.slice(-16) : session.sessionRef}</td><td>{session.modelId ?? t('未知', 'Unknown')}</td><td>{session.effort ?? t('未知', 'Unknown')}</td><td>{session.speed ?? t('未知', 'Unknown')}</td><td className="numeric">{formatCount(session.textTotal.value, locale)}</td></tr>)}</tbody></table></div>{visible < sessions.length && <button className="button button-small" onClick={() => setVisible(value => value + 100)}>{t('显示更多会话', 'Show more sessions')} ({visible}/{sessions.length})</button>}</>}
  </Panel></div>;
}
export function Quality(props: PageProps) {
  const { t } = useI18n();
  const { account, local, comparison } = selectDay(props.data, props.provider, props.date);
  if (props.data.dataMode === 'observed') return <LocalQuality data={props.data} timezone={props.timezone} />;
  const checks: [string, string, string][] = [
    [t('账号匹配', 'Account match'), account && local && account.scope.accountRef === local.scope.accountRef ? t('合成身份匹配', 'Synthetic identities match') : t('未知', 'Unknown'), 'IDENTITY_UNKNOWN'],
    [t('日期依据', 'Day basis'), account?.day.basis === 'verified-utc-day' ? t('合成 UTC 自然日，结束排他', 'Synthetic UTC day, exclusive end') : t('日界线未验证', 'Day boundary unverified'), 'TIME_BASIS_UNKNOWN'],
    [t('Token 分量', 'Token components'), comparison?.status === 'blocked' ? t('缺失或无法比较', 'Missing or incomparable') : account ? t('缓存计一次，输出子集独立', 'Cache counted once; output subset separate') : t('未知', 'Unknown'), 'MISSING_COMPONENT'],
    [t('本机覆盖', 'Local coverage'), local ? t('仅合成 fixture，真实发现未接入', 'Synthetic fixture only; discovery not connected') : t('未知', 'Unknown'), 'COVERAGE_INCOMPLETE'],
    [t('服务端回填', 'Server backfill'), t('等待回填，未证明 final', 'Pending backfill; finality unproven'), 'PENDING_BACKFILL'],
    [t('真实设备校准', 'Real device calibration'), t('未执行 · 实际误差未知', 'Not run · Actual error unknown'), 'CALIBRATION_NOT_RUN']
  ];
  return <div className="page-stack"><Panel title={t('可解释的质量检查', 'Explainable quality checks')}><div className="quality-checks">{checks.map(([label, result, code]) => <div className="quality-check" key={code}><div><h3>{label}</h3><p className="small muted">{result}</p></div><code>{code}</code></div>)}</div></Panel><Panel title={t('诊断原因', 'Diagnostic reasons')}><ReasonList codes={comparison?.reasonCodes ?? ['NO_DATA', 'CALIBRATION_NOT_RUN']} /><p className="panel-footnote">{t('校准估算能力为 false。更换账号、服务商、产品或日界线时，不允许混算。', 'Calibrated estimates are disabled. Accounts, providers, products and day boundaries cannot be mixed.')}</p></Panel></div>;
}
export function Settings({ data, timezone, setTimezone, onSetRoots, onSetAccountQueries, onSyncAccount, accountBusy }: { data: Dashboard; timezone: string; setTimezone: (zone: string) => void; onSetRoots?: (roots: string[]) => Promise<void> } & AccountActions) {
  const { t } = useI18n();
  const observed = data.dataMode === 'observed';
  return <div className="page-stack"><Panel title={t('数据与偏好', 'Data and preferences')}><div className="setting-row"><div><h3>{t('显示时区', 'Display timezone')}</h3><p className="muted">{t('只改变采集时间和重置时间的显示，不重新划分统计日。', 'Changes collection and reset time display, without rebucketing statistical days.')}</p></div><select aria-label={t('显示时区', 'Display timezone')} value={timezone} onChange={event => setTimezone(event.target.value)}><option>America/Phoenix</option><option>UTC</option></select></div><AccountQuerySwitch data={data} onSetAccountQueries={onSetAccountQueries} /><div className="setting-row"><div><h3>{t('本机增量采集', 'Local incremental collection')}</h3><p className="muted">{observed ? t('启动后先显示缓存，再同步新增完整 JSONL 行。同步可取消，事务检查点支持退出恢复。', 'Cache is shown before syncing new complete JSONL lines. Sync can be cancelled; transactional checkpoints support restart.') : t('当前为演示数据；桌面端默认进入本机账本。', 'Demo data is active. Desktop defaults to the local ledger.')}</p></div><span className="pill">{observed ? t('已接入', 'Connected') : t('演示', 'Demo')}</span></div></Panel><AccountSettings data={data} onSyncAccount={onSyncAccount} accountBusy={accountBusy} /><AccountStatus data={data} timezone={timezone} />{onSetRoots && <RootSettings onSave={onSetRoots} />}<Panel title={t('独立存储', 'Independent storage')}><dl className="storage-details"><dt>{t('应用标识', 'Application ID')}</dt><dd>dev.agentusage.monitor</dd><dt>{t('存储', 'Storage')}</dt><dd>{data.runtime.storage}</dd><dt>{t('Schema 版本', 'Schema version')}</dt><dd>{data.runtime.schemaVersion}</dd><dt>{t('运行模式', 'Runtime mode')}</dt><dd>{observed ? t('真实日志观测 · 按需启动', 'Real log observations · On demand') : t('合成预览 · 按需启动', 'Synthetic preview · On demand')}</dd></dl><p className="panel-footnote">{t('默认发现 CODEX_HOME 或用户目录下 .codex 的 sessions、archived_sessions、session-cleanup-archive。只读日志，数据库使用独立目录。', 'Discovers sessions, archived_sessions and session-cleanup-archive under CODEX_HOME or the user .codex directory. Logs are read-only; the database is independent.')}</p></Panel><Panel title={t('服务商接入状态', 'Provider integration status')}><div className="provider-grid">{data.providers.map(provider => <div className="provider-card" key={provider.providerId}><h3>{provider.displayName}</h3><p className="small muted">{provider.capabilities.localLedger ? t('本机日志与账号只读适配已接入', 'Local logs and read-only account adapter connected') : provider.state === 'synthetic-preview' ? t('双账本合成预览', 'Dual ledger synthetic preview') : t('后续独立适配，当前无数据', 'Future separate adapter; no data yet')}</p></div>)}</div></Panel></div>;
}

function DailyTable({ rows, locale }: { rows: LedgerRow[]; locale: string }) {
  const { t } = useI18n();
  if (!rows.length) return <EmptyState />;
  return <div className="table-scroll"><table><thead><tr>{[t('UTC 日期', 'UTC date'), t('未缓存输入', 'Uncached input'), t('缓存输入', 'Cached input'), t('输出', 'Output'), t('已观测文本总量', 'Observed text total')].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{rows.map(row => <tr key={row.rowId}><td>{row.day.dateLabel}<span className="cell-subtitle">{t('覆盖未核实', 'Coverage unverified')}</span></td>{(['uncachedInput','cachedInput','output','textTotal'] as const).map(key => <td className="numeric" key={key}>{formatCount(row.tokens[key].value,locale)}</td>)}</tr>)}</tbody></table></div>;
}
function LocalQuality({ data, timezone }: { data: Dashboard; timezone: string }) {
  const { t } = useI18n(); const collector = data.collector;
  return <div className="page-stack"><AccountStatus data={data} timezone={timezone} /><Panel title={t('本机采集与持久化', 'Local collection and persistence')}><div className="quality-grid monitor-quality">{[
    [t('当前已索引文件', 'Indexed files'), collector?.filesIndexed ?? 0], [t('保留的缺失来源', 'Retained missing sources'), collector?.retainedSources ?? 0],
    [t('有效增量事件', 'Validated increments'), collector?.acceptedEvents ?? 0], [t('未计入的歧义事件', 'Excluded ambiguous events'), collector?.ambiguousEvents ?? 0]
  ].map(([label,value]) => <div className="quality-item" key={String(label)}><span className="muted">{label}</span><span>{value}</span></div>)}</div><p className="panel-footnote">{t('文件发现覆盖不等于真实执行覆盖。账号归属、后台/云端调用、父子计数范围仍未核实。', 'File discovery is distinct from execution coverage. Account identity, background/cloud activity and parent/child counter scope remain unverified.')}</p></Panel><Panel title={t('上次增量同步', 'Last incremental sync')}><dl className="storage-details"><dt>{t('状态', 'State')}</dt><dd>{collector?.state ?? 'never-synced'}</dd><dt>{t('未重解析文件', 'Unchanged files')}</dt><dd>{collector?.lastRun?.filesUnchanged ?? '—'}</dd><dt>{t('解析新增字节', 'New bytes parsed')}</dt><dd>{collector?.lastRun?.bytesParsed ?? '—'}</dd><dt>{t('完整性检查字节', 'Integrity bytes checked')}</dt><dd>{collector?.lastRun?.bytesVerified ?? '—'}</dd><dt>{t('半行尾部等待', 'Incomplete tails pending')}</dt><dd>{collector?.lastRun?.partialTails ?? '—'}</dd></dl><p className="panel-footnote">{t('变化文件会核对已提交前缀摘要；该检查读取旧字节，但不会重解析旧 JSONL 记录。', 'Changed files verify the committed prefix digest. Integrity checks read old bytes without reparsing old JSONL records.')}</p></Panel><Panel title={t('质量与排除原因', 'Quality and exclusion reasons')}><ReasonList codes={collector?.reasonCodes ?? ['NO_DATA']} /></Panel></div>;
}
function RootSettings({ onSave }: { onSave: (roots: string[]) => Promise<void> }) {
  const { t } = useI18n(); const [value,setValue] = useState(''); const [status,setStatus] = useState('');
  return <Panel title={t('附加 Codex 根目录', 'Additional Codex roots')}><p className="small muted">{t('每行填写一个绝对目录；仅扫描其中三类会话目录。保存后点击同步。留空会移除附加目录，已采集历史保留。', 'One absolute directory per line; only the three session directories are scanned. Sync after saving. Empty removes additional roots while preserving collected history.')}</p><textarea className="root-input" aria-label={t('附加 Codex 根目录', 'Additional Codex roots')} value={value} onChange={event => setValue(event.target.value)} rows={3} /><button className="button button-small" onClick={() => { setStatus(''); void onSave(value.split(/\r?\n/).map(line => line.trim()).filter(Boolean)).then(() => setStatus(t('已保存，可同步本机日志', 'Saved; local logs can be synchronized'))).catch(() => setStatus(t('保存失败，请使用现有绝对目录', 'Save failed; use existing absolute directories'))); }}>{t('保存附加目录', 'Save additional roots')}</button>{status && <p className="small muted" role="status">{status}</p>}</Panel>;
}
