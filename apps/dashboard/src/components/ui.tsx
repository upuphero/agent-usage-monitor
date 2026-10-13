// Adapted from Agent Usage Dashboard; see THIRD_PARTY_NOTICES.md and LICENSE.
import type { ReactNode } from 'react';
import type { ReasonCode } from '../api/generated/monitor';
import { formatCount, reasonLabel } from './format';
import { Icon } from './Icon';
import { useI18n } from '../i18n/I18nContext';

export function Panel({ title, eyebrow, children, action, className = '' }: { title: string; eyebrow?: string; children: ReactNode; action?: ReactNode; className?: string }) {
  return <section className={`panel ${className}`}><div className="panel-header"><div>{eyebrow && <p className="eyebrow">{eyebrow}</p>}<h2>{title}</h2></div>{action}</div>{children}</section>;
}
export function Notice({ children, tone = 'info', title, action }: { children: ReactNode; tone?: 'info' | 'warning' | 'error'; title?: string; action?: ReactNode }) {
  return <div className={`notice notice-${tone}`} role={tone === 'error' ? 'alert' : 'status'}><Icon name="info" /><div>{title && <strong>{title}</strong>}<div>{children}</div></div>{action}</div>;
}
export function MetricCard({ label, value, note, status, accent = false, warning = false }: { label: string; value: string | null; note: string; status: string; accent?: boolean; warning?: boolean }) {
  const { locale, t } = useI18n();
  return <article className={`metric-card ${accent ? 'metric-accent' : ''} ${warning ? 'metric-warning' : ''}`}><div className="metric-label">{label}<Icon name="bolt" /></div><div className="metric-number" title={formatCount(value, locale)}><span className={value === null ? 'unavailable' : 'numeric'}>{value === null ? t('未知', 'Unknown') : formatCount(value, locale)}</span></div><div className="metric-bottom"><span className="quality">{status}</span><span className="small muted">{note}</span></div></article>;
}
export function EmptyState() {
  const { t } = useI18n();
  return <div className="empty-state"><span className="empty-icon"><Icon name="sessions" /></span><h3>{t('当前范围尚无数据', 'No data in this scope')}</h3><p>{t('未知与零分别表示。可切换服务商或演示场景。', 'Unknown values are distinct from zero. Change the provider or demo scenario.')}</p></div>;
}
export function ReasonList({ codes }: { codes: ReasonCode[] }) {
  const { language } = useI18n();
  return <div className="reason-list">{codes.map(code => <span className="pill" key={code} title={code}>{reasonLabel(code, language)}</span>)}</div>;
}
