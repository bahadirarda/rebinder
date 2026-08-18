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

#[test]
fn capabilities_and_compatibility_are_machine_readable() {
    let capabilities = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["capabilities", "claude", "--json"])
        .output()
        .expect("run capabilities command");
    assert!(capabilities.status.success());
    let capabilities: serde_json::Value =
        serde_json::from_slice(&capabilities.stdout).expect("decode capabilities");
    assert_eq!(capabilities["provider"], "claude");
    assert_eq!(
        capabilities["artifactFormat"],
        "text/markdown; profile=rebinder.continuation.v1"
    );
    assert!(
        capabilities["capabilities"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item["capability"] == "conversation.tool_results"
                        && item["support"] == "omitted"
                })
            })
    );

    let compatibility = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "compatibility",
            "examples/minimal-session",
            "--to",
            "claude",
            "--json",
        ])
        .output()
        .expect("run compatibility command");
    assert!(compatibility.status.success());
    let compatibility: serde_json::Value =
        serde_json::from_slice(&compatibility.stdout).expect("decode compatibility");
    assert_eq!(compatibility["canContinue"], true);
    assert_eq!(compatibility["level"], "compatible_with_loss");
    assert_eq!(compatibility["sourceProvider"], "codex");
    assert_eq!(compatibility["target"]["provider"], "claude");

    let invalid = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "compatibility",
            "missing-session-package",
            "--to",
            "claude",
            "--json",
        ])
        .output()
        .expect("run invalid compatibility command");
    assert_eq!(invalid.status.code(), Some(1));
    let invalid: serde_json::Value =
        serde_json::from_slice(&invalid.stdout).expect("decode invalid compatibility");
    assert_eq!(invalid["canContinue"], false);
    assert_eq!(invalid["level"], "incompatible");
    assert!(invalid["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["severity"] == "blocking")
    }));
}

#[test]
fn artifact_command_writes_once_without_tool_result_payloads() {
    let fixture = tempfile::tempdir().expect("artifact fixture");
    let output = fixture.path().join("continuation.md");
    let first = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "artifact",
            "examples/minimal-session",
            "--to",
            "claude",
            "--output",
            output.to_str().expect("UTF-8 output path"),
            "--json",
        ])
        .output()
        .expect("run artifact command");
    assert!(
        first.status.success(),
        "artifact failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("decode artifact metadata");
    assert_eq!(metadata["targetProvider"], "claude");
    assert_eq!(metadata["compatibility"]["level"], "compatible_with_loss");
    let contents = std::fs::read_to_string(&output).expect("read artifact");
    assert!(contents.contains("Build the provider-neutral validation core."));
    assert!(!contents.contains("Initial Architecture"));

    let repeated = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "artifact",
            "examples/minimal-session",
            "--to",
            "claude",
            "--output",
            output.to_str().expect("UTF-8 output path"),
        ])
        .output()
        .expect("repeat artifact command");
    assert_eq!(repeated.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("already exists"));
}

