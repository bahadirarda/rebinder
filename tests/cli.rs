use std::process::Command;

#[test]
fn version_uses_the_canonical_calendar_identity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .arg("--version")
        .output()
        .expect("run rebinder --version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("rebinder {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn validate_command_supports_json_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["validate", "examples/minimal-session", "--json"])
        .output()
        .expect("run rebinder");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("decode output");
    assert_eq!(value["valid"], true);
    assert_eq!(value["schemaVersion"], "0.1.0");
}

#[cfg(unix)]
#[test]
fn codex_command_forwards_arguments_unchanged() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let bin_directory = tempfile::tempdir().expect("create fake bin directory");
    let executable = bin_directory.path().join("codex");
    fs::write(&executable, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").expect("write fake codex");
    let mut permissions = fs::metadata(&executable)
        .expect("read fake codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fake codex executable");

    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["codex", "resume", "--last"])
        .env("PATH", bin_directory.path())
        .output()
        .expect("run rebinder codex");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "resume\n--last\n");
}

#[test]
fn cross_harness_transfer_interface_is_reserved() {
    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "claude",
            "--to",
            "codex",
            "session-123",
            "--",
            "--full-auto",
        ])
        .output()
        .expect("run cross-harness transfer");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("claude session session-123 -> codex"));
    assert!(stderr.contains("1 target argument(s)"));
}
