use std::time::{Duration, SystemTime};

use assert_fs::TempDir;
use moonup::utils::replace_exe;

fn shim_name() -> &'static str {
    if cfg!(windows) { "moon.exe" } else { "moon" }
}

#[test]
fn replace_exe_pours_new_content() {
    let tempdir = TempDir::new().expect("should create tempdir");
    let new = tempdir.path().join("new-shim");
    let old = tempdir.path().join(shim_name());

    std::fs::write(&new, b"fresh shim").expect("should write new shim");
    std::fs::write(&old, b"stale shim").expect("should write old shim");

    replace_exe(&new, &old).expect("replace should succeed");

    assert_eq!(
        std::fs::read(&old).expect("should read dest shim"),
        b"fresh shim"
    );
}

#[test]
fn replace_exe_pours_when_dest_missing() {
    let tempdir = TempDir::new().expect("should create tempdir");
    let new = tempdir.path().join("new-shim");
    let old = tempdir.path().join(shim_name());

    std::fs::write(&new, b"fresh shim").expect("should write new shim");
    assert!(!old.exists(), "dest should start missing");

    replace_exe(&new, &old).expect("replace should succeed");

    assert_eq!(
        std::fs::read(&old).expect("should read dest shim"),
        b"fresh shim"
    );
}

#[test]
fn replace_exe_skips_identical_content() {
    let tempdir = TempDir::new().expect("should create tempdir");
    let new = tempdir.path().join("new-shim");
    let old = tempdir.path().join(shim_name());

    std::fs::write(&new, b"shim bytes").expect("should write new shim");
    std::fs::write(&old, b"shim bytes").expect("should write old shim");

    let past = SystemTime::now() - Duration::from_secs(3600);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&old)
        .expect("should open old shim")
        .set_times(std::fs::FileTimes::new().set_modified(past))
        .expect("should set old mtime");

    replace_exe(&new, &old).expect("replace should succeed");

    assert_eq!(
        std::fs::read(&old).expect("should read dest shim"),
        b"shim bytes"
    );
    let modified = std::fs::metadata(&old)
        .expect("should stat dest shim")
        .modified()
        .expect("should read mtime");
    assert!(
        modified < past + Duration::from_secs(10),
        "identical shim should be left untouched"
    );

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&old)
            .expect("should stat dest shim")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o755,
            "skip should still normalize permissions like a replace"
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn replace_exe_hold_guard() {
    use std::process::Command;

    // Child mode: keep a real running image mapped so the parent can exercise
    // Windows' rename-ok / delete-denied semantics. The parent selects this
    // mode with `MOONUP_TEST_HOLD` and the test name filter.
    if std::env::var_os("MOONUP_TEST_HOLD").is_some() {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }

    let tempdir = TempDir::new().expect("should create tempdir");
    let exe = std::env::current_exe().expect("should resolve current exe");
    let old = tempdir.path().join("moon.exe");
    let new = tempdir.path().join("new-shim.exe");

    // `old` is a real running image: it can be renamed but not deleted.
    std::fs::copy(&exe, &old).expect("should copy current exe");
    std::fs::write(&new, b"fresh shim bytes").expect("should write new shim");

    let mut child = Command::new(&old)
        .arg("replace_exe_hold_guard")
        .env("MOONUP_TEST_HOLD", "1")
        .spawn()
        .expect("should spawn holder child");
    std::thread::sleep(Duration::from_millis(500));

    replace_exe(&new, &old).expect("replace should tolerate an in-use dest");
    assert_eq!(
        std::fs::read(&old).expect("should read dest shim"),
        b"fresh shim bytes"
    );

    let trash = tempdir.path().join(".trash");
    assert!(trash.exists(), "in-use retired shim should wait in .trash");

    child.kill().expect("should kill holder child");
    child.wait().expect("should reap holder child");

    std::fs::write(&new, b"another shim bytes").expect("should write newer shim");
    replace_exe(&new, &old).expect("replace should succeed");
    assert!(
        !trash.exists(),
        "trash should be swept once the holder exits"
    );
}