#[test]
fn claude_session_exports_to_a_valid_canonical_package_without_codex_discovery() {
    use std::fs;

    let fixture = tempfile::tempdir().expect("create export fixture");
    let config = fixture.path().join("claude");
    let project = config.join("projects/project");
    let workspace = fixture.path().join("workspace");
    let output = fixture.path().join("exported-session");
    fs::create_dir_all(&project).expect("create Claude project store");
    fs::create_dir_all(&workspace).expect("create workspace");
    let session_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let transcript = format!(
        "{{\"type\":\"user\",\"sessionId\":\"{session_id}\",\"cwd\":{},\"timestamp\":\"2026-08-18T08:00:00Z\",\"version\":\"2.0.0\",\"message\":{{\"content\":\"Build canonical export\\nAPI_KEY=super-secret\"}}}}\n{{\"type\":\"assistant\",\"sessionId\":\"{session_id}\",\"cwd\":{},\"timestamp\":\"2026-08-18T08:01:00Z\",\"version\":\"2.0.0\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"Export is ready\"}},{{\"type\":\"tool_use\",\"id\":\"call-1\",\"name\":\"Bash\",\"input\":{{\"command\":\"print secret output\"}}}}]}}}}\n",
        serde_json::to_string(workspace.to_str().expect("UTF-8 workspace")).expect("encode cwd"),
        serde_json::to_string(workspace.to_str().expect("UTF-8 workspace")).expect("encode cwd")
    );
    fs::write(project.join(format!("{session_id}.jsonl")), transcript)
        .expect("write Claude transcript");

    let export = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "export",
            "--from",
            "claude",
            session_id,
            "--output",
            output.to_str().expect("UTF-8 output"),
            "--json",
        ])
        .env("CLAUDE_CONFIG_DIR", &config)
        .output()
        .expect("run Claude export");
    assert!(
        export.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&export.stdout).expect("decode export metadata");
    assert_eq!(metadata["sourceProvider"], "claude");
    assert_eq!(metadata["validation"]["valid"], true);

    let conversation =
        fs::read_to_string(output.join("conversation.jsonl")).expect("read exported conversation");
    assert!(conversation.contains("[REDACTED]"));
    assert!(!conversation.contains("super-secret"));
    assert!(!conversation.contains("print secret output"));
    let validate = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .arg("validate")
        .arg(&output)
        .output()
        .expect("validate exported package");
    assert!(validate.status.success());

    let repeated = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "export",
            "--from",
            "claude",
            session_id,
            "--output",
            output.to_str().expect("UTF-8 output"),
        ])
        .env("CLAUDE_CONFIG_DIR", &config)
        .output()
        .expect("repeat Claude export");
    assert_eq!(repeated.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("already exists"));
}

#[cfg(unix)]
#[test]
fn codex_sessions_are_listed_and_exported_through_the_read_only_app_server_api() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create Codex export fixture");
    let bin = fixture.path().join("bin");
    let workspace = fixture.path().join("workspace");
    let output = fixture.path().join("codex-export");
    fs::create_dir_all(&bin).expect("create bin");
    fs::create_dir_all(&workspace).expect("create workspace");
    let executable = bin.join("codex");
    let cwd =
        serde_json::to_string(workspace.to_str().expect("UTF-8 workspace")).expect("encode cwd");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 1.0.0'
  exit 0
fi
if [ "$1" = "app-server" ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*)
        printf '%s\n' '{{"id":0,"result":{{"userAgent":"codex-cli/1.0.0"}}}}'
        ;;
      *'"id":100'*)
        printf '%s\n' '{{"id":100,"result":{{"data":[{{"id":"thread-export-1","name":"Canonical Codex fixture","preview":"Build the exporter","cwd":{cwd},"createdAt":1777685701,"updatedAt":1777685761}}],"nextCursor":null}}}}'
        ;;
      *'"id":500'*)
        printf '%s\n' '{{"id":500,"result":{{"thread":{{"id":"thread-export-1","name":"Canonical Codex fixture","cwd":{cwd},"createdAt":1777685701,"updatedAt":1777685761,"turns":[{{"id":"turn-1","status":"completed","items":[{{"type":"userMessage","id":"user-1","content":[{{"type":"text","text":"Continue Codex export with sk-private-token"}}]}},{{"type":"commandExecution","id":"tool-1","command":"print secret output","cwd":{cwd},"status":"completed","aggregatedOutput":"secret output"}},{{"type":"agentMessage","id":"agent-1","text":"Canonical package ready","phase":"final_answer"}},{{"type":"reasoning","id":"reasoning-1","summary":["private"],"content":["private chain"]}}]}}]}}}}}}'
        ;;
    esac
  done
  exit 0
