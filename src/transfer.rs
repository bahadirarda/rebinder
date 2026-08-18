use std::{
    cmp::Reverse,
    collections::{BTreeMap, VecDeque},
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::{Duration, Instant, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::handoff::{
    FULL_IMPORT_MAX_SOURCE_BYTES, HandoffMessage, HandoffMessageRole, completed_handoff_thread,
    prepare_context_safe_handoff, recommended_handoff, record_activating_handoff_binding,
    record_completed_handoff_binding, record_injected_handoff_binding,
    record_pending_handoff_binding, record_ready_handoff_binding, source_size,
};

const CLAUDE_CODE_SOURCE: &str = "claude-code";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const IMPORT_TIMEOUT: Duration = Duration::from_secs(180);
const COMPACT_TIMEOUT: Duration = Duration::from_secs(180);
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(180);

/// A Claude Code session that Codex can import or has already imported.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    #[serde(skip_serializing)]
    pub source_path: PathBuf,
    pub updated_at_unix_seconds: Option<u64>,
    pub source_size_bytes: Option<u64>,
    pub recommended_strategy: ClaudeTransferStrategy,
    pub state: ClaudeSessionState,
    pub codex_thread_id: Option<String>,
}

/// Whether a Claude session needs an import or can resume an existing Codex thread.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSessionState {
    ReadyToImport,
    Imported,
}

/// How Rebinder should move a Claude Code session into Codex.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeTransferStrategy {
    /// Choose a native full import for small sessions and a bounded handoff for large sessions.
    Auto,
    /// Import the complete Claude transcript through Codex's native importer.
    Full,
    /// Create or update a native Codex thread with a bounded Claude checkpoint.
    Handoff,
}

/// Result of preparing a Claude Code session for continuation in Codex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCodexSession {
    pub source_session_id: String,
    pub source_title: String,
    pub cwd: PathBuf,
    pub codex_thread_id: String,
    pub imported: bool,
    pub compacted: bool,
    pub activated: bool,
    pub strategy: ClaudeTransferStrategy,
    pub source_size_bytes: Option<u64>,
}

