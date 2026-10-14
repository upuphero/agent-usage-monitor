use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "0.3.2";
pub const COMMANDS: &[(&str, &str)] = &[
    ("getDashboard", "get_dashboard"),
    ("syncLocal", "sync_local"),
    ("cancelLocalSync", "cancel_local_sync"),
    ("setLocalRoots", "set_local_roots"),
    ("setAccountQueries", "set_account_queries"),
    ("syncAccount", "sync_account"),
];

macro_rules! dto {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
    };
}
macro_rules! enumeration {
    ($name:ident, $rename:literal, $($variant:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = $rename)]
        pub enum $name { $($variant),+ }
    };
}

enumeration!(DataMode, "kebab-case", Synthetic, Observed);
enumeration!(LedgerKind, "kebab-case", Account, Windows);
enumeration!(SourceKind, "kebab-case", ServerObservation, LocalLog);
enumeration!(
    TimeBasis,
    "kebab-case",
    VerifiedUtcDay,
    ServerDateLabel,
    LocalDay,
    Unknown
);
enumeration!(IdentityEvidence, "kebab-case", Verified, Unknown);
enumeration!(
    QualityStatus,
    "kebab-case",
    Known,
    Partial,
    Unknown,
    Inconsistent
);
enumeration!(CoverageStatus, "kebab-case", Complete, Partial, Unknown);
enumeration!(
    FreshnessStatus,
    "kebab-case",
    PendingBackfill,
    Final,
    Unknown
);
enumeration!(
    ComparisonStatus,
    "kebab-case",
    Unattributed,
    Blocked,
    Inconsistent
);
enumeration!(
    ProviderState,
    "kebab-case",
    SyntheticPreview,
    Planned,
    LocalReady,
    LocalPartial,
    NotDetected
);
enumeration!(
    DemoScenario,
    "kebab-case",
    Local,
    Ready,
    Missing,
    Negative,
    Empty
);
enumeration!(
    SyncState,
    "kebab-case",
    NeverSynced,
    Idle,
    Syncing,
    Cancelled,
    Failed
);
enumeration!(QuotaKind, "kebab-case", FiveHour, Weekly);
enumeration!(
    ErrorCode,
    "SCREAMING_SNAKE_CASE",
    InvalidData,
    InvalidQuery,
    Storage,
    StorageSchemaNewer,
    ApiVersionUnsupported,
    Internal,
    Cancelled,
    SyncBusy,
    SourceUnreadable,
    InvalidDirectory,
    NetworkDisabled,
    AuthRequired,
    AccessDenied,
    EndpointUnavailable,
    RateLimited,
    Offline,
    ResponseIncompatible
);
enumeration!(
    ReasonCode,
    "SCREAMING_SNAKE_CASE",
    SyntheticData,
    CalibrationNotRun,
    MissingComponent,
    IdentityUnknown,
    AccountMismatch,
    ProviderMismatch,
    ProductMismatch,
    TimeBasisUnknown,
    DateMismatch,
    CoverageIncomplete,
    SourceMismatch,
    PendingBackfill,
    FreshnessUnknown,
    NegativeResidual,
    InvalidToken,
    InconsistentComponents,
    NoData,
    WindowNotReported,
    NetworkDisabled,
    AccountLedgerUnavailable,
    ThreadIdentityMissing,
    InitialCounterBaseline,
    CounterReset,
    CounterGap,
    RepeatedCounterConflict,
    InheritedHistory,
    ChildScopeUnverified,
    CrossDayInterval,
    EventOwnershipAmbiguous,
    MetadataConflict,
    InvalidRecord,
    RecordTooLarge,
    SourceRewritten,
    SourceMissing,
    SourceChangedDuringRead,
    DiscoveryIncomplete,
    ImportedSource,
    ProductScopeUnverified,
    ModelUnknown,
    SessionListLimited,
    HistoryDayMissing,
    QuotaStale,
    ResponseIncompatible
);

enumeration!(
    AccountState,
    "kebab-case",
    Disabled,
    NeverSynced,
    Cached,
    Ready,
    Partial,
    AuthRequired,
    AccessDenied,
    EndpointUnavailable,
    RateLimited,
    Offline,
    Incompatible
);
dto!(AccountQueriesRequest { enabled: bool });
dto!(AccountSyncRequest {
    start_date: String,
    end_date: String
});
dto!(AccountSummary {
    enabled: bool, account_ref: Option<String>, state: AccountState,
    last_attempt_at: Option<String>, history_fetched_at: Option<String>, quota_fetched_at: Option<String>,
    history_error: Option<ErrorCode>, quota_error: Option<ErrorCode>, next_allowed_at: Option<String>,
    requested_start: Option<String>, requested_end: Option<String>, missing_dates: Vec<String>
});

