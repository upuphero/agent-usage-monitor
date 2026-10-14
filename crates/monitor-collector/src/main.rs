//! Standalone Windows/macOS/Linux log collector. No account API or credential reads.
use monitor_adapters::{collector::default_homes, Store};
use monitor_contracts::{ApiError, ErrorCode};
use monitor_core::{devices::MAX_REPORT_BYTES, error};
use std::{path::PathBuf, sync::atomic::AtomicBool};

fn collect(args: &[String]) -> Result<(), ApiError> {
    let mut output = None;
    let mut state = None;
    let mut homes = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| error(ErrorCode::InvalidQuery, "Option needs a path"))?;
        match args[index].as_str() {
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--state" if state.is_none() => state = Some(PathBuf::from(value)),
            "--home" => homes.push(PathBuf::from(value)),
            _ => {
                return Err(error(
                    ErrorCode::InvalidQuery,
                    "Unknown or duplicate option",
                ))
            }
        }
        index += 2;
    }
    let output = output.ok_or_else(|| error(ErrorCode::InvalidQuery, "--output is required"))?;
    if output.exists() {
        return Err(error(
            ErrorCode::InvalidQuery,
            "Output already exists; choose a new report filename",
        ));
    }
    let state = state
        .or_else(|| {
            std::env::var_os(if cfg!(windows) {
                "LOCALAPPDATA"
            } else {
                "HOME"
            })
            .map(|base| PathBuf::from(base).join(".agent-usage-monitor-collector"))
        })
        .ok_or_else(|| error(ErrorCode::InvalidQuery, "--state is required on this host"))?;
    if homes.is_empty() {
        homes = default_homes();
    }
    if homes.is_empty()
        || homes
            .iter()
            .any(|home| !home.is_absolute() || !home.is_dir())
    {
        return Err(error(
            ErrorCode::InvalidDirectory,
            "No readable Codex home; specify an absolute --home",
        ));
    }
    std::fs::create_dir_all(&state)
        .map_err(|_| error(ErrorCode::Storage, "Collector cache directory unavailable"))?;
    let mut store = Store::open(&state.join("collector.sqlite3"))?;
    store.synchronize_local(&homes, &AtomicBool::new(false))?;
    let report = store.export_device_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|_| error(ErrorCode::Internal, "Report encoding failed"))?;
    if bytes.len() > MAX_REPORT_BYTES {
        return Err(error(
            ErrorCode::InvalidData,
            "Device report exceeds size limit",
        ));
    }
    // Create-new prevents replacing an existing file or following an output symlink.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|_| error(ErrorCode::Storage, "Could not create report file"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| error(ErrorCode::Storage, "Could not finish report file"))?;
    println!("Collected {} log records; {} ambiguous records excluded. Import this JSON in Agent Usage Monitor.", report.events.len(), report.excluded_events);
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("Agent Usage Monitor collector {}\ncollect --output <new-report.json> [--home <absolute-Codex-home>] [--state <cache-directory>]\nReads local session logs only. Reuse the cache directory to retain this device's identity. No account requests or credentials. Reports contain anonymous token records; keep them private.", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args == ["--version"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let result = if args[0] == "collect" {
        collect(&args)
    } else {
        Err(error(
            ErrorCode::InvalidQuery,
            "Expected collect, --help or --version",
        ))
    };
    if let Err(failure) = result {
        eprintln!("{}", failure.message);
        std::process::exit(1);
    }
}
