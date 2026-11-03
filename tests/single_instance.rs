use raspiboard::storage::Store;
use std::{
    fs,
    io::{self, BufRead, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

#[cfg(unix)]
#[test]
fn duplicate_launch_exits_before_loading_settings_or_opening_a_window() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/single-instance");
    fs::create_dir_all(&root).unwrap();
    let home = tempfile::tempdir_in(root).unwrap();
    let data = home.path().join(".raspiboard");
    let _lock = Store::new(&data).try_lock().unwrap().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_raspiboard"))
        .env("HOME", home.path())
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .current_dir(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Raspiboard is already running."));
    assert!(!data.join("settings.toml").exists());
    assert!(!data.join("categories").exists());
}

#[test]
fn lock_is_released_when_owner_is_terminated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/single-instance");
    fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let mut owner = Owner(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "instance_lock_child", "--nocapture"])
            .env("RASPIBOARD_TEST_LOCK_ROOT", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut output = io::BufReader::new(owner.0.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert_ne!(
            output.read_line(&mut line).unwrap(),
            0,
            "owner exited early"
        );
        if line.trim() == "locked" {
            break;
        }
    }

    let store = Store::new(dir.path());
    assert!(store.try_lock().unwrap().is_none());
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    assert!(dir.path().join("instance.lock").exists());
    assert!(store.try_lock().unwrap().is_some());
}

#[test]
fn instance_lock_child() {
    let Some(root) = std::env::var_os("RASPIBOARD_TEST_LOCK_ROOT") else {
        return;
    };
    let _lock = Store::new(root).try_lock().unwrap().unwrap();
    println!("locked");
    io::stdout().flush().unwrap();
    let _ = io::stdin().read(&mut [0]);
}

struct Owner(Child);

impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
