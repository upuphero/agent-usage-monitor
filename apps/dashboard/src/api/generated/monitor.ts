// GENERATED from crates/monitor-contracts/src/lib.rs. Do not edit.
export const API_VERSION = "0.4.2" as const;
export const COMMANDS = {
  getDashboard: "get_dashboard",
  syncLocal: "sync_local",
  cancelLocalSync: "cancel_local_sync",
  setLocalRoots: "set_local_roots",
  setAccountQueries: "set_account_queries",
  syncAccount: "sync_account",
  exportDeviceReport: "export_device_report",
  importDeviceReport: "import_device_report",
  removeDeviceReport: "remove_device_report",
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
export type ErrorCode = "INVALID_DATA" | "INVALID_QUERY" | "STORAGE" | "STORAGE_SCHEMA_NEWER" | "API_VERSION_UNSUPPORTED" | "INTERNAL" | "CANCELLED" | "SYNC_BUSY" | "SOURCE_UNREADABLE" | "INVALID_DIRECTORY" | "NETWORK_DISABLED" | "AUTH_REQUIRED" | "ACCESS_DENIED" | "ENDPOINT_UNAVAILABLE" | "RATE_LIMITED" | "OFFLINE" | "RESPONSE_INCOMPATIBLE";
export const ERROR_CODES = ["INVALID_DATA", "INVALID_QUERY", "STORAGE", "STORAGE_SCHEMA_NEWER", "API_VERSION_UNSUPPORTED", "INTERNAL", "CANCELLED", "SYNC_BUSY", "SOURCE_UNREADABLE", "INVALID_DIRECTORY", "NETWORK_DISABLED", "AUTH_REQUIRED", "ACCESS_DENIED", "ENDPOINT_UNAVAILABLE", "RATE_LIMITED", "OFFLINE", "RESPONSE_INCOMPATIBLE"] as const;
export type ReasonCode = "SYNTHETIC_DATA" | "CALIBRATION_NOT_RUN" | "MISSING_COMPONENT" | "IDENTITY_UNKNOWN" | "ACCOUNT_MISMATCH" | "PROVIDER_MISMATCH" | "PRODUCT_MISMATCH" | "TIME_BASIS_UNKNOWN" | "DATE_MISMATCH" | "COVERAGE_INCOMPLETE" | "SOURCE_MISMATCH" | "PENDING_BACKFILL" | "FRESHNESS_UNKNOWN" | "NEGATIVE_RESIDUAL" | "INVALID_TOKEN" | "INCONSISTENT_COMPONENTS" | "NO_DATA" | "WINDOW_NOT_REPORTED" | "NETWORK_DISABLED" | "ACCOUNT_LEDGER_UNAVAILABLE" | "THREAD_IDENTITY_MISSING" | "INITIAL_COUNTER_BASELINE" | "COUNTER_RESET" | "COUNTER_GAP" | "REPEATED_COUNTER_CONFLICT" | "INHERITED_HISTORY" | "CHILD_SCOPE_UNVERIFIED" | "CROSS_DAY_INTERVAL" | "EVENT_OWNERSHIP_AMBIGUOUS" | "METADATA_CONFLICT" | "INVALID_RECORD" | "RECORD_TOO_LARGE" | "SOURCE_REWRITTEN" | "SOURCE_MISSING" | "SOURCE_CHANGED_DURING_READ" | "DISCOVERY_INCOMPLETE" | "IMPORTED_SOURCE" | "PRODUCT_SCOPE_UNVERIFIED" | "MODEL_UNKNOWN" | "SESSION_LIST_LIMITED" | "HISTORY_DAY_MISSING" | "QUOTA_STALE" | "RESPONSE_INCOMPATIBLE";
export const REASON_CODES = ["SYNTHETIC_DATA", "CALIBRATION_NOT_RUN", "MISSING_COMPONENT", "IDENTITY_UNKNOWN", "ACCOUNT_MISMATCH", "PROVIDER_MISMATCH", "PRODUCT_MISMATCH", "TIME_BASIS_UNKNOWN", "DATE_MISMATCH", "COVERAGE_INCOMPLETE", "SOURCE_MISMATCH", "PENDING_BACKFILL", "FRESHNESS_UNKNOWN", "NEGATIVE_RESIDUAL", "INVALID_TOKEN", "INCONSISTENT_COMPONENTS", "NO_DATA", "WINDOW_NOT_REPORTED", "NETWORK_DISABLED", "ACCOUNT_LEDGER_UNAVAILABLE", "THREAD_IDENTITY_MISSING", "INITIAL_COUNTER_BASELINE", "COUNTER_RESET", "COUNTER_GAP", "REPEATED_COUNTER_CONFLICT", "INHERITED_HISTORY", "CHILD_SCOPE_UNVERIFIED", "CROSS_DAY_INTERVAL", "EVENT_OWNERSHIP_AMBIGUOUS", "METADATA_CONFLICT", "INVALID_RECORD", "RECORD_TOO_LARGE", "SOURCE_REWRITTEN", "SOURCE_MISSING", "SOURCE_CHANGED_DURING_READ", "DISCOVERY_INCOMPLETE", "IMPORTED_SOURCE", "PRODUCT_SCOPE_UNVERIFIED", "MODEL_UNKNOWN", "SESSION_LIST_LIMITED", "HISTORY_DAY_MISSING", "QUOTA_STALE", "RESPONSE_INCOMPATIBLE"] as const;
export type AccountState = "disabled" | "never-synced" | "cached" | "ready" | "partial" | "auth-required" | "access-denied" | "endpoint-unavailable" | "rate-limited" | "offline" | "incompatible";
export type DevicePlatform = "windows" | "macos" | "linux" | "unknown";
export interface Metric<T> {
  value: T | null;
  quality: QualityStatus;
  reasonCodes: Array<ReasonCode>;
}
export interface AccountQueriesRequest {
  enabled: boolean;
}
export interface AccountSyncRequest {
  startDate: string;
  endDate: string;
}
export interface AccountSummary {
  enabled: boolean;
  accountRef: string | null;
  state: AccountState;
  lastAttemptAt: string | null;
  historyFetchedAt: string | null;
  quotaFetchedAt: string | null;
  historyError: ErrorCode | null;
  quotaError: ErrorCode | null;
  nextAllowedAt: string | null;
  requestedStart: string | null;
  requestedEnd: string | null;
  missingDates: Array<string>;
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
  accountNoticePending: boolean;
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
export interface ExclusionCount {
  reasonCode: ReasonCode;
  events: number;
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
  excludedBreakdown: Array<ExclusionCount>;
}
export interface LocalRootsRequest {
  roots: Array<string>;
}
export interface DeviceIdentity {
  deviceRef: string;
  platform: DevicePlatform;
}
export interface DeviceEvent {
  eventRef: string;
  at: string;
  scope: Scope;
  tokens: TokenComponents;
}
export interface DeviceReport {
  format: string;
  schemaVersion: number;
  device: DeviceIdentity;
  generatedAt: string;
  lastSyncAt: string | null;
  filesIndexed: number;
  excludedEvents: number;
  reasonCodes: Array<ReasonCode>;
  events: Array<DeviceEvent>;
}
export interface DeviceSummary {
  device: DeviceIdentity;
  generatedAt: string;
  lastSyncAt: string | null;
  filesIndexed: number;
  records: number;
  excludedEvents: number;
  reasonCodes: Array<ReasonCode>;
}
export interface DeviceDay {
  deviceRef: string | null;
  scope: Scope;
  dateLabel: string;
  tokens: TokenComponents;
  events: number;
}
export interface DeviceUsage {
  devices: Array<DeviceSummary>;
  daily: Array<DeviceDay>;
  duplicateEvents: number;
  conflictEvents: number;
}
export interface DeviceImportRequest {
  reportJson: string;
}
export interface DeviceRemoveRequest {
  deviceRef: string;
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
  account: AccountSummary | null;
  deviceUsage: DeviceUsage | null;
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
