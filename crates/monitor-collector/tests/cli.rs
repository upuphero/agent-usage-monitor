use std::{path::PathBuf, process::Command};
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "collector-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn packaged_cli_collects_synthetic_logs_with_stable_identity_and_no_overwrite() {
    let root = root();
    let home = root.join("home");
    std::fs::create_dir_all(home.join("sessions")).unwrap();
    let state = root.join("state");
    let logs = concat!("{\"type\":\"session_meta\",\"timestamp\":\"2026-10-01T08:00:00Z\",\"payload\":{\"id\":\"synthetic-thread\"}}\n",
        "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-01T08:01:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"total_tokens\":26},\"last_token_usage\":{\"input_tokens\":20,\"cached_input_tokens\":8,\"output_tokens\":6,\"total_tokens\":26}}}}\n");
    std::fs::write(home.join("sessions/fixture.jsonl"), logs).unwrap();
    // Deliberately unreadable/non-JSON credentials must never be inspected by this command.
    std::fs::write(home.join("auth.json"), "not credentials").unwrap();
    let run = |name| {
        Command::new(env!("CARGO_BIN_EXE_monitor-collector"))
            .arg("collect")
            .arg("--home")
            .arg(&home)
            .arg("--state")
            .arg(&state)
            .arg("--output")
            .arg(root.join(name))
            .output()
            .unwrap()
    };
    let first = run("one.json");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(run("two.json").status.success());
    assert!(!run("one.json").status.success());
    let read = |name| {
        monitor_core::devices::decode_report(&std::fs::read(root.join(name)).unwrap()).unwrap()
    };
    let one = read("one.json");
    let two = read("two.json");
    assert_eq!(one.device, two.device);
    assert_eq!(
        one.device.platform,
        monitor_core::devices::native_platform()
    );
    assert_eq!(one.events.len(), 1);
    assert_eq!(one.events[0].tokens.text_total.value.as_deref(), Some("26"));
    assert_eq!(one.events, two.events);
}