/// Errors returned by Claude-to-Codex discovery and transfer.
#[derive(Debug, Error)]
pub enum TransferError {
    #[error("cannot determine the current working directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("cannot launch Codex app-server: {0}")]
    AppServerLaunch(#[source] std::io::Error),
    #[error("cannot communicate with Codex app-server: {0}")]
    AppServerIo(#[source] std::io::Error),
    #[error("Codex app-server returned invalid JSON: {0}")]
    InvalidAppServerJson(#[source] serde_json::Error),
    #[error("Codex app-server closed before replying")]
    AppServerClosed,
    #[error("Codex app-server timed out while waiting for {operation}")]
    AppServerTimeout { operation: &'static str },
    #[error("Codex app-server rejected {operation}: {message}")]
    AppServerRejected {
        operation: &'static str,
        message: String,
    },
    #[error("Codex did not expose the Claude Code session importer; update Codex CLI and retry")]
    ImportUnsupported,
    #[error("no importable Claude Code sessions were found")]
    NoSessions,
    #[error(
        "no Claude Code session was found for the current directory; pass a session ID from `rebinder sessions claude`"
    )]
    NoSessionForCurrentDirectory,
    #[error("Claude Code session `{0}` was not found; run `rebinder sessions claude`")]
    SessionNotFound(String),
    #[error("Claude Code session ID `{0}` is ambiguous")]
    AmbiguousSession(String),
    #[error("Claude Code session `{session_id}` points to missing workspace `{cwd}`")]
    MissingWorkspace { session_id: String, cwd: PathBuf },
    #[error("Codex reported a session import failure: {0}")]
    ImportFailed(String),
    #[error("cannot prepare a context-safe Claude handoff: {0}")]
    Handoff(String),
    #[error("Codex did not return a target thread ID")]
    MissingCodexThread,
    #[error("Codex did not return a handoff activation turn ID")]
    MissingCodexTurn,
    #[error("cannot launch Codex thread: {0}")]
    CodexLaunch(#[source] std::io::Error),
}

#[derive(Debug, Clone)]
struct SessionRecord {
    public: ClaudeSession,
    migration_item: Option<Value>,
    imported_at_ms: Option<u64>,
    full_import_thread_id: Option<String>,
}

/// Discover Claude Code sessions through Codex's external-agent API.
pub fn discover_claude_sessions() -> Result<Vec<ClaudeSession>, TransferError> {
    let current_dir = std::env::current_dir().map_err(TransferError::CurrentDirectory)?;
    let mut app_server = CodexAppServer::launch(OsStr::new("codex"))?;
    let inventory = load_inventory(&mut app_server, &current_dir)?;
    Ok(inventory.into_iter().map(|record| record.public).collect())
}

/// Import a Claude Code session into Codex, or resolve its existing imported thread.
pub fn prepare_claude_to_codex(
    session_id: Option<&str>,
) -> Result<PreparedCodexSession, TransferError> {
    prepare_claude_to_codex_with_strategy(session_id, ClaudeTransferStrategy::Auto)
}

/// Import a Claude Code session using the requested transfer strategy.
pub fn prepare_claude_to_codex_with_strategy(
    session_id: Option<&str>,
    strategy: ClaudeTransferStrategy,
) -> Result<PreparedCodexSession, TransferError> {
    let current_dir = std::env::current_dir().map_err(TransferError::CurrentDirectory)?;
    let mut app_server = CodexAppServer::launch(OsStr::new("codex"))?;
    let inventory = load_inventory(&mut app_server, &current_dir)?;
    let selected = select_session(inventory, session_id, &current_dir)?;

    if !selected.public.cwd.is_dir() {
        return Err(TransferError::MissingWorkspace {
            session_id: selected.public.id,
            cwd: selected.public.cwd,
        });
    }

    let resolved_strategy = match strategy {
        ClaudeTransferStrategy::Auto => selected.public.recommended_strategy,
        explicit => explicit,
    };
    let source_size_bytes = selected.public.source_size_bytes;

    let (codex_thread_id, imported, compacted, activated) = match resolved_strategy {
        ClaudeTransferStrategy::Auto => unreachable!("auto strategy must resolve before import"),
        ClaudeTransferStrategy::Full => {
            if let Some(migration_item) = selected.migration_item.as_ref() {
                let thread_id =
                    app_server.import_session(migration_item, &selected.public.source_path)?;
                (thread_id, true, false, false)
            } else {
                let thread_id = selected
                    .full_import_thread_id
                    .clone()
                    .ok_or(TransferError::MissingCodexThread)?;
                (thread_id, false, false, false)
            }
        }
        ClaudeTransferStrategy::Handoff => {
            let handoff = prepare_context_safe_handoff(
                &selected.public.source_path,
                &selected.public.id,
                &selected.public.title,
                &selected.public.cwd,
            )
            .map_err(|error| TransferError::Handoff(error.to_string()))?;
            if handoff.binding.complete() {
                let thread_id = handoff
                    .codex_thread_id
                    .clone()
                    .ok_or(TransferError::MissingCodexThread)?;
                (thread_id, false, false, false)
            } else {
                let thread_id = if let Some(thread_id) = handoff.codex_thread_id.clone() {
                    app_server.resume_thread(&thread_id)?;
                    thread_id
                } else {
                    app_server.start_thread(&selected.public.cwd)?
                };
                let history_was_ready =
                    handoff.binding.injected() && !handoff.binding.requires_compaction();
                if !handoff.binding.injected() {
                    record_pending_handoff_binding(&handoff, &thread_id)
                        .map_err(|error| TransferError::Handoff(error.to_string()))?;
                    app_server.inject_handoff(&thread_id, &handoff.messages)?;
                    record_injected_handoff_binding(&handoff, &thread_id)
                        .map_err(|error| TransferError::Handoff(error.to_string()))?;
                }
                let compacted = if handoff.binding.requires_compaction() {
                    app_server.compact_thread(&thread_id)?;
                    true
                } else {
                    false
                };
                if !history_was_ready || compacted {
                    record_ready_handoff_binding(&handoff, &thread_id)
                        .map_err(|error| TransferError::Handoff(error.to_string()))?;
                }
                let activation_already_completed = if handoff.binding.activation_started() {
                    app_server.handoff_activation_completed(&thread_id, &handoff.source_sha256)?
                } else {
                    false
                };
                let activated = if activation_already_completed {
                    false
                } else {
                    record_activating_handoff_binding(&handoff, &thread_id)
                        .map_err(|error| TransferError::Handoff(error.to_string()))?;
                    app_server.activate_handoff(
                        &thread_id,
                        &selected.public.cwd,
                        &handoff.source_sha256,
                    )?;
                    true
                };
                record_completed_handoff_binding(&handoff, &thread_id)
                    .map_err(|error| TransferError::Handoff(error.to_string()))?;
                (thread_id, true, compacted, activated)
            }
        }
    };

    Ok(PreparedCodexSession {
        source_session_id: selected.public.id,
        source_title: selected.public.title,
        cwd: selected.public.cwd,
        codex_thread_id,
        imported,
        compacted,
        activated,
        strategy: resolved_strategy,
        source_size_bytes,
    })
}

/// Resume an imported thread with the native Codex CLI in its recorded workspace.
pub fn launch_prepared_codex_session<I, S>(
    prepared: &PreparedCodexSession,
    target_arguments: I,
) -> Result<ExitStatus, TransferError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut arguments = vec![
        OsString::from("resume"),
        OsString::from(&prepared.codex_thread_id),
    ];
    arguments.extend(
        target_arguments
            .into_iter()
            .map(|argument| argument.as_ref().to_os_string()),
    );

    Command::new("codex")
        .args(arguments)
        .current_dir(&prepared.cwd)
        .status()
        .map_err(TransferError::CodexLaunch)
}

fn load_inventory(
    app_server: &mut CodexAppServer,
    current_dir: &Path,
) -> Result<Vec<SessionRecord>, TransferError> {
    let detected = app_server.detect(current_dir)?;
    let histories = app_server.read_import_histories()?;
    Ok(merge_inventory(&detected, &histories))
}

fn merge_inventory(detected: &Value, histories: &Value) -> Vec<SessionRecord> {
    let mut records = imported_session_records(histories);

    for detected_record in detected_session_records(detected) {
        let key = path_key(&detected_record.public.source_path);
        if let Some(previous) = records.get(&key) {
            let mut merged = detected_record;
            merged
                .public
                .codex_thread_id
                .clone_from(&previous.public.codex_thread_id);
            merged.imported_at_ms = previous.imported_at_ms;
            merged
                .full_import_thread_id
                .clone_from(&previous.full_import_thread_id);
            if !source_changed_since_import(&merged.public.source_path, previous.imported_at_ms) {
                merged.public.state = ClaudeSessionState::Imported;
                merged.migration_item = None;
            }
            records.insert(key, merged);
        } else {
            records.insert(key, detected_record);
        }
    }

    let mut inventory = records.into_values().collect::<Vec<_>>();
    for record in &mut inventory {
        if let Ok(Some(thread_id)) = completed_handoff_thread(&record.public.source_path) {
            record.public.state = ClaudeSessionState::Imported;
            if record.public.recommended_strategy == ClaudeTransferStrategy::Handoff
                || record.public.codex_thread_id.is_none()
            {
                record.public.codex_thread_id = Some(thread_id);
            }
        }
    }
    inventory.sort_by_key(|record| Reverse(session_recency(record)));
    inventory
}

fn detected_session_records(response: &Value) -> Vec<SessionRecord> {
    let mut records = Vec::new();
    let Some(items) = response
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| response.pointer("/result/items").and_then(Value::as_array))
    else {
        return records;
    };

    for item in items {
        if item.get("itemType").and_then(Value::as_str) != Some("SESSIONS") {
            continue;
        }
        let Some(sessions) = item.pointer("/details/sessions").and_then(Value::as_array) else {
            continue;
        };
        for session in sessions {
            let Some(source_path) = session
                .get("path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
            else {
                continue;
            };
            let Some(cwd) = session
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from)
            else {
                continue;
            };
            let title = session
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Claude session")
                .to_owned();
            let Some(id) = session_id_from_path(&source_path) else {
                continue;
            };

            let mut migration_item = item.clone();
            if let Some(session_list) = migration_item
                .pointer_mut("/details/sessions")
                .and_then(Value::as_array_mut)
            {
                *session_list = vec![session.clone()];
            }
            records.push(SessionRecord {
                public: ClaudeSession {
                    id,
                    title,
                    cwd,
                    updated_at_unix_seconds: modified_unix_seconds(&source_path),
                    source_size_bytes: source_size(&source_path),
                    recommended_strategy: recommended_strategy(&source_path),
                    source_path,
                    state: ClaudeSessionState::ReadyToImport,
                    codex_thread_id: None,
                },
                migration_item: Some(migration_item),
                imported_at_ms: None,
                full_import_thread_id: None,
            });
        }
    }
    records
}

fn imported_session_records(histories: &Value) -> BTreeMap<String, SessionRecord> {
    let mut records = BTreeMap::new();
    let Some(history_entries) = histories
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| histories.pointer("/result/data").and_then(Value::as_array))
    else {
        return records;
    };

