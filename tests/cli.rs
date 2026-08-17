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
fn unsupported_transfer_direction_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "codex",
            "--to",
            "claude",
            "session-123",
        ])
        .output()
        .expect("run cross-harness transfer");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("codex to claude transfer is not implemented yet"));
}

#[cfg(unix)]
#[test]
fn claude_to_codex_imports_and_resumes_in_the_recorded_workspace() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create transfer fixture");
    let bin_directory = fixture.path().join("bin");
    let workspace = fixture.path().join("worktree");
    let session_directory = fixture.path().join("claude-sessions");
    fs::create_dir_all(&bin_directory).expect("create fake bin directory");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&session_directory).expect("create session directory");

    let session_id = "11111111-2222-3333-4444-555555555555";
    let source_path = session_directory.join(format!("{session_id}.jsonl"));
    fs::write(&source_path, "{\"type\":\"user\"}\n").expect("write source session");
    let arguments_log = fixture.path().join("resume-arguments.txt");
    let cwd_log = fixture.path().join("resume-cwd.txt");
    let executable = bin_directory.join("codex");

    let source_json = serde_json::to_string(source_path.to_str().expect("UTF-8 source path"))
        .expect("encode source path");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace path"))
        .expect("encode workspace path");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "app-server" ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*)
        printf '%s\n' '{{"id":0,"result":{{}}}}'
        ;;
      *'"id":1'*)
        printf '%s\n' '{{"id":1,"result":{{"items":[{{"itemType":"SESSIONS","description":"Import Claude sessions","cwd":null,"details":{{"plugins":[],"skills":[],"sessions":[{{"path":{source_json},"cwd":{cwd_json},"title":"Fixture session"}}],"mcpServers":[],"hooks":[],"subagents":[],"commands":[]}}}}]}}}}'
        ;;
      *'"id":2'*)
        printf '%s\n' '{{"id":2,"result":{{"data":[]}}}}'
        ;;
      *'"id":3'*)
        printf '%s\n' '{{"id":3,"result":{{"importId":"import-1"}}}}'
        printf '%s\n' '{{"method":"externalAgentConfig/import/completed","params":{{"importId":"import-1","itemTypeResults":[{{"itemType":"SESSIONS","successes":[{{"itemType":"SESSIONS","cwd":{cwd_json},"source":{source_json},"target":"019c0000-0000-7000-8000-000000000001","title":"Fixture session"}}],"failures":[]}}]}}}}'
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = "resume" ]; then
  printf '%s\n' "$@" > "$FAKE_CODEX_ARGUMENTS_LOG"
  printf '%s\n' "$PWD" > "$FAKE_CODEX_CWD_LOG"
  exit 0
fi
exit 64
"#
    );
    fs::write(&executable, script).expect("write fake codex");
    let mut permissions = fs::metadata(&executable)
        .expect("read fake codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fake codex executable");

    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "claude",
            "--to",
            "codex",
            session_id,
            "--",
            "--model",
            "gpt-fixture",
        ])
        .current_dir(&workspace)
        .env("PATH", &bin_directory)
        .env("FAKE_CODEX_ARGUMENTS_LOG", &arguments_log)
        .env("FAKE_CODEX_CWD_LOG", &cwd_log)
        .output()
        .expect("run Claude-to-Codex transfer");

    assert!(
        output.status.success(),
        "transfer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&arguments_log).expect("read arguments log"),
        "resume\n019c0000-0000-7000-8000-000000000001\n--model\ngpt-fixture\n"
    );
    assert_eq!(
        fs::read_to_string(cwd_log).expect("read cwd log"),
        format!("{}\n", workspace.display())
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("imported Claude Code session"));
    assert!(stderr.contains(session_id));
}

