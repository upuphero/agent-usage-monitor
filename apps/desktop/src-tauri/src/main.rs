#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args().any(|argument| argument == "--verify-synthetic-store") {
        if monitor_desktop::verify_synthetic_store().is_err() {
            std::process::exit(1);
        }
        return;
    }
    monitor_desktop::run();
}
