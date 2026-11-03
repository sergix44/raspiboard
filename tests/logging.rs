use raspiboard::{logging, storage::Store};
use std::{fs, path::Path, process::Command};

fn child(mode: &str, filter: Option<&str>) -> (tempfile::TempDir, std::process::Output) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/logging");
    fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "logging_child", "--nocapture"])
        .env("RASPIBOARD_TEST_LOG_ROOT", dir.path())
        .env("RASPIBOARD_TEST_LOG_MODE", mode)
        .env_remove("RUST_LOG");
    if let Some(filter) = filter {
        command.env("RUST_LOG", filter);
    }
    let output = command.output().unwrap();
    (dir, output)
}

#[test]
fn initialization_writes_to_the_store_logs_directory() {
    let (dir, output) = child("normal", None);
    assert!(output.status.success(), "{output:?}");
    let text = fs::read_to_string(dir.path().join("logs/raspiboard_rCURRENT.log")).unwrap();
    assert!(text.contains("Logging initialized"));
    assert!(!text.contains("Debug diagnostic"));
}

#[test]
fn rust_log_can_enable_debug_diagnostics() {
    let (dir, output) = child("normal", Some("raspiboard=debug"));
    assert!(output.status.success(), "{output:?}");
    let text = fs::read_to_string(dir.path().join("logs/raspiboard_rCURRENT.log")).unwrap();
    assert!(text.contains("raspiboard::test.DEBUG: Debug diagnostic"));
}

#[test]
fn panics_are_written_to_the_log_and_still_reported_on_stderr() {
    let (dir, output) = child("panic", None);
    assert!(!output.status.success());
    let text = fs::read_to_string(dir.path().join("logs/raspiboard_rCURRENT.log")).unwrap();
    assert!(text.contains("raspiboard::panic.ERROR:"), "{text}");
    assert!(text.contains("Logging panic diagnostic"), "{text}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Logging panic diagnostic"));
}

#[test]
fn logging_child() {
    let Some(root) = std::env::var_os("RASPIBOARD_TEST_LOG_ROOT") else {
        return;
    };
    let store = Store::new(root);
    let handle = logging::init(&store.logs_directory()).unwrap();
    log::info!(target: "raspiboard::test", "Logging initialized");
    log::debug!(target: "raspiboard::test", "Debug diagnostic");
    if std::env::var("RASPIBOARD_TEST_LOG_MODE").unwrap() == "panic" {
        panic!("Logging panic diagnostic");
    }
    handle.shutdown();
}