    for history in history_entries {
        let provider = history.get("providerId").and_then(Value::as_str);
        if provider.is_some_and(|provider| provider != CLAUDE_CODE_SOURCE) {
            continue;
        }
        let imported_at_ms = history.get("completedAtMs").and_then(Value::as_u64);
        let Some(successes) = history.get("successes").and_then(Value::as_array) else {
            continue;
        };
        for success in successes {
            if success.get("itemType").and_then(Value::as_str) != Some("SESSIONS") {
                continue;
            }
            let Some(source_path) = success
                .get("source")
                .and_then(Value::as_str)
                .map(PathBuf::from)
            else {
                continue;
            };
            if !looks_like_claude_session_path(&source_path) {
                continue;
            }
            let Some(codex_thread_id) = success
                .get("target")
                .and_then(Value::as_str)
                .filter(|target| !target.is_empty())
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            let Some(cwd) = success
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from)
            else {
                continue;
            };
            let Some(id) = session_id_from_path(&source_path) else {
                continue;
            };
            let title = success
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Imported Claude session")
                .to_owned();
            let key = path_key(&source_path);
            let replace = records.get(&key).is_none_or(|previous: &SessionRecord| {
                imported_at_ms.unwrap_or_default() >= previous.imported_at_ms.unwrap_or_default()
            });
            if replace {
                records.insert(
                    key,
                    SessionRecord {
                        public: ClaudeSession {
                            id,
                            title,
                            cwd,
                            updated_at_unix_seconds: modified_unix_seconds(&source_path),
                            source_size_bytes: source_size(&source_path),
                            recommended_strategy: recommended_strategy(&source_path),
                            source_path,
                            state: ClaudeSessionState::Imported,
                            codex_thread_id: Some(codex_thread_id.clone()),
                        },
                        migration_item: None,
                        imported_at_ms,
                        full_import_thread_id: Some(codex_thread_id),
                    },
                );
            }
        }
    }
    records
}

