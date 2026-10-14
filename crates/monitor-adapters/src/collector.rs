//! Read-only discovery and durable, complete-line incremental parsing.
use crate::{storage_error, Store};
use monitor_contracts::*;
use monitor_core::{
    error,
    local::{add_reason, opaque_id, ParserState, Snapshot, PARSER_VERSION},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

const GUARD_BYTES: u64 = 4096;
const MAX_LINE: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct Candidate {
    pub source_id: String,
    pub root_ref: String,
    pub path: PathBuf,
    pub boundary: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Checkpoint {
    pub source_id: String,
    pub root_ref: String,
    pub parser_version: String,
    pub offset: u64,
    pub size: u64,
    pub modified: String,
    pub digest: String,
    pub head: String,
    pub tail: String,
    pub state: ParserState,
}
pub(crate) struct ParsedSource {
    pub checkpoint: Checkpoint,
    pub snapshots: Vec<Snapshot>,
    pub rebuilt: bool,
    pub partial_tail: bool,
    pub bytes_parsed: u64,
    pub bytes_verified: u64,
}

fn reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
fn path_id(path: &Path, kind: &str) -> String {
    let text = path.to_string_lossy();
    opaque_id(
        kind,
        &if cfg!(windows) {
            text.to_lowercase()
        } else {
            text.into_owned()
        },
    )
}
pub fn default_homes() -> Vec<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|profile| PathBuf::from(profile).join(".codex"))
        })
        .into_iter()
        .collect()
}
pub(crate) fn discover(homes: &[PathBuf]) -> (Vec<Candidate>, Vec<ReasonCode>) {
    fn walk(
        root: &Path,
        directory: &Path,
        depth: usize,
        candidates: &mut Vec<Candidate>,
        reasons: &mut Vec<ReasonCode>,
    ) {
        if depth > 64 {
            add_reason(reasons, ReasonCode::DiscoveryIncomplete);
            return;
        }
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => {
                add_reason(reasons, ReasonCode::DiscoveryIncomplete);
                return;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    add_reason(reasons, ReasonCode::DiscoveryIncomplete);
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    add_reason(reasons, ReasonCode::DiscoveryIncomplete);
                    continue;
                }
            };
            if reparse(&metadata) {
                add_reason(reasons, ReasonCode::DiscoveryIncomplete);
                continue;
            }
            if metadata.is_dir() {
                walk(root, &path, depth + 1, candidates, reasons);
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
            {
                let canonical = match path.canonicalize() {
                    Ok(path) => path,
                    Err(_) => {
                        add_reason(reasons, ReasonCode::DiscoveryIncomplete);
                        continue;
                    }
                };
                if canonical.starts_with(root) {
                    candidates.push(Candidate {
                        source_id: path_id(&canonical, "source"),
                        root_ref: path_id(root, "root"),
                        path: canonical,
                        boundary: root.to_owned(),
                    });
                }
            }
        }
    }
    let mut candidates = Vec::new();
    let mut reasons = Vec::new();
    for home in homes {
        for name in ["sessions", "archived_sessions", "session-cleanup-archive"] {
            let directory = home.join(name);
            match fs::symlink_metadata(&directory) {
                Ok(metadata) if metadata.is_dir() && !reparse(&metadata) => {
                    match directory.canonicalize() {
                        Ok(root) => walk(&root, &root, 0, &mut candidates, &mut reasons),
                        Err(_) => add_reason(&mut reasons, ReasonCode::DiscoveryIncomplete),
                    }
                }
                Ok(_) => add_reason(&mut reasons, ReasonCode::DiscoveryIncomplete),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => add_reason(&mut reasons, ReasonCode::DiscoveryIncomplete),
            }
        }
    }
    candidates.sort_by(|a, b| a.source_id.cmp(&b.source_id));
    candidates.dedup_by(|a, b| a.source_id == b.source_id);
    (candidates, reasons)
}
fn modified(metadata: &Metadata) -> String {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|| "unknown".into())
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), ApiError> {
    if cancel.load(Ordering::Relaxed) {
        Err(error(
            ErrorCode::Cancelled,
            "Local synchronization cancelled",
        ))
    } else {
        Ok(())
    }
}
fn hash_region(
    file: &mut File,
    start: u64,
    length: u64,
    cancel: &AtomicBool,
) -> Result<Sha256, ApiError> {
    file.seek(SeekFrom::Start(start)).map_err(storage_error)?;
    let mut remaining = length;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        check_cancel(cancel)?;
        let requested = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..requested]).map_err(storage_error)?;
        if read == 0 {
            return Err(error(
                ErrorCode::SourceUnreadable,
                "Source changed during integrity check",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(hasher)
}
fn digest(hasher: &Sha256) -> String {
    format!("{:x}", hasher.clone().finalize())
}
fn guards(file: &mut File, offset: u64, cancel: &AtomicBool) -> Result<(String, String), ApiError> {
    let length = offset.min(GUARD_BYTES);
    Ok((
        digest(&hash_region(file, 0, length, cancel)?),
        digest(&hash_region(file, offset - length, length, cancel)?),
    ))
}
fn source_file(candidate: &Candidate) -> Result<File, ApiError> {
    // The source is a regular .jsonl within a known session directory, not an arbitrary filename.
    if candidate
        .path
        .extension()
        .is_none_or(|extension| extension != "jsonl")
        || !candidate.path.starts_with(&candidate.boundary)
    {
        return Err(error(
            ErrorCode::SourceUnreadable,
            "Source is outside the session boundary",
        ));
    }
    let metadata = fs::symlink_metadata(&candidate.path).map_err(storage_error)?;
    if !metadata.is_file() || reparse(&metadata) {
        return Err(error(
            ErrorCode::SourceUnreadable,
            "Source is not a regular session log",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let file = options
        .open(&candidate.path)
        .map_err(|_| error(ErrorCode::SourceUnreadable, "Session log is unavailable"))?;
    if reparse(&file.metadata().map_err(storage_error)?) {
        return Err(error(
            ErrorCode::SourceUnreadable,
            "Reparse sources are not followed",
        ));
    }
    Ok(file)
}

/// Unchanged files use small guards. Modified files verify the old prefix without reparsing it.
pub(crate) fn read_incremental(
    candidate: &Candidate,
    old: Option<Checkpoint>,
    cancel: &AtomicBool,
) -> Result<Option<ParsedSource>, ApiError> {
    check_cancel(cancel)?;
    let mut file = source_file(candidate)?;
    let before = file.metadata().map_err(storage_error)?;
    let size = before.len();
    let tag = modified(&before);
    let mut offset = 0;
    let mut state = ParserState::default();
    let mut hasher = Sha256::new();
    let mut verified = 0;
    let mut rebuilt = false;
    if let Some(old) = old {
        if old.parser_version == PARSER_VERSION && old.offset <= size {
            let (head, tail) = guards(&mut file, old.offset, cancel)?;
            verified += old.offset.min(GUARD_BYTES) * 2;
            let guarded = head == old.head && tail == old.tail;
            if guarded && size == old.size && tag == old.modified && tag != "unknown" {
                return Ok(None);
            }
            if guarded {
                let prefix = hash_region(&mut file, 0, old.offset, cancel)?;
                verified += old.offset;
                if digest(&prefix) == old.digest {
                    offset = old.offset;
                    state = old.state;
                    hasher = prefix;
                } else {
                    rebuilt = true;
                }
            } else {
                rebuilt = true;
            }
        } else {
            rebuilt = true;
        }
    }
    if rebuilt {
        add_reason(&mut state.reason_codes, ReasonCode::SourceRewritten);
    }
    file.seek(SeekFrom::Start(offset)).map_err(storage_error)?;
    let mut reader = BufReader::new((&mut file).take(size - offset));
    let mut line = Vec::new();
    let mut oversized = false;
    let mut snapshots = Vec::new();
    let mut consumed = offset;
    let mut committed = offset;
    let mut committed_hasher = hasher.clone();
    let mut incomplete = false;
    loop {
        check_cancel(cancel)?;
        let buffer = reader.fill_buf().map_err(storage_error)?;
        if buffer.is_empty() {
            if consumed > committed {
                incomplete = true;
            }
            break;
        }
        let length = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(buffer.len());
        let complete = buffer[length - 1] == b'\n';
        hasher.update(&buffer[..length]);
        consumed += length as u64;
        if !oversized {
            if line.len() + length > MAX_LINE {
                oversized = true;
                line.clear();
            } else {
                line.extend_from_slice(&buffer[..length]);
            }
        }
        reader.consume(length);
        if complete {
            if oversized {
                add_reason(&mut state.reason_codes, ReasonCode::RecordTooLarge);
                state.invalid_records = state.invalid_records.saturating_add(1);
            } else if let Some(snapshot) = state.project_line(&line) {
                snapshots.push(snapshot);
            }
            committed = consumed;
            committed_hasher = hasher.clone();
            line.clear();
            oversized = false;
        }
    }
    drop(reader);
    let after = file.metadata().map_err(storage_error)?;
    if after.len() != size || modified(&after) != tag {
        let current = hash_region(&mut file, 0, committed, cancel)?;
        verified += committed;
        if digest(&current) != digest(&committed_hasher) {
            return Err(error(
                ErrorCode::SourceUnreadable,
                "Session log changed during reading",
            ));
        }
        // Safe concurrent appends are deferred to the next synchronization.
    }
    let (head, tail) = guards(&mut file, committed, cancel)?;
    verified += committed.min(GUARD_BYTES) * 2;
    let checkpoint = Checkpoint {
        source_id: candidate.source_id.clone(),
        root_ref: candidate.root_ref.clone(),
        parser_version: PARSER_VERSION.into(),
        offset: committed,
        size,
        modified: tag,
        digest: digest(&committed_hasher),
        head,
        tail,
        state,
    };
    Ok(Some(ParsedSource {
        checkpoint,
        snapshots,
        rebuilt,
        partial_tail: incomplete,
        bytes_parsed: consumed - offset,
        bytes_verified: verified,
    }))
}

impl Store {
    pub fn synchronize_local(
        &mut self,
        homes: &[PathBuf],
        cancel: &AtomicBool,
    ) -> Result<SyncReport, ApiError> {
        let (candidates, mut reasons) = discover(homes);
        let ids: BTreeSet<String> = candidates
            .iter()
            .map(|candidate| candidate.source_id.clone())
            .collect();
        let mut report = SyncReport {
            files_found: candidates.len().try_into().map_err(storage_error)?,
            files_parsed: 0,
            files_unchanged: 0,
            files_rebuilt: 0,
            new_snapshots: 0,
            duplicate_snapshots: 0,
            bytes_parsed: "0".into(),
            bytes_verified: "0".into(),
            partial_tails: 0,
            reason_codes: Vec::new(),
        };
        let mut parsed_bytes = 0u64;
        let mut verified_bytes = 0u64;
        self.record_sync_state(SyncState::Syncing, None)?;
        for candidate in candidates {
            if let Err(cancelled) = check_cancel(cancel) {
                self.record_sync_state(SyncState::Cancelled, None)?;
                return Err(cancelled);
            }
            let old = self.checkpoint(&candidate.source_id)?;
            let pending_tail = old
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.offset < checkpoint.size);
            match read_incremental(&candidate, old, cancel) {
                Ok(None) => {
                    report.files_unchanged += 1;
                    if pending_tail {
                        report.partial_tails += 1;
                    }
                    self.mark_present(&candidate.source_id)?;
                }
                Ok(Some(parsed)) => {
                    check_cancel(cancel).inspect_err(|_| {
                        let _ = self.record_sync_state(SyncState::Cancelled, None);
                    })?;
                    parsed_bytes += parsed.bytes_parsed;
                    verified_bytes += parsed.bytes_verified;
                    report.files_parsed += 1;
                    if parsed.rebuilt {
                        report.files_rebuilt += 1;
                    }
                    if parsed.partial_tail {
                        report.partial_tails += 1;
                    }
                    for reason in &parsed.checkpoint.state.reason_codes {
                        add_reason(&mut reasons, *reason);
                    }
                    let (new, duplicate) = self.commit_source(parsed)?;
                    report.new_snapshots += new;
                    report.duplicate_snapshots += duplicate;
                }
                Err(e) if e.code == ErrorCode::Cancelled => {
                    self.record_sync_state(SyncState::Cancelled, None)?;
                    return Err(e);
                }
                Err(_) => {
                    add_reason(&mut reasons, ReasonCode::DiscoveryIncomplete);
                    add_reason(&mut reasons, ReasonCode::SourceChangedDuringRead);
                }
            }
        }
        if !reasons.contains(&ReasonCode::DiscoveryIncomplete) {
            self.mark_missing(&ids)?;
        }
        report.bytes_parsed = parsed_bytes.to_string();
        report.bytes_verified = verified_bytes.to_string();
        report.reason_codes = reasons;
        self.finish_sync(&report)?;
        Ok(report)
    }
}
