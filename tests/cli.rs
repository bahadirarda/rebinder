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
fn worktree_repository_hint_requires_explicit_recovery() {
    let output = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args([
            "transfer",
            "--from",
            "codex",
            "--to",
            "claude",
            "thread-1",
            "--worktree-repository",
            "/tmp/repository",
        ])
        .output()
        .expect("parse recovery options");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--recover-worktree"));
    assert!(stderr.contains("required"));
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

#[cfg(unix)]
#[test]
fn claude_continuity_plugin_offers_one_consent_gated_weekly_handoff() {
    use std::{
        fs,
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };

    let fixture = tempfile::tempdir().expect("create continuity fixture");
    let claude_config = fixture.path().join("claude");
    let data = fixture.path().join("data");
    let workspace = fixture.path().join("workspace");
    let bin = fixture.path().join("bin");
    fs::create_dir_all(&claude_config).expect("create Claude config");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&bin).expect("create fake bin");
    fs::write(
        claude_config.join("settings.json"),
        r#"{"statusLine":{"type":"command","command":"printf 'existing status'","padding":2},"theme":"dark"}"#,
    )
    .expect("write existing Claude settings");
    let codex = bin.join("codex");
    fs::write(
        &codex,
        "#!/bin/sh\n[ \"$1\" = login ] && [ \"$2\" = status ] && exit 0\nexit 64\n",
    )
    .expect("write fake Codex");
    let mut permissions = fs::metadata(&codex)
        .expect("read fake Codex metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&codex, permissions).expect("make fake Codex executable");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let enabled = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "enable", "claude", "--to", "codex"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("enable Claude continuity");
    assert!(
        enabled.status.success(),
        "enable failed: {}",
        String::from_utf8_lossy(&enabled.stderr)
    );
    let settings: serde_json::Value = serde_json::from_slice(
        &fs::read(claude_config.join("settings.json")).expect("read wrapped settings"),
    )
    .expect("decode wrapped settings");
    assert_eq!(
        settings["statusLine"]["command"],
        "rebinder continuity observe"
    );
    assert_eq!(settings["statusLine"]["padding"], 2);
    assert_eq!(settings["theme"], "dark");
    let plugin = claude_config.join("skills/rebinder-continuity");
    assert!(plugin.join("hooks/hooks.json").is_file());
    assert!(plugin.join("skills/handoff/SKILL.md").is_file());
    let hooks: serde_json::Value = serde_json::from_slice(
        &fs::read(plugin.join("hooks/hooks.json")).expect("read plugin hooks"),
    )
    .expect("decode plugin hooks");
    assert_eq!(hooks["hooks"]["StopFailure"][0]["matcher"], "rate_limit");
    assert_eq!(
        hooks["hooks"]["StopFailure"][0]["hooks"][0]["args"],
        serde_json::json!(["continuity", "failure"])
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(plugin.join(".claude-plugin/plugin.json")).expect("read plugin manifest"),
    )
    .expect("decode plugin manifest");
    assert_eq!(manifest["name"], "rebinder-continuity");
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));

    let statusline_input = serde_json::json!({
        "session_id": "claude-session-1",
        "cwd": workspace,
        "transcript_path": fixture.path().join("claude-session-1.jsonl"),
        "rate_limits": {
            "five_hour": { "used_percentage": 40.0, "resets_at": u64::MAX - 1 },
            "seven_day": { "used_percentage": 86.0, "resets_at": u64::MAX }
        }
    });
    let mut observer = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "observe"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_LAUNCH_ID", "launch-1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start continuity observer");
    observer
        .stdin
        .take()
        .expect("observer stdin")
        .write_all(statusline_input.to_string().as_bytes())
        .expect("write status-line input");
    let observer_output = observer.wait_with_output().expect("wait for observer");
    assert!(
        observer_output.status.success(),
        "observer failed: {}",
        String::from_utf8_lossy(&observer_output.stderr)
    );
    let rendered = String::from_utf8_lossy(&observer_output.stdout);
    assert!(
        rendered.contains("existing status"),
        "unexpected status line: {rendered}; stderr: {}",
        String::from_utf8_lossy(&observer_output.stderr)
    );
    assert!(
        rendered.contains("7-day usage 86%"),
        "unexpected status line: {rendered}"
    );

    let status = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read continuity status");
    assert!(status.status.success());
    let status: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("decode continuity status");
    assert_eq!(status["enabled"], true);
    assert_eq!(
        status["latestObservation"]["sevenDay"]["usedPercentage"],
        86.0
    );
    assert_eq!(status["offers"].as_array().map(Vec::len), Some(1));
    assert_eq!(status["offers"][0]["state"], "ready");
    let offer_id = status["offers"][0]["id"]
        .as_str()
        .expect("offer ID")
        .to_owned();

    let hook_input = serde_json::json!({
        "session_id": "claude-session-1",
        "cwd": workspace,
        "hook_event_name": "UserPromptSubmit"
    });
    let mut hook = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "hook"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_LAUNCH_ID", "launch-1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start continuity hook");
    hook.stdin
        .take()
        .expect("hook stdin")
        .write_all(hook_input.to_string().as_bytes())
        .expect("write hook input");
    let hook = hook.wait_with_output().expect("wait for hook");
    assert!(hook.status.success());
    let hook: serde_json::Value = serde_json::from_slice(&hook.stdout).expect("decode hook output");
    assert_eq!(
        hook["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    let context = hook["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("hook context");
    assert!(context.contains("User authorization is required"));
    assert!(context.contains(&offer_id));

    let repeated_hook = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "hook"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .expect("repeated hook stdin")
                .write_all(hook_input.to_string().as_bytes())?;
            child.wait_with_output()
        })
        .expect("repeat continuity hook");
    assert!(repeated_hook.status.success());
    assert!(repeated_hook.stdout.is_empty());

    let accepted = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "accept", "--offer", &offer_id])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("accept continuity offer");
    assert!(accepted.status.success());
    assert!(String::from_utf8_lossy(&accepted.stdout).contains("enter `/exit`"));

    let settings_path = claude_config.join("settings.json");
    let mut changed_settings: serde_json::Value = serde_json::from_slice(
        &fs::read(&settings_path).expect("read settings before ownership test"),
    )
    .expect("decode settings before ownership test");
    changed_settings["statusLine"]["padding"] = serde_json::json!(3);
    fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&changed_settings).expect("encode changed settings"),
    )
    .expect("change status-line ownership");
    let refused_disable = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "disable", "claude"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("refuse changed Claude status line");
    assert_eq!(refused_disable.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused_disable.stderr).contains("statusLine changed"));
    assert!(plugin.exists());
    changed_settings["statusLine"]["padding"] = serde_json::json!(2);
    fs::write(
        &settings_path,
        serde_json::to_vec_pretty(&changed_settings).expect("encode restored wrapper"),
    )
    .expect("restore managed wrapper");

    let disabled = Command::new(env!("CARGO_BIN_EXE_rebinder"))
        .args(["continuity", "disable", "claude"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("disable Claude continuity");
    assert!(
        disabled.status.success(),
        "disable failed: {}",
        String::from_utf8_lossy(&disabled.stderr)
    );
    let restored: serde_json::Value = serde_json::from_slice(
        &fs::read(claude_config.join("settings.json")).expect("read restored settings"),
    )
    .expect("decode restored settings");
    assert_eq!(
        restored["statusLine"]["command"],
        "printf 'existing status'"
    );
    assert_eq!(restored["statusLine"]["padding"], 2);
    assert_eq!(restored["theme"], "dark");
    assert!(!plugin.exists());
}

#[cfg(unix)]
#[test]
fn hard_limit_rescue_respects_a_decline_from_an_earlier_launch() {
    use std::{fs, io::Write, os::unix::fs::PermissionsExt, process::Stdio};

    let fixture = tempfile::tempdir().expect("create declined rescue fixture");
    let claude_config = fixture.path().join("claude");
    let data = fixture.path().join("data");
    let workspace = fixture.path().join("workspace");
    let bin = fixture.path().join("bin");
    let transcript = fixture.path().join("declined-session.jsonl");
    let failure_output = fixture.path().join("failure-output.json");
    fs::create_dir_all(&claude_config).expect("create Claude config");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&bin).expect("create fake bin");
    fs::write(&transcript, "declined rescue fixture\n").expect("write transcript");
    let codex = bin.join("codex");
    fs::write(
        &codex,
        "#!/bin/sh\n[ \"$1\" = login ] && [ \"$2\" = status ] && exit 0\nexit 64\n",
    )
    .expect("write fake Codex");
    let session_id = "55555555-4444-3333-2222-111111111111";
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
        .expect("encode workspace");
    let transcript_json = serde_json::to_string(transcript.to_str().expect("UTF-8 transcript"))
        .expect("encode transcript");
    let claude = bin.join("claude");
    fs::write(
        &claude,
        format!(
            r#"#!/bin/sh
failure='{{"session_id":"{session_id}","cwd":{cwd_json},"transcript_path":{transcript_json},"hook_event_name":"StopFailure","error":"rate_limit"}}'
printf '%s' "$failure" | "$REBINDER_TEST_BIN" continuity failure > "$FAKE_FAILURE_OUTPUT" || exit 71
exit 0
"#
        ),
    )
    .expect("write fake Claude");
    for executable in [&codex, &claude] {
        let mut permissions = fs::metadata(executable)
            .expect("read fake executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("make fake executable runnable");
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let rebinder = env!("CARGO_BIN_EXE_rebinder");
    let enabled = Command::new(rebinder)
        .args(["continuity", "enable", "claude", "--to", "codex"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("enable declined rescue fixture");
    assert!(enabled.status.success());

    let statusline_input = serde_json::json!({
        "session_id": session_id,
        "cwd": workspace,
        "transcript_path": transcript,
        "rate_limits": {
            "seven_day": { "used_percentage": 90.0, "resets_at": u64::MAX }
        }
    });
    let mut observer = Command::new(rebinder)
        .args(["continuity", "observe"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_LAUNCH_ID", "earlier-launch")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("start declined-offer observer");
    observer
        .stdin
        .take()
        .expect("observer stdin")
        .write_all(statusline_input.to_string().as_bytes())
        .expect("write declined-offer status line");
    assert!(observer.wait().expect("wait for observer").success());
    let status = Command::new(rebinder)
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read proactive offer");
    let status: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("decode proactive offer");
    let offer_id = status["offers"][0]["id"]
        .as_str()
        .expect("offer ID")
        .to_owned();
    let declined = Command::new(rebinder)
        .args(["continuity", "decline", "--offer", &offer_id])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("decline proactive offer");
    assert!(declined.status.success());

    let stopped = Command::new(rebinder)
        .arg("claude")
        .current_dir(&workspace)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_TEST_BIN", rebinder)
        .env("FAKE_FAILURE_OUTPUT", &failure_output)
        .output()
        .expect("run declined hard-limit failure");
    assert!(stopped.status.success());
    assert!(!String::from_utf8_lossy(&stopped.stderr).contains("rescue is ready"));
    assert_eq!(
        fs::read_to_string(&failure_output).expect("read suppressed notification"),
        ""
    );
    let final_status = Command::new(rebinder)
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read declined rescue status");
    let final_status: serde_json::Value =
        serde_json::from_slice(&final_status.stdout).expect("decode declined rescue status");
    assert_eq!(final_status["offers"].as_array().map(Vec::len), Some(1));
    assert_eq!(final_status["offers"][0]["state"], "declined");
    assert_eq!(final_status["offers"][0]["rescueReady"], false);
}

#[cfg(unix)]
#[test]
fn rebinder_claude_opens_codex_after_an_accepted_continuity_exit() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create automatic continuity fixture");
    let bin = fixture.path().join("bin");
    let claude_config = fixture.path().join("claude");
    let data = fixture.path().join("data");
    let workspace = fixture.path().join("workspace");
    let sessions = fixture.path().join("sessions");
    fs::create_dir_all(&bin).expect("create fake bin");
    fs::create_dir_all(&claude_config).expect("create Claude config");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&sessions).expect("create session store");
    let session_id = "99999999-8888-7777-6666-555555555555";
    let source = sessions.join(format!("{session_id}.jsonl"));
    fs::write(
        &source,
        format!(
            "{{\"type\":\"user\",\"sessionId\":\"{session_id}\",\"cwd\":{},\"message\":{{\"content\":\"Continue automatically\"}}}}\n",
            serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
                .expect("encode workspace")
        ),
    )
    .expect("write Claude source session");
    let source_json =
        serde_json::to_string(source.to_str().expect("UTF-8 source")).expect("encode source path");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
        .expect("encode workspace path");
    let codex_log = fixture.path().join("codex-resume.txt");

    let codex = bin.join("codex");
    let codex_script = format!(
        r#"#!/bin/sh
if [ "$1" = login ] && [ "$2" = status ]; then exit 0; fi
if [ "$1" = app-server ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*) printf '%s\n' '{{"id":0,"result":{{}}}}' ;;
      *'"id":1'*) printf '%s\n' '{{"id":1,"result":{{"items":[{{"itemType":"SESSIONS","description":"Import Claude sessions","cwd":null,"details":{{"plugins":[],"skills":[],"sessions":[{{"path":{source_json},"cwd":{cwd_json},"title":"Automatic continuity fixture"}}],"mcpServers":[],"hooks":[],"subagents":[],"commands":[]}}}}]}}}}' ;;
      *'"id":2'*) printf '%s\n' '{{"id":2,"result":{{"data":[]}}}}' ;;
      *'"id":3'*)
        printf '%s\n' '{{"id":3,"result":{{"importId":"automatic-import"}}}}'
        printf '%s\n' '{{"method":"externalAgentConfig/import/completed","params":{{"importId":"automatic-import","itemTypeResults":[{{"itemType":"SESSIONS","successes":[{{"itemType":"SESSIONS","cwd":{cwd_json},"source":{source_json},"target":"019c1111-0000-7000-8000-000000000001","title":"Automatic continuity fixture"}}],"failures":[]}}]}}}}'
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = resume ]; then
  printf '%s\n' "$@" > "$FAKE_CODEX_LOG"
  exit 0
fi
exit 64
"#
    );
    fs::write(&codex, codex_script).expect("write fake Codex");

    let claude = bin.join("claude");
    let claude_script = format!(
        r#"#!/bin/sh
test -n "$REBINDER_LAUNCH_ID" || exit 70
printf '%s' '{{"session_id":"{session_id}","cwd":{cwd_json},"transcript_path":{source_json},"rate_limits":{{"five_hour":{{"used_percentage":30,"resets_at":18446744073709551614}},"seven_day":{{"used_percentage":90,"resets_at":18446744073709551615}}}}}}' | "$REBINDER_TEST_BIN" continuity observe >/dev/null || exit 71
"$REBINDER_TEST_BIN" continuity accept >/dev/null || exit 72
exit 0
"#
    );
    fs::write(&claude, claude_script).expect("write fake Claude");
    for executable in [&codex, &claude] {
        let mut permissions = fs::metadata(executable)
            .expect("read fake executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("make fake executable runnable");
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let rebinder = env!("CARGO_BIN_EXE_rebinder");
    let enabled = Command::new(rebinder)
        .args(["continuity", "enable", "claude", "--to", "codex"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("enable automatic continuity");
    assert!(
        enabled.status.success(),
        "enable failed: {}",
        String::from_utf8_lossy(&enabled.stderr)
    );

    let switched = Command::new(rebinder)
        .arg("claude")
        .current_dir(&workspace)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_TEST_BIN", rebinder)
        .env("FAKE_CODEX_LOG", &codex_log)
        .output()
        .expect("run Rebinder-owned Claude session");
    assert!(
        switched.status.success(),
        "automatic handoff failed: {}",
        String::from_utf8_lossy(&switched.stderr)
    );
    assert!(
        String::from_utf8_lossy(&switched.stderr).contains("accepted continuity handoff detected")
    );
    assert_eq!(
        fs::read_to_string(&codex_log).expect("read Codex resume log"),
        "resume\n019c1111-0000-7000-8000-000000000001\n"
    );
    let status = Command::new(rebinder)
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read completed continuity status");
    let status: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("decode continuity status");
    assert_eq!(status["offers"][0]["state"], "completed");
}

#[cfg(unix)]
#[test]
fn claude_rate_limit_failure_requires_consent_then_rescues_into_codex() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create hard-limit rescue fixture");
    let bin = fixture.path().join("bin");
    let claude_config = fixture.path().join("claude");
    let data = fixture.path().join("data");
    let workspace = fixture.path().join("workspace");
    let sessions = fixture.path().join("sessions");
    fs::create_dir_all(&bin).expect("create fake bin");
    fs::create_dir_all(&claude_config).expect("create Claude config");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&sessions).expect("create session store");
    let session_id = "77777777-6666-5555-4444-333333333333";
    let source = sessions.join(format!("{session_id}.jsonl"));
    fs::write(
        &source,
        format!(
            "{{\"type\":\"user\",\"sessionId\":\"{session_id}\",\"cwd\":{},\"message\":{{\"content\":\"Continue after the provider limit\"}}}}\n",
            serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
                .expect("encode workspace")
        ),
    )
    .expect("write Claude rescue source");
    let source_json =
        serde_json::to_string(source.to_str().expect("UTF-8 source")).expect("encode source path");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
        .expect("encode workspace path");
    let codex_log = fixture.path().join("codex-rescue.txt");
    let failure_output = fixture.path().join("failure-output.jsonl");

    let codex = bin.join("codex");
    let codex_script = format!(
        r#"#!/bin/sh
if [ "$1" = login ] && [ "$2" = status ]; then exit 0; fi
if [ "$1" = app-server ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*) printf '%s\n' '{{"id":0,"result":{{}}}}' ;;
      *'"id":1'*) printf '%s\n' '{{"id":1,"result":{{"items":[{{"itemType":"SESSIONS","description":"Import Claude sessions","cwd":null,"details":{{"plugins":[],"skills":[],"sessions":[{{"path":{source_json},"cwd":{cwd_json},"title":"Hard-limit rescue fixture"}}],"mcpServers":[],"hooks":[],"subagents":[],"commands":[]}}}}]}}}}' ;;
      *'"id":2'*) printf '%s\n' '{{"id":2,"result":{{"data":[]}}}}' ;;
      *'"id":3'*)
        printf '%s\n' '{{"id":3,"result":{{"importId":"rescue-import"}}}}'
        printf '%s\n' '{{"method":"externalAgentConfig/import/completed","params":{{"importId":"rescue-import","itemTypeResults":[{{"itemType":"SESSIONS","successes":[{{"itemType":"SESSIONS","cwd":{cwd_json},"source":{source_json},"target":"019c2222-0000-7000-8000-000000000002","title":"Hard-limit rescue fixture"}}],"failures":[]}}]}}}}'
        ;;
    esac
  done
  exit 0
fi
if [ "$1" = resume ]; then
  printf '%s\n' "$@" > "$FAKE_CODEX_LOG"
  exit 0
fi
exit 64
"#
    );
    fs::write(&codex, codex_script).expect("write fake Codex");

    let claude = bin.join("claude");
    let claude_script = format!(
        r#"#!/bin/sh
test -n "$REBINDER_LAUNCH_ID" || exit 70
failure='{{"session_id":"{session_id}","cwd":{cwd_json},"transcript_path":{source_json},"hook_event_name":"StopFailure","error":"rate_limit","error_details":"429 Too Many Requests","last_assistant_message":"API Error: Rate limit reached"}}'
printf '%s' "$failure" | "$REBINDER_TEST_BIN" continuity failure > "$FAKE_FAILURE_OUTPUT" || exit 71
printf '%s' "$failure" | "$REBINDER_TEST_BIN" continuity failure >> "$FAKE_FAILURE_OUTPUT" || exit 72
exit 0
"#
    );
    fs::write(&claude, claude_script).expect("write fake Claude");
    for executable in [&codex, &claude] {
        let mut permissions = fs::metadata(executable)
            .expect("read fake executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("make fake executable runnable");
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let rebinder = env!("CARGO_BIN_EXE_rebinder");
    let enabled = Command::new(rebinder)
        .args(["continuity", "enable", "claude", "--to", "codex"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("enable hard-limit rescue");
    assert!(enabled.status.success());

    let stopped = Command::new(rebinder)
        .arg("claude")
        .current_dir(&workspace)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_TEST_BIN", rebinder)
        .env("FAKE_FAILURE_OUTPUT", &failure_output)
        .env("FAKE_CODEX_LOG", &codex_log)
        .output()
        .expect("run rate-limited Claude fixture");
    assert!(stopped.status.success());
    assert!(String::from_utf8_lossy(&stopped.stderr).contains("Claude rate-limit rescue is ready"));
    assert!(!codex_log.exists(), "target started without consent");
    let notification_lines = fs::read_to_string(&failure_output)
        .expect("read failure hook output")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(notification_lines.len(), 1, "rescue notification repeated");
    let notification: serde_json::Value =
        serde_json::from_str(&notification_lines[0]).expect("decode terminal notification");
    assert!(
        notification["terminalSequence"]
            .as_str()
            .is_some_and(|sequence| sequence.contains("Rebinder recorded"))
    );

    let status = Command::new(rebinder)
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read rescue status");
    let status: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("decode rescue status");
    assert_eq!(status["offers"].as_array().map(Vec::len), Some(1));
    assert_eq!(status["offers"][0]["reason"], "rate_limit_failure");
    assert_eq!(status["offers"][0]["state"], "ready");
    assert_eq!(status["offers"][0]["rescueReady"], true);
    let offer_id = status["offers"][0]["id"]
        .as_str()
        .expect("rescue offer ID")
        .to_owned();

    let nested = Command::new(rebinder)
        .args(["continuity", "rescue", "--offer", &offer_id, "--yes"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("REBINDER_LAUNCH_ID", "still-inside-claude")
        .env("FAKE_CODEX_LOG", &codex_log)
        .output()
        .expect("refuse nested rescue");
    assert_eq!(nested.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&nested.stderr).contains("exit Claude Code"));
    assert!(!codex_log.exists(), "target nested inside Claude");

    let refused = Command::new(rebinder)
        .args(["continuity", "rescue", "--offer", &offer_id])
        .current_dir(&workspace)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("FAKE_CODEX_LOG", &codex_log)
        .output()
        .expect("refuse non-interactive rescue without consent");
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("explicit consent"));
    assert!(!codex_log.exists(), "target started after refused rescue");

    let rescued = Command::new(rebinder)
        .args(["continuity", "rescue", "--offer", &offer_id, "--yes"])
        .current_dir(&workspace)
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .env("FAKE_CODEX_LOG", &codex_log)
        .output()
        .expect("accept hard-limit rescue");
    assert!(
        rescued.status.success(),
        "rescue failed: {}",
        String::from_utf8_lossy(&rescued.stderr)
    );
    assert_eq!(
        fs::read_to_string(&codex_log).expect("read rescued Codex resume log"),
        "resume\n019c2222-0000-7000-8000-000000000002\n"
    );
    let completed = Command::new(rebinder)
        .args(["continuity", "status", "--json"])
        .env("CLAUDE_CONFIG_DIR", &claude_config)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &path)
        .output()
        .expect("read completed rescue status");
    let completed: serde_json::Value =
        serde_json::from_slice(&completed.stdout).expect("decode completed rescue status");
    assert_eq!(completed["offers"][0]["state"], "completed");
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

#[cfg(unix)]
#[test]
fn codex_to_claude_creates_then_idempotently_resumes_a_native_session() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create reverse transfer fixture");
    let bin = fixture.path().join("bin");
    let repository = fixture.path().join("repository");
    let worktree_parent = fixture.path().join("worktrees");
    let workspace = worktree_parent.join("reverse-session");
    let claude_config = fixture.path().join("claude");
    let arguments_log = fixture.path().join("claude-arguments.txt");
    let cwd_log = fixture.path().join("claude-cwd.txt");
    fs::create_dir_all(&bin).expect("create bin");
    fs::create_dir_all(&repository).expect("create repository");
    fs::create_dir_all(&worktree_parent).expect("create worktree parent");
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(arguments)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "Rebinder Tests"]);
    git(&["config", "user.email", "rebinder-tests@example.invalid"]);
    fs::write(repository.join("tracked.txt"), "committed worktree state\n")
        .expect("write tracked fixture");
    git(&["add", "tracked.txt"]);
    git(&["commit", "-m", "fixture"]);
    git(&["branch", "reverse-session"]);
    let workspace_text = workspace.to_string_lossy().into_owned();
    git(&["worktree", "add", &workspace_text, "reverse-session"]);
    fs::remove_dir_all(&workspace).expect("simulate missing registered worktree");
    let cwd_json = serde_json::to_string(workspace.to_str().expect("UTF-8 workspace"))
        .expect("encode workspace");

    let codex = bin.join("codex");
    let codex_script = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 1.0.0'
  exit 0
fi
if [ "$1" = "app-server" ]; then
  while IFS= read -r line; do
    case "$line" in
      *'"id":0'*) printf '%s\n' '{{"id":0,"result":{{}}}}' ;;
      *'"id":100'*) printf '%s\n' '{{"id":100,"result":{{"data":[{{"id":"codex-source-1","name":"Reverse transfer fixture","cwd":{cwd_json},"createdAt":1777685701,"updatedAt":1777685761}}],"nextCursor":null}}}}' ;;
      *'"id":500'*) printf '%s\n' '{{"id":500,"result":{{"thread":{{"id":"codex-source-1","name":"Reverse transfer fixture","cwd":{cwd_json},"createdAt":1777685701,"updatedAt":1777685761,"turns":[{{"id":"turn-1","status":"completed","items":[{{"type":"userMessage","id":"user-1","content":[{{"type":"text","text":"Continue the reverse adapter"}}]}},{{"type":"commandExecution","id":"tool-1","command":"private command","cwd":{cwd_json},"status":"completed","aggregatedOutput":"private output"}},{{"type":"agentMessage","id":"agent-1","text":"The source checkpoint is ready","phase":"final_answer"}}]}}]}}}}}}' ;;
    esac
  done
  exit 0