fi
exit 64
"#
    );
    fs::write(&executable, script).expect("write fake Codex");
    let mut permissions = fs::metadata(&executable)
        .expect("read fake Codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fake Codex executable");

    let sessions = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["sessions", "codex", "--json"])
        .env("PATH", &bin)
        .output()
        .expect("list Codex sessions");
    assert!(
        sessions.status.success(),
        "session listing failed: {}",
        String::from_utf8_lossy(&sessions.stderr)
    );
    let sessions: serde_json::Value =
        serde_json::from_slice(&sessions.stdout).expect("decode sessions");
    assert_eq!(sessions[0]["id"], "thread-export-1");

    let export = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "export",
            "--from",
            "codex",
            "thread-export-1",
            "--output",
            output.to_str().expect("UTF-8 output"),
            "--json",
        ])
        .env("PATH", &bin)
        .output()
        .expect("export Codex session");
    assert!(
        export.status.success(),
        "Codex export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );
    let conversation =
        fs::read_to_string(output.join("conversation.jsonl")).expect("read Codex conversation");
    assert!(conversation.contains("[REDACTED]"));
    assert!(conversation.contains("Canonical package ready"));
    assert!(!conversation.contains("print secret output"));
    assert!(!conversation.contains("private chain"));
    assert!(
        Command::new(env!("CARGO_BIN_EXE_rebinder"))
            .arg("validate")
            .arg(&output)
            .output()
            .expect("validate Codex export")
            .status
            .success()
    );
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
    use std::{fs, io::Write, os::unix::fs::PermissionsExt};

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
    let thread_log = fixture.path().join("thread-requests.txt");
    let injection_log = fixture.path().join("injections.txt");
    let activation_log = fixture.path().join("activations.txt");

    let source_json = serde_json::to_string(source_path.to_str().expect("UTF-8 source path"))
        .expect("encode source path");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace path"))
        .expect("encode workspace path");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "app-server" ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*) printf '%s\n' '{{"id":0,"result":{{}}}}' ;;
      *'"id":1'*) printf '%s\n' '{{"id":1,"result":{{"items":[{{"itemType":"SESSIONS","description":"Import Claude sessions","cwd":null,"details":{{"plugins":[],"skills":[],"sessions":[{{"path":{source_json},"cwd":{cwd_json},"title":"Large fixture session"}}],"mcpServers":[],"hooks":[],"subagents":[],"commands":[]}}}}]}}}}' ;;
      *'"id":2'*) printf '%s\n' '{{"id":2,"result":{{"data":[]}}}}' ;;
      *'"method":"thread/start"'*)
        printf '%s\n' start >> "$FAKE_CODEX_THREAD_LOG"
        printf '%s\n' '{{"id":5,"result":{{"thread":{{"id":"019c0000-0000-7000-8000-000000000002"}}}}}}'
        ;;
      *'"method":"thread/resume"'*)
        printf '%s\n' resume >> "$FAKE_CODEX_THREAD_LOG"
        printf '%s\n' '{{"id":5,"result":{{"thread":{{"id":"019c0000-0000-7000-8000-000000000002"}}}}}}'
        ;;
      *'"method":"thread/inject_items"'*)
        printf '%s\n' inject >> "$FAKE_CODEX_THREAD_LOG"
        printf '%s\n' "$line" >> "$FAKE_CODEX_INJECTION_LOG"
        printf '%s\n' '{{"id":6,"result":{{}}}}'
        ;;
      *'"method":"thread/compact/start"'*)
        printf '%s\n' compact >> "$FAKE_CODEX_THREAD_LOG"
        printf '%s\n' '{{"id":7,"result":{{}}}}'
        if [ -n "$FAKE_CODEX_COMPACT_FAIL_ONCE" ] && [ ! -e "$FAKE_CODEX_COMPACT_FAIL_ONCE" ]; then
          : > "$FAKE_CODEX_COMPACT_FAIL_ONCE"
          printf '%s\n' '{{"method":"turn/completed","params":{{"threadId":"019c0000-0000-7000-8000-000000000002","turn":{{"id":"compact-turn","status":"failed","items":[],"error":{{"message":"fixture compact failure"}}}}}}}}'
        else
          printf '%s\n' '{{"method":"turn/completed","params":{{"threadId":"019c0000-0000-7000-8000-000000000002","turn":{{"id":"compact-turn","status":"completed","items":[],"error":null}}}}}}'
        fi
        ;;
      *'"method":"thread/read"'*)
        if [ -n "$FAKE_CODEX_RECOVER_ACTIVATION" ]; then
          printf '%s\n' recover >> "$FAKE_CODEX_THREAD_LOG"
          printf '{{"id":8,"result":{{"thread":{{"id":"019c0000-0000-7000-8000-000000000002","turns":[{{"status":"completed","items":[{{"type":"userMessage","content":[{{"type":"text","text":"Rebinder handoff revision: %s"}}]}},{{"type":"agentMessage","text":"Recovered continuation brief"}}]}}]}}}}}}\n' "$FAKE_CODEX_RECOVER_ACTIVATION"
        else
          printf '%s\n' '{{"id":8,"result":{{"thread":{{"id":"019c0000-0000-7000-8000-000000000002","turns":[]}}}}}}'
        fi
        ;;
      *'"method":"turn/start"'*)
        printf '%s\n' activate >> "$FAKE_CODEX_THREAD_LOG"
        printf '%s\n' "$line" >> "$FAKE_CODEX_ACTIVATION_LOG"
        printf '%s\n' '{{"id":9,"result":{{"turn":{{"id":"activation-turn","status":"inProgress","items":[]}}}}}}'
        printf '%s\n' '{{"method":"turn/completed","params":{{"threadId":"019c0000-0000-7000-8000-000000000002","turn":{{"id":"activation-turn","status":"completed","items":[{{"type":"agentMessage","id":"activation-message","text":"Visible continuation brief"}}],"error":null}}}}}}'
        ;;
      *'"method":"externalAgentConfig/import"'*)
        printf '%s\n' '{{"id":3,"error":{{"message":"handoffs must not use external import"}}}}'
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("context-safe Codex thread"));
    assert_eq!(
        fs::read_to_string(&thread_log).expect("read thread request log"),
        "start\ninject\nactivate\n"
    );
    let injection = fs::read_to_string(&injection_log).expect("read injection log");
    assert!(injection.contains("verified compact state"));
    assert!(injection.contains("recent visible answer"));
    assert!(injection.contains("\"role\":\"user\""));
    assert!(injection.contains("\"role\":\"assistant\""));
    assert!(injection.contains("\"type\":\"input_text\""));
    assert!(injection.contains("\"type\":\"output_text\""));
    assert!(!injection.contains("obsolete request"));
    assert!(!injection.contains("private tool output"));
    let activation = fs::read_to_string(&activation_log).expect("read activation log");
    assert!(activation.contains("Rebinder continuity activation"));
    assert!(activation.contains("Rebinder handoff revision:"));
    assert!(activation.contains("\"approvalPolicy\":\"never\""));
    assert!(activation.contains("\"sandboxPolicy\":{\"type\":\"readOnly\"}"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("visible continuation brief"));

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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .output()
        .expect("repeat context-safe transfer");
    assert!(
        repeated.status.success(),
        "repeat handoff failed: {}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("reusing"));
    assert_eq!(
        fs::read_to_string(&thread_log).expect("read repeated thread log"),
        "start\ninject\nactivate\n"
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read repeated handoff")
            .lines()
            .count(),
        7
    );

    fs::OpenOptions::new()
        .append(true)
        .open(&source_path)
        .expect("open metadata-only source")
        .write_all(b"{\"type\":\"mode\",\"mode\":\"plan\"}\n")
        .expect("append irrelevant metadata");
    let metadata_only = Command::new(env!("CARGO_BIN_EXE_rebinder"))
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .output()
        .expect("repeat after irrelevant source metadata");
    assert!(
        metadata_only.status.success(),
        "metadata-only handoff failed: {}",
        String::from_utf8_lossy(&metadata_only.stderr)
    );
    assert_eq!(
        fs::read_to_string(&thread_log).expect("read metadata-only thread log"),
        "start\ninject\nactivate\n"
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read metadata-only handoff")
            .lines()
            .count(),
        7
    );

    fs::OpenOptions::new()
        .append(true)
        .open(&source_path)
        .expect("open changed source")
        .write_all(b"{\"type\":\"user\",\"message\":{\"content\":\"new checkpoint detail\"}}\n")
        .expect("append changed source");
    let changed = Command::new(env!("CARGO_BIN_EXE_rebinder"))
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .output()
        .expect("transfer changed context-safe source");
    assert!(
        changed.status.success(),
        "changed handoff failed: {}",
        String::from_utf8_lossy(&changed.stderr)
    );
    assert_eq!(
        fs::read_to_string(&thread_log).expect("read changed thread log"),
        "start\ninject\nactivate\nresume\ninject\ncompact\nactivate\n"
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read changed handoff")
            .lines()
            .count(),
        13
    );
    assert!(
        String::from_utf8_lossy(&changed.stderr)
            .contains("compacted the updated handoff before opening Codex")
    );

    fs::OpenOptions::new()
        .append(true)
        .open(&source_path)
        .expect("open source for retry fixture")
        .write_all(b"{\"type\":\"assistant\",\"message\":{\"content\":\"retry-safe detail\"}}\n")
        .expect("append retry-safe detail");
    let compact_marker = fixture.path().join("compact-failed-once");
    let failed = Command::new(env!("CARGO_BIN_EXE_rebinder"))
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .env("FAKE_CODEX_COMPACT_FAIL_ONCE", &compact_marker)
        .output()
        .expect("run failed compact fixture");
    assert_eq!(failed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("fixture compact failure"));
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read handoff after failed compact")
            .lines()
            .count(),
        16
    );

    let retried = Command::new(env!("CARGO_BIN_EXE_rebinder"))
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .env("FAKE_CODEX_COMPACT_FAIL_ONCE", &compact_marker)
        .output()
        .expect("retry compact fixture");
    assert!(
        retried.status.success(),
        "compact retry failed: {}",
        String::from_utf8_lossy(&retried.stderr)
    );
    assert_eq!(
        fs::read_to_string(&thread_log).expect("read retry-safe thread log"),
        "start\ninject\nactivate\nresume\ninject\ncompact\nactivate\nresume\ninject\ncompact\nresume\ncompact\nactivate\n"
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read handoff after compact retry")
            .lines()
            .count(),
        19
    );

    let handoff = fs::read_to_string(&handoff_path).expect("read activation recovery handoff");
    let recovery_hash = handoff
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|record| {
            (record["type"] == "user")
                .then(|| record["rebinderSourceSha256"].as_str().map(str::to_owned))
                .flatten()
        })
        .expect("latest handoff source hash");
    let mut handoff_file = fs::OpenOptions::new()
        .append(true)
        .open(&handoff_path)
        .expect("open handoff for activation recovery fixture");
    writeln!(
        handoff_file,
        "{}",
        serde_json::json!({
            "type": "rebinder-binding",
            "rebinderSourceSha256": recovery_hash,
            "rebinderFormatVersion": 2,
            "codexThreadId": "019c0000-0000-7000-8000-000000000002",
            "status": "activating",
            "requiresCompaction": false,
            "rebinderActivationVersion": 0
        })
    )
    .expect("append activation recovery fixture");
    let recovered = Command::new(env!("CARGO_BIN_EXE_rebinder"))
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
        .env("FAKE_CODEX_THREAD_LOG", &thread_log)
        .env("FAKE_CODEX_INJECTION_LOG", &injection_log)
        .env("FAKE_CODEX_ACTIVATION_LOG", &activation_log)
        .env("FAKE_CODEX_RECOVER_ACTIVATION", &recovery_hash)
        .output()
        .expect("recover completed activation");
    assert!(
        recovered.status.success(),
        "activation recovery failed: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert!(!String::from_utf8_lossy(&recovered.stderr).contains("visible continuation brief"));
    assert_eq!(
        fs::read_to_string(&activation_log)
            .expect("read recovered activation log")
            .lines()
            .count(),
        3
    );
    assert!(
        fs::read_to_string(&thread_log)
            .expect("read recovered thread log")
            .ends_with("resume\nrecover\n")
    );
    assert_eq!(
        fs::read_to_string(&handoff_path)
            .expect("read recovered handoff")
            .lines()
            .count(),
        21
    );
}