fn select_session(
    inventory: Vec<SessionRecord>,
    session_id: Option<&str>,
    current_dir: &Path,
) -> Result<SessionRecord, TransferError> {
    if inventory.is_empty() {
        return Err(TransferError::NoSessions);
    }

    if let Some(session_id) = session_id {
        let mut matches = inventory
            .into_iter()
            .filter(|record| record.public.id == session_id);
        let selected = matches
            .next()
            .ok_or_else(|| TransferError::SessionNotFound(session_id.to_owned()))?;
        if matches.next().is_some() {
            return Err(TransferError::AmbiguousSession(session_id.to_owned()));
        }
        return Ok(selected);
    }

    inventory
        .into_iter()
        .filter(|record| paths_equivalent(&record.public.cwd, current_dir))
        .max_by_key(session_recency)
        .ok_or(TransferError::NoSessionForCurrentDirectory)
}

fn session_recency(record: &SessionRecord) -> u64 {
    record
        .public
        .updated_at_unix_seconds
        .map(|seconds| seconds.saturating_mul(1_000))
        .or(record.imported_at_ms)
        .unwrap_or_default()
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn session_id_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(OsStr::to_str)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
}

fn modified_unix_seconds(path: &Path) -> Option<u64> {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn modified_unix_milliseconds(path: &Path) -> Option<u64> {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn source_changed_since_import(path: &Path, imported_at_ms: Option<u64>) -> bool {
    modified_unix_milliseconds(path)
        .zip(imported_at_ms)
        .is_some_and(|(modified_at_ms, imported_at_ms)| modified_at_ms > imported_at_ms)
}

fn recommended_strategy(path: &Path) -> ClaudeTransferStrategy {
    if recommended_handoff(path) {
        ClaudeTransferStrategy::Handoff
    } else {
        debug_assert!(
            source_size(path).is_none_or(|bytes| bytes <= FULL_IMPORT_MAX_SOURCE_BYTES),
            "full import recommendation must stay within its size threshold"
        );
        ClaudeTransferStrategy::Full
    }
}

fn looks_like_claude_session_path(path: &Path) -> bool {
    if let Some(config_directory) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        let projects = PathBuf::from(config_directory).join("projects");
        if path.starts_with(&projects)
            || path
                .canonicalize()
                .ok()
                .zip(projects.canonicalize().ok())
                .is_some_and(|(path, projects)| path.starts_with(projects))
        {
            return true;
        }
    }

    let components = path
        .components()
        .map(std::path::Component::as_os_str)
        .collect::<Vec<_>>();
    components
        .windows(2)
        .any(|pair| pair[0] == ".claude" && pair[1] == "projects")
}

fn path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

struct CodexAppServer {
    child: Child,
    stdin: Option<ChildStdin>,
    messages: Receiver<Result<Value, RpcReadError>>,
    pending: VecDeque<Value>,
    reader: Option<JoinHandle<()>>,
}

#[derive(Debug)]
enum RpcReadError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl CodexAppServer {
    fn launch(executable: &OsStr) -> Result<Self, TransferError> {
        let mut child = Command::new(executable)
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(TransferError::AppServerLaunch)?;
        let stdin = child.stdin.take().ok_or(TransferError::AppServerClosed)?;
        let stdout = child.stdout.take().ok_or(TransferError::AppServerClosed)?;
        let (sender, messages) = mpsc::channel();
        let reader = thread::Builder::new()
            .name("rebinder-codex-app-server".to_owned())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let value = match line {
                        Ok(line) => serde_json::from_str(&line).map_err(RpcReadError::Json),
                        Err(error) => Err(RpcReadError::Io(error)),
                    };
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            });
        let reader = match reader {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TransferError::AppServerLaunch(error));
            }
        };
        let mut server = Self {
            child,
            stdin: Some(stdin),
            messages,
            pending: VecDeque::new(),
            reader: Some(reader),
        };
        server.initialize()?;
        Ok(server)
    }

    fn initialize(&mut self) -> Result<(), TransferError> {
        self.request(
            0,
            "initialize",
            json!({
                "clientInfo": {
                    "name": "rebinder",
                    "title": "Rebinder",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }),
            RPC_TIMEOUT,
            "initialization",
        )?;
        self.send(&json!({ "method": "initialized", "params": {} }))
    }

    fn detect(&mut self, current_dir: &Path) -> Result<Value, TransferError> {
        let result = self.request(
            1,
            "externalAgentConfig/detect",
            json!({
                "includeHome": true,
                "cwds": [current_dir],
                "migrationSource": CLAUDE_CODE_SOURCE
            }),
            RPC_TIMEOUT,
            "Claude session discovery",
        );
        match result {
            Err(TransferError::AppServerRejected { message, .. })
                if message.contains("method") || message.contains("experimental") =>
            {
                Err(TransferError::ImportUnsupported)
            }
            other => other,
        }
    }

    fn read_import_histories(&mut self) -> Result<Value, TransferError> {
        self.request(
            2,
            "externalAgentConfig/import/readHistories",
            Value::Null,
            RPC_TIMEOUT,
            "import history lookup",
        )
    }

    fn start_thread(&mut self, cwd: &Path) -> Result<String, TransferError> {
        let result = self.request(
            5,
            "thread/start",
            json!({
                "cwd": cwd,
                "serviceName": "rebinder"
            }),
            RPC_TIMEOUT,
            "native handoff thread creation",
        )?;
        result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or(TransferError::MissingCodexThread)
    }

    fn resume_thread(&mut self, thread_id: &str) -> Result<(), TransferError> {
        self.request(
            5,
            "thread/resume",
            json!({ "threadId": thread_id }),
            RPC_TIMEOUT,
            "native handoff thread resume",
        )?;
        Ok(())
    }

    fn inject_handoff(
        &mut self,
        thread_id: &str,
        messages: &[HandoffMessage],
    ) -> Result<(), TransferError> {
        let items = messages
            .iter()
            .map(|message| {
                let content_type = match message.role {
                    HandoffMessageRole::User => "input_text",
                    HandoffMessageRole::Assistant => "output_text",
                };
                json!({
                    "type": "message",
                    "role": message.role.wire_role(),
                    "content": [{
                        "type": content_type,
                        "text": message.text
                    }]
                })
            })
            .collect::<Vec<_>>();
        self.request(
            6,
            "thread/inject_items",
            json!({
                "threadId": thread_id,
                "items": items
            }),
            RPC_TIMEOUT,
            "context-safe handoff injection",
        )?;
        Ok(())
    }

    fn compact_thread(&mut self, thread_id: &str) -> Result<(), TransferError> {
        self.request(
            7,
            "thread/compact/start",
            json!({ "threadId": thread_id }),
            RPC_TIMEOUT,
            "context-safe handoff compaction start",
        )?;
        let completed = self.receive_matching(
            |message| {
                message.get("method").and_then(Value::as_str) == Some("turn/completed")
                    && message.pointer("/params/threadId").and_then(Value::as_str)
                        == Some(thread_id)
            },
            COMPACT_TIMEOUT,
            "context-safe handoff compaction completion",
        )?;
        if completed
            .pointer("/params/turn/status")
            .and_then(Value::as_str)
            == Some("completed")
        {
            return Ok(());
        }
        let message = completed
            .pointer("/params/turn/error/message")
            .and_then(Value::as_str)
            .or_else(|| {
                completed
                    .pointer("/params/turn/error/additionalDetails")
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                completed
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
            })
            .unwrap_or("Codex compaction did not complete")
            .to_owned();
        Err(TransferError::AppServerRejected {
            operation: "context-safe handoff compaction",
            message,
        })
    }

    fn handoff_activation_completed(
        &mut self,
        thread_id: &str,
        source_sha256: &str,
    ) -> Result<bool, TransferError> {
        let result = self.request(
            8,
            "thread/read",
            json!({
                "threadId": thread_id,
                "includeTurns": true
            }),
            RPC_TIMEOUT,
            "handoff activation recovery",
        )?;
        let marker = handoff_activation_marker(source_sha256);
        Ok(result
            .pointer("/thread/turns")
            .and_then(Value::as_array)
            .is_some_and(|turns| {
                turns.iter().any(|turn| {
                    turn.get("status").and_then(Value::as_str) == Some("completed")
                        && turn_contains_activation_marker(turn, &marker)
                        && turn_has_visible_agent_message(turn)
                })
            }))
    }

    fn activate_handoff(
        &mut self,
        thread_id: &str,
        cwd: &Path,
        source_sha256: &str,
    ) -> Result<(), TransferError> {
        let response = self.request(
            9,
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{
                    "type": "text",
                    "text": handoff_activation_prompt(source_sha256)
                }],
                "cwd": cwd,
                "approvalPolicy": "never",
                "sandboxPolicy": { "type": "readOnly" }
            }),
            RPC_TIMEOUT,
            "handoff continuity activation start",
        )?;
        let turn_id = response
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or(TransferError::MissingCodexTurn)?
            .to_owned();
        let completed = self.receive_matching(
            |message| {
                message.get("method").and_then(Value::as_str) == Some("turn/completed")
                    && message.pointer("/params/threadId").and_then(Value::as_str)
                        == Some(thread_id)
                    && message.pointer("/params/turn/id").and_then(Value::as_str)
                        == Some(turn_id.as_str())
            },
            ACTIVATION_TIMEOUT,
            "handoff continuity activation completion",
        )?;
        if completed
            .pointer("/params/turn/status")
            .and_then(Value::as_str)
            == Some("completed")
        {
            return if completed
                .pointer("/params/turn")
                .is_some_and(turn_has_visible_agent_message)
            {
                Ok(())
            } else {
                Err(TransferError::AppServerRejected {
                    operation: "handoff continuity activation",
                    message: "Codex completed without a visible continuation brief".to_owned(),
                })
            };
        }
        let message = turn_failure_message(&completed, "Codex handoff activation did not complete");
        Err(TransferError::AppServerRejected {
            operation: "handoff continuity activation",
            message,
        })
    }

    fn import_session(
        &mut self,
        migration_item: &Value,
        source_path: &Path,
    ) -> Result<String, TransferError> {
        let response = self.request(
            3,
            "externalAgentConfig/import",
            json!({
                "migrationItems": [migration_item],
                "source": CLAUDE_CODE_SOURCE
            }),
            RPC_TIMEOUT,
            "session import",
        )?;
        let import_id = response
            .get("importId")
            .and_then(Value::as_str)
            .ok_or(TransferError::MissingCodexThread)?
            .to_owned();
        let completed = self.receive_matching(
            |message| {
                message.get("method").and_then(Value::as_str)
                    == Some("externalAgentConfig/import/completed")
                    && message.pointer("/params/importId").and_then(Value::as_str)
                        == Some(import_id.as_str())
            },
            IMPORT_TIMEOUT,
            "session import completion",
        )?;

        if let Some(thread_id) = imported_thread_from_completion(&completed, source_path) {
            return Ok(thread_id);
        }
        if let Some(message) = import_failure_from_completion(&completed, source_path) {
            return Err(TransferError::ImportFailed(message));
        }

        let histories = self.request(
            4,
            "externalAgentConfig/import/readHistories",
            Value::Null,
            RPC_TIMEOUT,
            "import result lookup",
        )?;
        imported_thread_from_histories(&histories, source_path)
            .ok_or(TransferError::MissingCodexThread)
    }

    fn request(
        &mut self,
        id: u64,
        method: &'static str,
        params: Value,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<Value, TransferError> {
        let mut message = json!({ "method": method, "id": id });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(&message)?;
        let response = self.receive_matching(
            |candidate| candidate.get("id").and_then(Value::as_u64) == Some(id),
            timeout,
            operation,
        )?;
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown app-server error")
                .to_owned();
            return Err(TransferError::AppServerRejected { operation, message });
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    fn send(&mut self, message: &Value) -> Result<(), TransferError> {
        let stdin = self.stdin.as_mut().ok_or(TransferError::AppServerClosed)?;
        serde_json::to_writer(&mut *stdin, &message).map_err(|error| {
            TransferError::AppServerIo(std::io::Error::other(error.to_string()))
        })?;
        stdin
            .write_all(b"\n")
            .and_then(|()| stdin.flush())
            .map_err(TransferError::AppServerIo)
    }

    fn receive_matching(
        &mut self,
        predicate: impl Fn(&Value) -> bool,
        timeout: Duration,
        operation: &'static str,
    ) -> Result<Value, TransferError> {
        if let Some(position) = self.pending.iter().position(&predicate) {
            return Ok(self
                .pending
                .remove(position)
                .expect("pending position must exist"));
        }

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(TransferError::AppServerTimeout { operation });
            }
            match self.messages.recv_timeout(remaining) {
                Ok(Ok(message)) if predicate(&message) => return Ok(message),
                Ok(Ok(message)) => self.pending.push_back(message),
                Ok(Err(RpcReadError::Io(error))) => {
                    return Err(TransferError::AppServerIo(error));
                }
                Ok(Err(RpcReadError::Json(error))) => {
                    return Err(TransferError::InvalidAppServerJson(error));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(TransferError::AppServerTimeout { operation });
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(TransferError::AppServerClosed);
                }
            }
        }
    }
}