fi
exit 64
"#
    );
    fs::write(&codex, codex_script).expect("write fake Codex");

    let claude = bin.join("claude");
    let claude_script = format!(
        r#"#!/bin/sh
printf '%s\n' '---' >> "$FAKE_CLAUDE_ARGUMENTS_LOG"
printf '%s\n' "$@" >> "$FAKE_CLAUDE_ARGUMENTS_LOG"
printf '%s\n' "$PWD" >> "$FAKE_CLAUDE_CWD_LOG"
previous=''
target_id=''
artifact=''
prompt=''
for argument in "$@"; do
  if [ "$previous" = '--session-id' ] || [ "$previous" = '--resume' ]; then target_id="$argument"; fi
  if [ "$previous" = '--append-system-prompt-file' ]; then artifact="$argument"; fi
  previous="$argument"
  prompt="$argument"
done
if [ -n "$artifact" ]; then
  test "$(stat -c '%a' "$artifact")" = '600' || exit 65
  grep -q '# Rebinder Continuation Artifact' "$artifact" || exit 66
  if grep -q 'private output' "$artifact"; then exit 67; fi
fi
if printf '%s\n' "$@" | grep -q -- '--session-id'; then
  revision="$(printf '%s\n' "$prompt" | sed -n 's/.*Rebinder revision: `\([^`]*\)`.*/\1/p')"
  test -n "$revision" || exit 68
  mkdir -p "$CLAUDE_CONFIG_DIR/projects/project"
  printf '%s\n' '{{"type":"user","sessionId":"'"$target_id"'","cwd":{cwd_json},"timestamp":"2026-08-18T09:00:00Z","message":{{"content":"Rebinder revision: `'"$revision"'`"}}}}' > "$CLAUDE_CONFIG_DIR/projects/project/$target_id.jsonl"
  printf '%s\n' '{{"type":"assistant","sessionId":"'"$target_id"'","cwd":{cwd_json},"timestamp":"2026-08-18T09:00:01Z","message":{{"content":"Ready to continue"}}}}' >> "$CLAUDE_CONFIG_DIR/projects/project/$target_id.jsonl"
fi
exit 0
"#
    );
    fs::write(&claude, claude_script).expect("write fake Claude");
    for executable in [&codex, &claude] {
        let mut permissions = fs::metadata(executable)
            .expect("read fake executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("make fake executable runnable");
    }

    let run = || {
        Command::new(env!("CARGO_BIN_EXE_rebinder"))
            .args([
                "transfer",
                "--from",
                "codex",
                "--to",
                "claude",
                "codex-source-1",
                "--recover-worktree",
                "--worktree-repository",
                repository.to_str().expect("UTF-8 repository"),
                "--",
                "--model",
                "fixture-model",
            ])
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("CLAUDE_CONFIG_DIR", &claude_config)
            .env("FAKE_CLAUDE_ARGUMENTS_LOG", &arguments_log)
            .env("FAKE_CLAUDE_CWD_LOG", &cwd_log)
            .output()
            .expect("run reverse transfer")
    };
    let first = run();
    assert!(
        first.status.success(),
        "first reverse transfer failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stderr).contains("as new Claude session"));
    assert!(String::from_utf8_lossy(&first.stderr).contains("recreated registered worktree"));
    assert_eq!(
        fs::read_to_string(workspace.join("tracked.txt")).expect("read recovered worktree"),
        "committed worktree state\n"
    );
    let second = run();
    assert!(
        second.status.success(),
        "repeat reverse transfer failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("already active"));

    let arguments = fs::read_to_string(&arguments_log).expect("read Claude arguments");
    assert_eq!(arguments.matches("--session-id").count(), 1);
    assert_eq!(arguments.matches("--resume").count(), 1);
    assert_eq!(arguments.matches("--append-system-prompt-file").count(), 1);
    assert_eq!(arguments.matches("Rebinder revision:").count(), 1);
    assert_eq!(arguments.matches("--model").count(), 2);
    let workspaces = fs::read_to_string(&cwd_log).expect("read Claude cwd");
    assert!(
        workspaces
            .lines()
            .all(|line| line == workspace.display().to_string())
    );
}

#[cfg(unix)]
#[test]
fn claude_to_codex_imports_and_resumes_in_the_recorded_workspace() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let fixture = tempfile::tempdir().expect("create transfer fixture");
    let bin_directory = fixture.path().join("bin");
    let repository = fixture.path().join("repository");
    let worktree_parent = fixture.path().join("worktrees");
    let workspace = worktree_parent.join("claude-session");
    let session_directory = fixture.path().join("claude-sessions");
    fs::create_dir_all(&bin_directory).expect("create fake bin directory");
    fs::create_dir_all(&repository).expect("create repository");
    fs::create_dir_all(&worktree_parent).expect("create worktree parent");
    fs::create_dir_all(&session_directory).expect("create session directory");

    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(arguments)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.name", "Rebinder Tests"]);
    git(&["config", "user.email", "rebinder-tests@example.invalid"]);
    fs::write(
        repository.join("tracked.txt"),
        "committed Claude workspace\n",
    )
    .expect("write tracked fixture");
    git(&["add", "tracked.txt"]);
    git(&["commit", "-m", "fixture"]);
    git(&["branch", "claude-session"]);
    let workspace_text = workspace.to_string_lossy().into_owned();
    git(&["worktree", "add", &workspace_text, "claude-session"]);
    fs::remove_dir_all(&workspace).expect("simulate missing registered worktree");

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
            "--recover-worktree",
            "--worktree-repository",
            repository.to_str().expect("UTF-8 repository"),
            "--",
            "--model",
            "gpt-fixture",
        ])
        .current_dir(&repository)
        .env("PATH", format!("{}:/usr/bin:/bin", bin_directory.display()))
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
    assert!(stderr.contains("recreated registered worktree"));
    assert_eq!(
        fs::read_to_string(workspace.join("tracked.txt")).expect("read recovered worktree"),
        "committed Claude workspace\n"
    );
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
        printf '%s\n' '{{"method":"item/completed","params":{{"threadId":"019c0000-0000-7000-8000-000000000002","turnId":"activation-turn","item":{{"type":"agentMessage","id":"activation-message","text":"Visible continuation brief","phase":"final_answer"}}}}}}'
        printf '%s\n' '{{"method":"turn/completed","params":{{"threadId":"019c0000-0000-7000-8000-000000000002","turn":{{"id":"activation-turn","status":"completed","items":[],"error":null}}}}}}'
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
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("[transfer] connecting to the Codex app-server")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("[transfer] asking Codex for the visible continuation brief")
    );
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
