#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    type Command = fn() -> Result<(), monitor_contracts::ApiError>;
    let command: Option<Command> = if args.iter().any(|arg| arg == "--verify-synthetic-store") {
        Some(monitor_desktop::verify_synthetic_store)
    } else if args.iter().any(|arg| arg == "--verify-local-collector") {
        Some(monitor_desktop::verify_local_collector)
    } else if args.iter().any(|arg| arg == "--sync-local") {
        Some(monitor_desktop::collect_local_report)
    } else if args.iter().any(|arg| arg == "--inspect-local-cache") {
        Some(monitor_desktop::inspect_local_cache)
    } else {
        None
    };
    if let Some(command) = command {
        if command().is_err() {
            std::process::exit(1);
        }
        return;
    }
    monitor_desktop::run();
}