#[cfg(unix)]
#[test]
fn context_safe_handoff_bounds_history_and_resumes_the_derived_thread() {
    use std::{fs, os::unix::fs::PermissionsExt};

    use sha2::{Digest, Sha256};

    let fixture = tempfile::tempdir().expect("create handoff fixture");
    let bin_directory = fixture.path().join("bin");
    let workspace = fixture.path().join("worktree");
    let session_directory = fixture.path().join(".claude/projects/project");
    let data_directory = fixture.path().join("data");
    fs::create_dir_all(&bin_directory).expect("create fake bin directory");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&session_directory).expect("create session directory");

    let session_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let source_path = session_directory.join(format!("{session_id}.jsonl"));
    fs::write(
        &source_path,
        concat!(
            "{\"type\":\"user\",\"message\":{\"content\":\"obsolete request\"}}\n",
            "{\"type\":\"user\",\"isCompactSummary\":true,\"message\":{\"content\":\"verified compact state\"}}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"recent visible answer\"}]}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"content\":\"private tool output\"}]}}\n"
        ),
    )
    .expect("write source session");

    let digest = Sha256::digest(source_path.to_string_lossy().as_bytes());
    let digits = b"0123456789abcdef";
    let mut source_key = String::with_capacity(digest.len() * 2);
    for byte in digest {
        source_key.push(char::from(digits[usize::from(byte >> 4)]));
        source_key.push(char::from(digits[usize::from(byte & 0x0f)]));
    }
    let handoff_path = data_directory
        .join("rebinder/handoffs")
        .join(format!("{source_key}.jsonl"));
    let executable = bin_directory.join("codex");
    let arguments_log = fixture.path().join("resume-arguments.txt");
    let import_marker = fixture.path().join("import-complete");
    let import_log = fixture.path().join("imports.txt");

    let source_json = serde_json::to_string(source_path.to_str().expect("UTF-8 source path"))
        .expect("encode source path");
    let handoff_json = serde_json::to_string(handoff_path.to_str().expect("UTF-8 handoff path"))
        .expect("encode handoff path");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace path"))
        .expect("encode workspace path");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "app-server" ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*) printf '%s\n' '{{"id":0,"result":{{}}}}' ;;
      *'"id":1'*) printf '%s\n' '{{"id":1,"result":{{"items":[{{"itemType":"SESSIONS","description":"Import Claude sessions","cwd":null,"details":{{"plugins":[],"skills":[],"sessions":[{{"path":{source_json},"cwd":{cwd_json},"title":"Large fixture session"}}],"mcpServers":[],"hooks":[],"subagents":[],"commands":[]}}}}]}}}}' ;;
      *'"id":2'*)
        if [ -f "$FAKE_CODEX_IMPORT_MARKER" ]; then
          printf '%s\n' '{{"id":2,"result":{{"data":[{{"providerId":"claude-code","completedAtMs":42,"successes":[{{"itemType":"SESSIONS","cwd":{cwd_json},"source":{handoff_json},"target":"019c0000-0000-7000-8000-000000000002","title":"Large fixture session (Rebinder handoff)"}}]}}]}}}}'
        else
          printf '%s\n' '{{"id":2,"result":{{"data":[]}}}}'
        fi
        ;;
      *'"id":3'*)
        : > "$FAKE_CODEX_IMPORT_MARKER"
        printf '%s\n' imported >> "$FAKE_CODEX_IMPORT_LOG"
        printf '%s\n' '{{"id":3,"result":{{"importId":"handoff-import"}}}}'
        printf '%s\n' '{{"method":"externalAgentConfig/import/completed","params":{{"importId":"handoff-import","itemTypeResults":[{{"itemType":"SESSIONS","successes":[{{"itemType":"SESSIONS","cwd":{cwd_json},"source":{handoff_json},"target":"019c0000-0000-7000-8000-000000000002","title":"Large fixture session (Rebinder handoff)"}}],"failures":[]}}]}}}}'
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = "resume" ]; then
  printf '%s\n' "$@" > "$FAKE_CODEX_ARGUMENTS_LOG"
  exit 0
fi
exit 64
"#
    );
    fs::write(&executable, script).expect("write fake codex");
    let mut permissions = fs::metadata(&executable)
        .expect("read fake codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fake codex executable");

    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "claude",
            "--to",
            "codex",
            session_id,
            "--strategy",
            "handoff",
        ])
        .current_dir(&workspace)
        .env("PATH", &bin_directory)
        .env("XDG_DATA_HOME", &data_directory)
        .env("FAKE_CODEX_ARGUMENTS_LOG", &arguments_log)
        .env("FAKE_CODEX_IMPORT_MARKER", &import_marker)
        .env("FAKE_CODEX_IMPORT_LOG", &import_log)
        .output()
        .expect("run context-safe transfer");

    assert!(
        output.status.success(),
        "handoff failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&arguments_log).expect("read arguments log"),
        "resume\n019c0000-0000-7000-8000-000000000002\n"
    );
    let handoff = fs::read_to_string(&handoff_path).expect("read bounded handoff");
    assert!(handoff.contains("verified compact state"));
    assert!(handoff.contains("recent visible answer"));
    assert!(!handoff.contains("obsolete request"));
    assert!(!handoff.contains("private tool output"));
    assert_eq!(
        fs::metadata(&handoff_path)
            .expect("handoff metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("context-safe handoff"));

    let repeated = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "claude",
            "--to",
            "codex",
            session_id,
            "--strategy",
            "handoff",
        ])
        .current_dir(&workspace)
        .env("PATH", &bin_directory)
        .env("XDG_DATA_HOME", &data_directory)
        .env("FAKE_CODEX_ARGUMENTS_LOG", &arguments_log)
        .env("FAKE_CODEX_IMPORT_MARKER", &import_marker)
        .env("FAKE_CODEX_IMPORT_LOG", &import_log)
        .output()
        .expect("repeat context-safe transfer");
    assert!(
        repeated.status.success(),
        "repeat handoff failed: {}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("reusing"));
    assert_eq!(
        fs::read_to_string(&import_log).expect("read import log"),
        "imported\n"
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read repeated handoff")
            .lines()
            .count(),
        2
    );
}