/// Token metrics only. Canonical decimal strings, 0..=i64::MAX. Null requires a reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metric<T> {
    pub value: Option<T>,
    pub quality: QualityStatus,
    pub reason_codes: Vec<ReasonCode>,
}

dto!(TokenComponents {
    uncached_input: Metric<String>, cached_input: Metric<String>, inclusive_input: Metric<String>,
    output: Metric<String>, reasoning_output: Metric<String>, text_total: Metric<String>
});
dto!(Scope { provider_id: String, product_id: String, account_ref: Option<String>, identity_evidence: IdentityEvidence });
dto!(DayBucket { date_label: String, basis: TimeBasis, timezone: Option<String>, start_utc: Option<String>, end_utc: Option<String> });
dto!(Source {
    source_id: String,
    kind: SourceKind,
    collected_at: String,
    data_mode: DataMode
});
dto!(Quality { status: QualityStatus, coverage: CoverageStatus, freshness: FreshnessStatus, reason_codes: Vec<ReasonCode> });
dto!(LedgerRow {
    row_id: String,
    ledger: LedgerKind,
    scope: Scope,
    day: DayBucket,
    source: Source,
    tokens: TokenComponents,
    quality: Quality
});
// Signed diagnostic differences, never a calibrated device estimate in API v0.1.
dto!(ComparisonResult { scope: Scope, account_row_id: String, local_row_id: String, date_label: String, status: ComparisonStatus, uncached_input: Option<String>, cached_input: Option<String>, output: Option<String>, text_total: Option<String>, reason_codes: Vec<ReasonCode> });
dto!(ProviderCapabilities {
    local_ledger: bool,
    account_ledger: bool,
    incremental_collection: bool,
    calibrated_estimate: bool
});
dto!(Provider {
    provider_id: String,
    display_name: String,
    state: ProviderState,
    capabilities: ProviderCapabilities
});
// Percent has its own fields and cannot enter TokenComponents.
dto!(QuotaWindow { kind: QuotaKind, window_seconds: u32, used_percent: Option<String>, resets_at: Option<String>, reason_codes: Vec<ReasonCode> });
dto!(QuotaSnapshot { snapshot_id: String, scope: Scope, source: Source, windows: Vec<QuotaWindow> });
dto!(LocalModel { model_id: String, scope: Scope, text_total: Metric<String>, sessions: u32 });
dto!(LocalSession { session_ref: String, model_id: Option<String>, effort: Option<String>, speed: Option<String>, scope: Scope, text_total: Metric<String> });
dto!(RuntimeInfo {
    app_version: String,
    schema_version: u32,
    storage: String,
    installation_ref: String,
    network_enabled: bool,
    account_notice_pending: bool,
    local_collection_enabled: bool
});
dto!(SyncReport { files_found: u32, files_parsed: u32, files_unchanged: u32, files_rebuilt: u32, new_snapshots: u32, duplicate_snapshots: u32, bytes_parsed: String, bytes_verified: String, partial_tails: u32, reason_codes: Vec<ReasonCode> });
dto!(CollectorSummary { state: SyncState, last_sync_at: Option<String>, files_indexed: u32, retained_sources: u32, snapshots: u32, accepted_events: u32, ambiguous_events: u32, sessions_total: u32, reason_codes: Vec<ReasonCode>, last_run: Option<SyncReport> });
dto!(LocalRootsRequest { roots: Vec<String> });
dto!(Dashboard { api_version: String, data_mode: DataMode, display_timezone: String, providers: Vec<Provider>, account_rows: Vec<LedgerRow>, local_rows: Vec<LedgerRow>, comparisons: Vec<ComparisonResult>, quota: Option<QuotaSnapshot>, models: Vec<LocalModel>, sessions: Vec<LocalSession>, runtime: RuntimeInfo, collector: Option<CollectorSummary>, account: Option<AccountSummary> });
dto!(DashboardQuery {
    scenario: DemoScenario
});
dto!(ApiError {
    api_version: String,
    code: ErrorCode,
    message: String,
    retryable: bool
});