fn handoff_activation_marker(source_sha256: &str) -> String {
    format!("Rebinder handoff revision: {source_sha256}")
}

fn handoff_activation_prompt(source_sha256: &str) -> String {
    format!(
        "Rebinder continuity activation. Using only the historical Claude Code context already present in this thread, write a concise continuation brief covering the current objective, verified state, important decisions, and the next concrete action. Do not call tools or modify files. If context is insufficient, state exactly what is missing.\n\n{}",
        handoff_activation_marker(source_sha256)
    )
}

fn turn_contains_activation_marker(turn: &Value, marker: &str) -> bool {
    turn.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("userMessage")
                    && item
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|content| {
                            content.iter().any(|part| {
                                part.get("text")
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| text.contains(marker))
                            })
                        })
            })
        })
}

fn turn_has_visible_agent_message(turn: &Value) -> bool {
    turn.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("agentMessage")
                    && item
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
            })
        })
}

fn turn_failure_message(completed: &Value, fallback: &str) -> String {
    completed
        .pointer("/params/turn/error/message")
        .and_then(Value::as_str)
        .or_else(|| {
            completed
                .pointer("/params/turn/error/additionalDetails")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            completed
                .pointer("/params/turn/status")
                .and_then(Value::as_str)
        })
        .unwrap_or(fallback)
        .to_owned()
}

