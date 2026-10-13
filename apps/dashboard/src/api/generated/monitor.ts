// GENERATED from crates/monitor-contracts/src/lib.rs. Do not edit.
export const API_VERSION = "0.1.0" as const;
export const COMMANDS = {
  getDashboard: "get_dashboard",
} as const;
export type DataMode = "synthetic" | "observed";
export type LedgerKind = "account" | "windows";
export type SourceKind = "server-observation" | "local-log";
export type TimeBasis = "verified-utc-day" | "server-date-label" | "local-day" | "unknown";
export type IdentityEvidence = "verified" | "unknown";
export type QualityStatus = "known" | "partial" | "unknown" | "inconsistent";
export type CoverageStatus = "complete" | "partial" | "unknown";
export type FreshnessStatus = "pending-backfill" | "final" | "unknown";
export type ComparisonStatus = "unattributed" | "blocked" | "inconsistent";
export type ProviderState = "synthetic-preview" | "planned";
export type DemoScenario = "ready" | "missing" | "negative" | "empty";
export type QuotaKind = "five-hour" | "weekly";
export type ErrorCode = "INVALID_DATA" | "INVALID_QUERY" | "STORAGE" | "STORAGE_SCHEMA_NEWER" | "API_VERSION_UNSUPPORTED" | "INTERNAL";
export const ERROR_CODES = ["INVALID_DATA", "INVALID_QUERY", "STORAGE", "STORAGE_SCHEMA_NEWER", "API_VERSION_UNSUPPORTED", "INTERNAL"] as const;
export type ReasonCode = "SYNTHETIC_DATA" | "CALIBRATION_NOT_RUN" | "MISSING_COMPONENT" | "IDENTITY_UNKNOWN" | "ACCOUNT_MISMATCH" | "PROVIDER_MISMATCH" | "PRODUCT_MISMATCH" | "TIME_BASIS_UNKNOWN" | "DATE_MISMATCH" | "COVERAGE_INCOMPLETE" | "SOURCE_MISMATCH" | "PENDING_BACKFILL" | "FRESHNESS_UNKNOWN" | "NEGATIVE_RESIDUAL" | "INVALID_TOKEN" | "INCONSISTENT_COMPONENTS" | "NO_DATA" | "WINDOW_NOT_REPORTED" | "NETWORK_DISABLED";
export interface Metric<T> {
  value: T | null;
  quality: QualityStatus;
  reasonCodes: Array<ReasonCode>;
}
export interface TokenComponents {
  uncachedInput: Metric<string>;
  cachedInput: Metric<string>;
  inclusiveInput: Metric<string>;
  output: Metric<string>;
  reasoningOutput: Metric<string>;
  textTotal: Metric<string>;
}
export interface Scope {
  providerId: string;
  productId: string;
  accountRef: string | null;
  identityEvidence: IdentityEvidence;
}
export interface DayBucket {
  dateLabel: string;
  basis: TimeBasis;
  timezone: string | null;
  startUtc: string | null;
  endUtc: string | null;
}
export interface Source {
  sourceId: string;
  kind: SourceKind;
  collectedAt: string;
  dataMode: DataMode;
}
export interface Quality {
  status: QualityStatus;
  coverage: CoverageStatus;
  freshness: FreshnessStatus;
  reasonCodes: Array<ReasonCode>;
}
export interface LedgerRow {
  rowId: string;
  ledger: LedgerKind;
  scope: Scope;
  day: DayBucket;
  source: Source;
  tokens: TokenComponents;
  quality: Quality;
}
export interface ComparisonResult {
  dateLabel: string;
  status: ComparisonStatus;
  uncachedInput: string | null;
  cachedInput: string | null;
  output: string | null;
  textTotal: string | null;
  reasonCodes: Array<ReasonCode>;
}
export interface ProviderCapabilities {
  localLedger: boolean;
  accountLedger: boolean;
  incrementalCollection: boolean;
  calibratedEstimate: boolean;
}
export interface Provider {
  providerId: string;
  displayName: string;
  state: ProviderState;
  capabilities: ProviderCapabilities;
}
export interface QuotaWindow {
  kind: QuotaKind;
  windowSeconds: number;
  usedPercent: string | null;
  resetsAt: string | null;
  reasonCodes: Array<ReasonCode>;
}
export interface QuotaSnapshot {
  snapshotId: string;
  scope: Scope;
  source: Source;
  windows: Array<QuotaWindow>;
}
export interface LocalModel {
  modelId: string;
  scope: Scope;
  textTotal: Metric<string>;
  sessions: number;
}
export interface LocalSession {
  sessionRef: string;
  modelId: string | null;
  effort: string | null;
  speed: string | null;
  scope: Scope;
  textTotal: Metric<string>;
}
export interface RuntimeInfo {
  appVersion: string;
  schemaVersion: number;
  storage: string;
  installationRef: string;
  networkEnabled: boolean;
  localCollectionEnabled: boolean;
}
export interface Dashboard {
  apiVersion: string;
  dataMode: DataMode;
  displayTimezone: string;
  providers: Array<Provider>;
  accountRows: Array<LedgerRow>;
  localRows: Array<LedgerRow>;
  comparisons: Array<ComparisonResult>;
  quota: QuotaSnapshot | null;
  models: Array<LocalModel>;
  sessions: Array<LocalSession>;
  runtime: RuntimeInfo;
}
export interface DashboardQuery {
  scenario: DemoScenario;
}
export interface ApiError {
  apiVersion: string;
  code: ErrorCode;
  message: string;
  retryable: boolean;
}
