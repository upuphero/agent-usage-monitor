// GENERATED from crates/monitor-contracts/src/lib.rs. Do not edit.
export const API_VERSION = "0.2.0" as const;
export const COMMANDS = {
  getDashboard: "get_dashboard",
  syncLocal: "sync_local",
  cancelLocalSync: "cancel_local_sync",
  setLocalRoots: "set_local_roots",
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
export type ProviderState = "synthetic-preview" | "planned" | "local-ready" | "local-partial" | "not-detected";
export type DemoScenario = "local" | "ready" | "missing" | "negative" | "empty";
export type SyncState = "never-synced" | "idle" | "syncing" | "cancelled" | "failed";
export type QuotaKind = "five-hour" | "weekly";
export type ErrorCode = "INVALID_DATA" | "INVALID_QUERY" | "STORAGE" | "STORAGE_SCHEMA_NEWER" | "API_VERSION_UNSUPPORTED" | "INTERNAL" | "CANCELLED" | "SYNC_BUSY" | "SOURCE_UNREADABLE" | "INVALID_DIRECTORY";
export const ERROR_CODES = ["INVALID_DATA", "INVALID_QUERY", "STORAGE", "STORAGE_SCHEMA_NEWER", "API_VERSION_UNSUPPORTED", "INTERNAL", "CANCELLED", "SYNC_BUSY", "SOURCE_UNREADABLE", "INVALID_DIRECTORY"] as const;
export type ReasonCode = "SYNTHETIC_DATA" | "CALIBRATION_NOT_RUN" | "MISSING_COMPONENT" | "IDENTITY_UNKNOWN" | "ACCOUNT_MISMATCH" | "PROVIDER_MISMATCH" | "PRODUCT_MISMATCH" | "TIME_BASIS_UNKNOWN" | "DATE_MISMATCH" | "COVERAGE_INCOMPLETE" | "SOURCE_MISMATCH" | "PENDING_BACKFILL" | "FRESHNESS_UNKNOWN" | "NEGATIVE_RESIDUAL" | "INVALID_TOKEN" | "INCONSISTENT_COMPONENTS" | "NO_DATA" | "WINDOW_NOT_REPORTED" | "NETWORK_DISABLED" | "ACCOUNT_LEDGER_UNAVAILABLE" | "THREAD_IDENTITY_MISSING" | "INITIAL_COUNTER_BASELINE" | "COUNTER_RESET" | "COUNTER_GAP" | "REPEATED_COUNTER_CONFLICT" | "INHERITED_HISTORY" | "CHILD_SCOPE_UNVERIFIED" | "CROSS_DAY_INTERVAL" | "EVENT_OWNERSHIP_AMBIGUOUS" | "METADATA_CONFLICT" | "INVALID_RECORD" | "RECORD_TOO_LARGE" | "SOURCE_REWRITTEN" | "SOURCE_MISSING" | "SOURCE_CHANGED_DURING_READ" | "DISCOVERY_INCOMPLETE" | "IMPORTED_SOURCE" | "PRODUCT_SCOPE_UNVERIFIED" | "MODEL_UNKNOWN" | "SESSION_LIST_LIMITED";
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
  scope: Scope;
  accountRowId: string;
  localRowId: string;
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
export interface SyncReport {
  filesFound: number;
  filesParsed: number;
  filesUnchanged: number;
  filesRebuilt: number;
  newSnapshots: number;
  duplicateSnapshots: number;
  bytesParsed: string;
  bytesVerified: string;
  partialTails: number;
  reasonCodes: Array<ReasonCode>;
}
export interface CollectorSummary {
  state: SyncState;
  lastSyncAt: string | null;
  filesIndexed: number;
  retainedSources: number;
  snapshots: number;
  acceptedEvents: number;
  ambiguousEvents: number;
  sessionsTotal: number;
  reasonCodes: Array<ReasonCode>;
  lastRun: SyncReport | null;
}
export interface LocalRootsRequest {
  roots: Array<string>;
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
  collector: CollectorSummary | null;
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
