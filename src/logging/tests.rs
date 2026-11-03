use super::*;
use log::{Level, Log, Record};
use std::{
    fs,
    time::{Duration, SystemTime},
};

fn fixture() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/logging");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn configured_logger(directory: &Path) -> Logger {
    logger(
        directory,
        LogSpecification::parse("warn,raspiboard=info").unwrap(),
    )
}

fn write_log(logger: &dyn Log, level: Level, message: &str) {
    logger.log(
        &Record::builder()
            .args(format_args!("{message}"))
            .level(level)
            .target("raspiboard::audio")
            .build(),
    );
    logger.flush();
}

fn old_file(directory: &Path, name: &str, days: u64) -> std::path::PathBuf {
    let path = directory.join(name);
    let file = fs::File::create(&path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(days * 86400))
        .unwrap();
    path
}

#[test]
fn creates_logs_with_timestamp_channel_level_and_default_filter() {
    let dir = fixture();
    let logs = dir.path().join("logs");
    let (logger, handle) = configured_logger(&logs).build().unwrap();
    write_log(&*logger, Level::Info, "Audio connected");
    write_log(&*logger, Level::Error, "Playback failed");
    write_log(&*logger, Level::Debug, "Debug details");
    handle.shutdown();

    let text = fs::read_to_string(logs.join("raspiboard_rCURRENT.log")).unwrap();
    assert!(
        text.contains("] raspiboard::audio.INFO: Audio connected\n"),
        "{text}"
    );
    assert!(
        text.contains("] raspiboard::audio.ERROR: Playback failed\n"),
        "{text}"
    );
    assert!(!text.contains("Debug details"));
    let timestamp = &text[1..24];
    assert_eq!(timestamp.len(), 23);
    for (index, separator) in [
        (4, '-'),
        (7, '-'),
        (10, ' '),
        (13, ':'),
        (16, ':'),
        (19, '.'),
    ] {
        assert_eq!(timestamp.chars().nth(index), Some(separator));
    }
}

#[test]
fn startup_removes_logs_older_than_seven_days_and_preserves_other_files() {
    let dir = fixture();
    let expired = old_file(dir.path(), "raspiboard_r2000-01-01_00-00-00.log", 8);
    let recent = old_file(dir.path(), "raspiboard_r2000-01-02_00-00-00.log", 6);
    let unrelated = old_file(dir.path(), "notes.txt", 30);
    let (logger, handle) = configured_logger(dir.path()).build().unwrap();
    write_log(&*logger, Level::Info, "Starting");
    handle.shutdown();

    assert!(!expired.exists());
    assert!(recent.exists());
    assert!(unrelated.exists());
}

#[test]
fn rotation_preserves_previous_messages_and_cleans_expired_files() {
    let dir = fixture();
    let (logger, handle) = configured_logger(dir.path()).build().unwrap();
    write_log(&*logger, Level::Info, "Before rotation");
    let expired = old_file(dir.path(), "raspiboard_r2000-01-01_00-00-00.log", 8);
    handle.trigger_rotation().unwrap();
    write_log(&*logger, Level::Warn, "After rotation");
    handle.shutdown();

    assert!(!expired.exists());
    let current = dir.path().join("raspiboard_rCURRENT.log");
    let text = fs::read_to_string(&current).unwrap();
    assert!(text.contains("After rotation"));
    assert!(!text.contains("Before rotation"));
    let archives: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| *path != current)
        .collect();
    assert_eq!(archives.len(), 1);
    assert!(
        fs::read_to_string(&archives[0])
            .unwrap()
            .contains("Before rotation")
    );
}

#[test]
fn restarting_preserves_previous_session_logs() {
    let dir = fixture();
    let current = dir.path().join("raspiboard_rCURRENT.log");
    fs::write(&current, "Previous session\n").unwrap();
    let (logger, handle) = configured_logger(dir.path()).build().unwrap();
    write_log(&*logger, Level::Info, "New session");
    handle.shutdown();

    let contents: String = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect();
    assert!(contents.contains("Previous session"));
    assert!(contents.contains("New session"));
}

#[test]
fn unusable_log_directory_reports_an_error() {
    let dir = fixture();
    let path = dir.path().join("logs");
    fs::write(&path, "occupied").unwrap();
    assert!(configured_logger(&path).build().is_err());
}