impl Drop for CodexAppServer {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn imported_thread_from_completion(completed: &Value, source_path: &Path) -> Option<String> {
    completed
        .pointer("/params/itemTypeResults")
        .and_then(Value::as_array)?
        .iter()
        .filter(|result| result.get("itemType").and_then(Value::as_str) == Some("SESSIONS"))
        .filter_map(|result| result.get("successes").and_then(Value::as_array))
        .flatten()
        .find(|success| success_source_matches(success, source_path))?
        .get("target")?
        .as_str()
        .filter(|target| !target.is_empty())
        .map(ToOwned::to_owned)
}

fn import_failure_from_completion(completed: &Value, source_path: &Path) -> Option<String> {
    completed
        .pointer("/params/itemTypeResults")
        .and_then(Value::as_array)?
        .iter()
        .filter(|result| result.get("itemType").and_then(Value::as_str) == Some("SESSIONS"))
        .filter_map(|result| result.get("failures").and_then(Value::as_array))
        .flatten()
        .find(|failure| success_source_matches(failure, source_path))?
        .get("message")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn imported_thread_from_histories(histories: &Value, source_path: &Path) -> Option<String> {
    histories
        .get("data")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|history| history.get("successes").and_then(Value::as_array))
        .flatten()
        .filter(|success| success.get("itemType").and_then(Value::as_str) == Some("SESSIONS"))
        .find(|success| success_source_matches(success, source_path))?
        .get("target")?
        .as_str()
        .filter(|target| !target.is_empty())
        .map(ToOwned::to_owned)
}

fn success_source_matches(success: &Value, source_path: &Path) -> bool {
    success
        .get("source")
        .and_then(Value::as_str)
        .is_some_and(|source| paths_equivalent(Path::new(source), source_path))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn detected(source: &Path, cwd: &Path, title: &str) -> Value {
        json!({
            "items": [{
                "itemType": "SESSIONS",
                "description": "Import Claude sessions",
                "cwd": null,
                "details": {
                    "plugins": [],
                    "skills": [],
                    "sessions": [{ "path": source, "cwd": cwd, "title": title }],
                    "mcpServers": [],
                    "hooks": [],
                    "subagents": [],
                    "commands": []
                }
            }]
        })
    }

    #[test]
    fn detection_creates_a_single_session_migration_item() {
        let response = detected(
            Path::new("/tmp/1234.jsonl"),
            Path::new("/tmp/project"),
            "Fix parser",
        );
        let records = detected_session_records(&response);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].public.id, "1234");
        assert_eq!(records[0].public.title, "Fix parser");
        assert_eq!(records[0].public.state, ClaudeSessionState::ReadyToImport);
        assert_eq!(
            records[0]
                .migration_item
                .as_ref()
                .and_then(|item| item.pointer("/details/sessions"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn history_maps_a_claude_session_to_its_codex_thread() {
        let histories = json!({
            "data": [{
                "providerId": "claude-code",
                "completedAtMs": 42,
                "successes": [{
                    "itemType": "SESSIONS",
                    "cwd": "/tmp/project",
                    "source": "/tmp/.claude/projects/project/1234.jsonl",
                    "target": "019c-thread",
                    "title": "Fix parser"
                }]
            }]
        });
        let records = imported_session_records(&histories);
        let record = records.values().next().expect("imported record");
        assert_eq!(record.public.id, "1234");
        assert_eq!(
            record.public.codex_thread_id.as_deref(),
            Some("019c-thread")
        );
        assert_eq!(record.public.state, ClaudeSessionState::Imported);
    }

    #[test]
    fn history_ignores_untrusted_providerless_session_paths() {
        let histories = json!({
            "data": [{
                "completedAtMs": 42,
                "successes": [{
                    "itemType": "SESSIONS",
                    "cwd": "/tmp/project",
                    "source": "/tmp/1234.jsonl",
                    "target": "019c-thread",
                    "title": "Fix parser"
                }]
            }]
        });

        assert!(imported_session_records(&histories).is_empty());
    }

    #[test]
    fn history_accepts_legacy_providerless_entries_from_the_claude_store() {
        let histories = json!({
            "data": [{
                "completedAtMs": 42,
                "successes": [{
                    "itemType": "SESSIONS",
                    "cwd": "/tmp/project",
                    "source": "/tmp/.claude/projects/project/1234.jsonl",
                    "target": "019c-thread",
                    "title": "Fix parser"
                }]
            }]
        });

        let records = imported_session_records(&histories);
        assert_eq!(
            records
                .values()
                .next()
                .and_then(|record| record.public.codex_thread_id.as_deref()),
            Some("019c-thread")
        );
    }

    #[test]
    fn unchanged_imported_session_resumes_without_another_migration() {
        let fixture = tempfile::tempdir().expect("fixture");
        let source = fixture.path().join(".claude/projects/project/1234.jsonl");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source directory");
        fs::write(&source, "{\"type\":\"user\"}\n").expect("source session");
        let cwd = fixture.path().join("project");
        fs::create_dir(&cwd).expect("workspace");
        let completed_at = modified_unix_milliseconds(&source)
            .expect("source timestamp")
            .saturating_add(1_000);
        let histories = json!({
            "data": [{
                "providerId": "claude-code",
                "completedAtMs": completed_at,
                "successes": [{
                    "itemType": "SESSIONS",
                    "cwd": cwd,
                    "source": source,
                    "target": "019c-thread",
                    "title": "Fix parser"
                }]
            }]
        });
        let inventory = merge_inventory(&detected(&source, &cwd, "Fix parser"), &histories);
        assert_eq!(inventory.len(), 1);
        assert!(inventory[0].migration_item.is_none());
        assert_eq!(inventory[0].public.state, ClaudeSessionState::Imported);
        assert_eq!(
            inventory[0].public.codex_thread_id.as_deref(),
            Some("019c-thread")
        );
    }

    #[test]
    fn changed_imported_session_keeps_its_binding_and_requests_a_checkpoint() {
        let fixture = tempfile::tempdir().expect("fixture");
        let source = fixture.path().join(".claude/projects/project/1234.jsonl");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source directory");
        fs::write(&source, "{\"type\":\"user\"}\n").expect("source session");
        let cwd = fixture.path().join("project");
        fs::create_dir(&cwd).expect("workspace");
        let histories = json!({
            "data": [{
                "providerId": "claude-code",
                "completedAtMs": 42,
                "successes": [{
                    "itemType": "SESSIONS",
                    "cwd": cwd,
                    "source": source,
                    "target": "019c-thread",
                    "title": "Fix parser"
                }]
            }]
        });
        let inventory = merge_inventory(&detected(&source, &cwd, "Fix parser"), &histories);
        assert_eq!(inventory.len(), 1);
        assert!(inventory[0].migration_item.is_some());
        assert_eq!(
            inventory[0].public.codex_thread_id.as_deref(),
            Some("019c-thread")
        );
        assert_eq!(inventory[0].public.state, ClaudeSessionState::ReadyToImport);
    }

    #[test]
    fn explicit_session_selection_is_exact() {
        let cwd = PathBuf::from("/tmp/project");
        let records = vec!["first", "second"]
            .into_iter()
            .map(|id| SessionRecord {
                public: ClaudeSession {
                    id: id.to_owned(),
                    title: id.to_owned(),
                    cwd: cwd.clone(),
                    source_path: PathBuf::from(format!("/tmp/{id}.jsonl")),
                    updated_at_unix_seconds: None,
                    source_size_bytes: None,
                    recommended_strategy: ClaudeTransferStrategy::Full,
                    state: ClaudeSessionState::ReadyToImport,
                    codex_thread_id: None,
                },
                migration_item: Some(json!({})),
                imported_at_ms: None,
                full_import_thread_id: None,
            })
            .collect();
        let selected = select_session(records, Some("second"), &cwd).expect("select session");
        assert_eq!(selected.public.id, "second");
    }

    #[test]
    fn implicit_selection_stays_inside_the_current_workspace() {
        let current = tempfile::tempdir().expect("current workspace");
        let other = tempfile::tempdir().expect("other workspace");
        let record = |id: &str, cwd: &Path, updated| SessionRecord {
            public: ClaudeSession {
                id: id.to_owned(),
                title: id.to_owned(),
                cwd: cwd.to_path_buf(),
                source_path: cwd.join(format!("{id}.jsonl")),
                updated_at_unix_seconds: Some(updated),
                source_size_bytes: None,
                recommended_strategy: ClaudeTransferStrategy::Full,
                state: ClaudeSessionState::ReadyToImport,
                codex_thread_id: None,
            },
            migration_item: Some(json!({})),
            imported_at_ms: None,
            full_import_thread_id: None,
        };
        let records = vec![
            record("current-old", current.path(), 10),
            record("other-new", other.path(), 30),
            record("current-new", current.path(), 20),
        ];
        let selected = select_session(records, None, current.path()).expect("select session");
        assert_eq!(selected.public.id, "current-new");
    }

    #[test]
    fn completion_returns_the_imported_thread_id() {
        let completed = json!({
            "method": "externalAgentConfig/import/completed",
            "params": {
                "importId": "import-1",
                "itemTypeResults": [{
                    "itemType": "SESSIONS",
                    "successes": [{
                        "source": "/tmp/1234.jsonl",
                        "target": "019c-thread"
                    }],
                    "failures": []
                }]
            }
        });
        assert_eq!(
            imported_thread_from_completion(&completed, Path::new("/tmp/1234.jsonl")),
            Some("019c-thread".to_owned())
        );
    }

    #[test]
    fn activation_recovery_requires_the_matching_user_marker() {
        let marker = handoff_activation_marker("source-hash");
        let turn = json!({
            "status": "completed",
            "items": [
                {
                    "type": "userMessage",
                    "content": [{ "type": "text", "text": handoff_activation_prompt("source-hash") }]
                },
                {
                    "type": "agentMessage",
                    "text": "Continue from the verified parser state."
                }
            ]
        });

        assert!(turn_contains_activation_marker(&turn, &marker));
        assert!(turn_has_visible_agent_message(&turn));
        assert!(!turn_contains_activation_marker(
            &turn,
            &handoff_activation_marker("different-hash")
        ));
    }
}
