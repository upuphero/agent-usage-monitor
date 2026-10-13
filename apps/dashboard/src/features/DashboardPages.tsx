import type { ComparisonResult, Dashboard, LedgerRow } from '../api/generated/monitor';
import { formatCount, formatTime, latestRows, percentOf } from '../components/format';
import { EmptyState, MetricCard, Notice, Panel, ReasonList } from '../components/ui';
import { useI18n } from '../i18n/I18nContext';
import { TrendChart } from './TrendChart';

export type Page = 'overview' | 'history' | 'sessions' | 'quality' | 'settings';
export interface PageProps { data: Dashboard; provider: string; date: string; timezone: string }
const scopeRows = (rows: LedgerRow[], provider: string) => rows.filter(row => row.scope.providerId === provider);
export function selectDay(data: Dashboard, provider: string, date: string) {
  const account = latestRows(scopeRows(data.accountRows, provider)).find(row => row.day.dateLabel === date);
  const local = scopeRows(data.localRows, provider).find(row => row.day.dateLabel === date);
  const comparison = local && account ? data.comparisons.find(item => item.dateLabel === date) : undefined;
  return { account, local, comparison };
}
function Quota({ data, provider, timezone }: PageProps) {
  const { t, locale } = useI18n();
  const quota = data.quota?.scope.providerId === provider ? data.quota : null;
  return <div className="quota-strip">{(['five-hour', 'weekly'] as const).map(kind => {
    const window = quota?.windows.find(item => item.kind === kind);
    return <div className="quota-item" key={kind}><span className="small muted">{kind === 'five-hour' ? t('5 小时窗口', '5-hour window') : t('每周窗口', 'Weekly window')}</span><strong>{window?.usedPercent == null ? t('未报告', 'Not reported') : `${window.usedPercent}% ${t('已用', 'used')}`}</strong><span className="small muted">{window?.resetsAt ? `${t('演示重置', 'Demo reset')}: ${formatTime(window.resetsAt, timezone, locale)} · ${timezone}` : t('重置时间未知', 'Reset time unknown')}</span></div>;
  })}</div>;
}
function Composition({ row }: { row?: LedgerRow }) {
  const { t, locale } = useI18n();
  const parts = [
    ['uncachedInput', t('未缓存输入', 'Uncached input')], ['cachedInput', t('缓存输入', 'Cached input')], ['output', t('输出', 'Output')]
  ] as const;
  const total = row?.tokens.textTotal.value ?? null;
  return <Panel title={t('账号 Token 组成', 'Account Token composition')} eyebrow={t('缓存只计一次', 'Cache counted once')}>
    {!row ? <EmptyState /> : <><div className="composition-track" aria-hidden="true">{parts.map(([key], i) => <span key={key} className={`composition-part color-${i}`} style={{ width: `${total === null || row.tokens[key].value === null ? 0 : percentOf(row.tokens[key].value, total)}%` }} />)}</div><div className="distribution-list">{parts.map(([key, label], i) => <div className="distribution-label" key={key}><i className={`chart-dot color-${i}`} /><span>{label}</span><span className="distribution-value">{formatCount(row.tokens[key].value, locale)}</span></div>)}</div><p className="panel-footnote">{t('文本总量 = 未缓存输入 + 缓存输入 + 输出；reasoning 是输出子集。', 'Text total = uncached input + cached input + output. Reasoning is a subset of output.')}</p></>}
  </Panel>;
}
function ResidualNotice({ comparison }: { comparison?: ComparisonResult }) {
  const { t } = useI18n();
  return <Notice tone={comparison?.status === 'inconsistent' ? 'error' : 'warning'} title={comparison?.status === 'inconsistent' ? t('两个来源出现矛盾', 'The sources are inconsistent') : t('设备归因尚未开放', 'Device attribution is unavailable')}>
    <p>{t('诊断差值来自账号减本机，仍为未归因状态。真实设备校准尚未执行。', 'The diagnostic difference is account minus local usage and remains unattributed. Real device calibration has not run.')}</p>
    <ReasonList codes={comparison?.reasonCodes ?? ['NO_DATA', 'CALIBRATION_NOT_RUN']} />
  </Notice>;
}
export function Overview(props: PageProps) {
  const { t } = useI18n();
  const { account, local, comparison } = selectDay(props.data, props.provider, props.date);
  return <div className="page-stack"><Quota {...props} /><div className="metrics-grid monitor-metrics">
    <MetricCard label={t('账号服务端账本', 'Account server ledger')} value={account?.tokens.textTotal.value ?? null} accent status={t('合成服务端观测', 'Synthetic server observation')} note={account ? `${account.scope.productId} · ${account.day.dateLabel} UTC` : t('尚无账号数据', 'No account data')} />
    <MetricCard label={t('Windows 本机账本', 'Windows local ledger')} value={local?.tokens.textTotal.value ?? null} status={t('合成本地日志', 'Synthetic local logs')} note={local ? `${local.scope.productId} · ${local.day.dateLabel} UTC` : t('尚无本机数据', 'No local data')} />
    <MetricCard label={t('未归因诊断差值', 'Unattributed diagnostic difference')} value={comparison?.textTotal ?? null} warning={comparison?.status === 'inconsistent'} status={comparison?.status === 'blocked' ? t('减法已阻断', 'Subtraction blocked') : comparison?.status === 'inconsistent' ? t('负差值 · 矛盾', 'Negative · Inconsistent') : t('未归因', 'Unattributed')} note={t('账号 − Windows；不能命名为 MacBook 用量', 'Account − Windows; cannot be labeled MacBook usage')} />
  </div><div className="monitor-overview-grid"><TrendChart account={scopeRows(props.data.accountRows, props.provider)} local={scopeRows(props.data.localRows, props.provider)} /><Composition row={account} /></div><ResidualNotice comparison={comparison} /><Panel title={t('来源与时间依据', 'Sources and time basis')}><div className="quality-grid monitor-quality"><div className="quality-item"><span className="muted">{t('账号范围', 'Account scope')}</span><span>{account?.scope.accountRef ?? t('未知', 'Unknown')}</span></div><div className="quality-item"><span className="muted">{t('统计日界线', 'Statistical boundary')}</span><span>{account?.day.basis === 'verified-utc-day' ? t('合成 UTC 日', 'Synthetic UTC day') : t('未验证', 'Unverified')}</span></div><div className="quality-item"><span className="muted">{t('本地覆盖', 'Local coverage')}</span><span>{local ? t('合成完整覆盖', 'Synthetic complete coverage') : t('未知', 'Unknown')}</span></div><div className="quality-item"><span className="muted">{t('回填水位', 'Backfill status')}</span><span>{t('未确定 final', 'Finality unconfirmed')}</span></div></div></Panel></div>;
}
export function History(props: PageProps) {
  const { t, locale } = useI18n();
  const account = scopeRows(props.data.accountRows, props.provider), local = scopeRows(props.data.localRows, props.provider);
  return <div className="page-stack"><TrendChart account={account} local={local} /><Panel title={t('账号追加快照', 'Append-only account snapshots')} eyebrow={t('同日旧版本保留', 'Earlier revisions retained')}>
    {!account.length ? <EmptyState /> : <div className="table-scroll"><table><thead><tr>{[t('统计日期', 'Date label'), t('采集时间', 'Collected at'), t('未缓存输入', 'Uncached input'), t('缓存输入', 'Cached input'), t('输出', 'Output'), t('文本总量', 'Text total')].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{account.map(row => <tr key={row.rowId}><td>{row.day.dateLabel}<span className="cell-subtitle">{row.day.basis}</span></td><td>{formatTime(row.source.collectedAt, props.timezone, locale)}<span className="cell-subtitle">{props.timezone}</span></td>{(['uncachedInput', 'cachedInput', 'output', 'textTotal'] as const).map(key => <td key={key} className="numeric">{formatCount(row.tokens[key].value, locale)}</td>)}</tr>)}</tbody></table></div>}
    <p className="panel-footnote">{t('趋势使用每个日期最新快照；快照版本不相加。演示日期不代表当前账号用量。', 'The trend uses the latest snapshot for each date; revisions are not summed. Demo dates are not current account usage.')}</p>
  </Panel></div>;
}
export function Sessions(props: PageProps) {
  const { t, locale } = useI18n();
  const models = props.data.models.filter(row => row.scope.providerId === props.provider), sessions = props.data.sessions.filter(row => row.scope.providerId === props.provider);
  const maximum = models.reduce((max, model) => model.textTotal.value !== null && BigInt(model.textTotal.value) > BigInt(max) ? model.textTotal.value : max, '0');
  return <div className="page-stack"><Notice title={t('仅 Windows 本机', 'Windows local only')}>{t('模型与会话来自七个演示日的合成本地统计。远程模型未知。', 'Models and sessions come from synthetic local statistics across seven demo days. Remote models are unknown.')}</Notice><Panel title={t('本机模型', 'Local models')}>
    {!models.length ? <EmptyState /> : <div className="distribution-list">{models.map((model, i) => <div key={model.modelId}><div className="distribution-label"><i className={`chart-dot color-${i}`} /><span>{model.modelId}</span><span className="distribution-value">{formatCount(model.textTotal.value, locale)}</span></div><div className="bar-track"><div className={`bar-fill color-${i}`} style={{ width: `${model.textTotal.value === null ? 0 : percentOf(model.textTotal.value, maximum)}%` }} /></div><span className="small muted">{model.sessions} {t('个合成会话', 'synthetic sessions')}</span></div>)}</div>}
  </Panel><Panel title={t('本机会话', 'Local sessions')}>
    {!sessions.length ? <EmptyState /> : <div className="table-scroll"><table><thead><tr>{[t('会话', 'Session'), t('模型', 'Model'), 'Effort', 'Speed', 'Token'].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{sessions.map(session => <tr key={session.sessionRef}><td>{session.sessionRef}</td><td>{session.modelId ?? t('未知', 'Unknown')}</td><td>{session.effort ?? t('未知', 'Unknown')}</td><td>{session.speed ?? t('未知', 'Unknown')}</td><td className="numeric">{formatCount(session.textTotal.value, locale)}</td></tr>)}</tbody></table></div>}
  </Panel></div>;
}
export function Quality(props: PageProps) {
  const { t } = useI18n();
  const { account, local, comparison } = selectDay(props.data, props.provider, props.date);
  const checks: [string, string, string][] = [
    [t('账号匹配', 'Account match'), account && local && account.scope.accountRef === local.scope.accountRef ? t('合成身份匹配', 'Synthetic identities match') : t('未知', 'Unknown'), 'IDENTITY_UNKNOWN'],
    [t('日期依据', 'Day basis'), account?.day.basis === 'verified-utc-day' ? t('合成 UTC 自然日，结束排他', 'Synthetic UTC day, exclusive end') : t('日界线未验证', 'Day boundary unverified'), 'TIME_BASIS_UNKNOWN'],
    [t('Token 分量', 'Token components'), comparison?.status === 'blocked' ? t('缺失或无法比较', 'Missing or incomparable') : account ? t('缓存计一次，输出子集独立', 'Cache counted once; output subset separate') : t('未知', 'Unknown'), 'MISSING_COMPONENT'],
    [t('本机覆盖', 'Local coverage'), local ? t('仅合成 fixture，真实发现未接入', 'Synthetic fixture only; discovery not connected') : t('未知', 'Unknown'), 'COVERAGE_INCOMPLETE'],
    [t('服务端回填', 'Server backfill'), t('等待回填，未证明 final', 'Pending backfill; finality unproven'), 'PENDING_BACKFILL'],
    [t('真实设备校准', 'Real device calibration'), t('未执行 · 实际误差未知', 'Not run · Actual error unknown'), 'CALIBRATION_NOT_RUN']
  ];
  return <div className="page-stack"><ResidualNotice comparison={comparison} /><Panel title={t('可解释的质量检查', 'Explainable quality checks')}><div className="quality-checks">{checks.map(([label, result, code]) => <div className="quality-check" key={code}><div><h3>{label}</h3><p className="small muted">{result}</p></div><code>{code}</code></div>)}</div></Panel><Panel title={t('诊断原因', 'Diagnostic reasons')}><ReasonList codes={comparison?.reasonCodes ?? ['NO_DATA', 'CALIBRATION_NOT_RUN']} /><p className="panel-footnote">{t('校准估算能力为 false。更换账号、服务商、产品或日界线时，不允许混算。', 'Calibrated estimates are disabled. Accounts, providers, products and day boundaries cannot be mixed.')}</p></Panel></div>;
}
export function Settings({ data, timezone, setTimezone }: { data: Dashboard; timezone: string; setTimezone: (zone: string) => void }) {
  const { t } = useI18n();
  return <div className="page-stack"><Panel title={t('数据与偏好', 'Data and preferences')}><div className="setting-row"><div><h3>{t('显示时区', 'Display timezone')}</h3><p className="muted">{t('只改变采集时间和重置时间的显示，不重新划分统计日。', 'Changes collection and reset time display, without rebucketing statistical days.')}</p></div><select aria-label={t('显示时区', 'Display timezone')} value={timezone} onChange={event => setTimezone(event.target.value)}><option>America/Phoenix</option><option>UTC</option></select></div><div className="setting-row"><div><h3>{t('账号查询', 'Account queries')}</h3><p className="muted">{t('P0 未接入真实服务端查询。', 'Real account queries are not connected in P0.')}</p></div><span className="pill">{t('关闭', 'Disabled')}</span></div><div className="setting-row"><div><h3>{t('本机增量采集', 'Local incremental collection')}</h3><p className="muted">{t('将在 A 阶段实现；当前仅读取合成数据。', 'Planned for stage A; currently reads synthetic data only.')}</p></div><span className="pill">{t('未接入', 'Not connected')}</span></div></Panel><Panel title={t('独立存储', 'Independent storage')}><dl className="storage-details"><dt>{t('应用标识', 'Application ID')}</dt><dd>dev.agentusage.monitor</dd><dt>{t('存储', 'Storage')}</dt><dd>{data.runtime.storage}</dd><dt>{t('Schema 版本', 'Schema version')}</dt><dd>{data.runtime.schemaVersion}</dd><dt>{t('运行模式', 'Runtime mode')}</dt><dd>{t('合成预览 · 按需启动', 'Synthetic preview · On demand')}</dd></dl><p className="panel-footnote">{t('桌面端使用独立 SQLite；浏览器预览使用内存。账号快照追加保存，迁移失败回滚，新版数据库拒绝降级。', 'Desktop uses independent SQLite; browser preview uses memory. Snapshots append, failed migrations roll back, and newer schemas cannot be downgraded.')}</p></Panel><Panel title={t('服务商接入状态', 'Provider integration status')}><div className="provider-grid">{data.providers.map(provider => <div className="provider-card" key={provider.providerId}><h3>{provider.displayName}</h3><p className="small muted">{provider.state === 'synthetic-preview' ? t('首个双账本合成预览', 'First dual ledger synthetic preview') : t('后续独立适配，当前无数据', 'Future separate adapter; no data yet')}</p></div>)}</div></Panel></div>;
}
