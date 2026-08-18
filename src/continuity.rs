//! Consent-gated, limit-aware cross-harness continuity for Claude Code.

use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::Harness;

pub const DEFAULT_FIVE_HOUR_THRESHOLD: u8 = 90;
pub const DEFAULT_SEVEN_DAY_THRESHOLD: u8 = 85;

const RECEIPT_SCHEMA_VERSION: u8 = 1;
const OFFER_SCHEMA_VERSION: u8 = 1;
const STATUSLINE_COMMAND: &str = "rebinder continuity observe";
const LAUNCH_ID_ENV: &str = "REBINDER_LAUNCH_ID";
const MANAGED_MARKER: &str =
    include_str!("../integrations/claude-code/rebinder-continuity/.rebinder-managed");
const PLUGIN_MANIFEST: &str =
    include_str!("../integrations/claude-code/rebinder-continuity/.claude-plugin/plugin.json");
const PLUGIN_HOOKS: &str =
    include_str!("../integrations/claude-code/rebinder-continuity/hooks/hooks.json");
const PLUGIN_SKILL: &str =
    include_str!("../integrations/claude-code/rebinder-continuity/skills/handoff/SKILL.md");

#[derive(Debug, Error)]
pub enum ContinuityError {
    #[error("cannot determine the Rebinder data directory")]
    DataDirectoryUnavailable,
    #[error("cannot determine the Claude Code configuration directory")]
    ClaudeConfigurationUnavailable,
    #[error("unsupported continuity route: {source_harness:?} to {target:?}")]
    UnsupportedRoute {
        source_harness: Harness,
        target: Harness,
    },
    #[error("usage threshold must be between 1 and 100")]
    InvalidThreshold,
    #[error("Codex is not authenticated; run `codex login` before enabling this continuity target")]
    TargetUnavailable,
    #[error("cannot inspect `{path}`: {source}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsafe managed path `{0}`")]
    UnsafePath(PathBuf),
    #[error("cannot create directory `{path}`: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot decode `{path}`: {source}")]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot encode continuity state: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("Claude Code settings must contain a JSON object")]
    InvalidSettings,
    #[error(
        "Claude Code statusLine changed after Rebinder enabled continuity; restore it or disable continuity before replacing it"
    )]
    StatusLineOwnershipLost,
    #[error("the Claude Code plugin path contains files not managed by Rebinder: `{0}`")]
    UnmanagedPlugin(PathBuf),
    #[error("invalid Claude Code status-line input: {0}")]
    InvalidStatusLine(serde_json::Error),
    #[error("invalid Claude Code hook input: {0}")]
    InvalidHook(serde_json::Error),
    #[error("continuity is not enabled")]
    NotEnabled,
    #[error("no matching continuity offer is available")]
    OfferNotFound,
    #[error("continuity offer `{0}` has already been declined")]
    OfferDeclined(String),
    #[error("continuity offer `{0}` has already been completed")]
    OfferCompleted(String),
    #[error("cannot execute the previous Claude Code status line: {0}")]
    PreviousStatusLine(std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ContinuityReceipt {
    schema_version: u8,
    source: Harness,
    target: Harness,
    five_hour_threshold: u8,
    seven_day_threshold: u8,
    previous_status_line: Option<Value>,
    claude_settings_path: PathBuf,
    plugin_path: PathBuf,
    enabled_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub used_percentage: f64,
    pub resets_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityObservation {
    pub session_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub launch_id: Option<String>,
    pub five_hour: Option<LimitWindow>,
    pub seven_day: Option<LimitWindow>,
    pub observed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityOfferReason {
    SevenDayLimit,
    FiveHourLimit,
}

impl ContinuityOfferReason {
    const fn label(self) -> &'static str {
        match self {
            Self::SevenDayLimit => "7-day",
            Self::FiveHourLimit => "5-hour",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityOffer {
    pub schema_version: u8,
    pub id: String,
    pub source: Harness,
    pub target: Harness,
    pub session_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub launch_id: Option<String>,
    pub reason: ContinuityOfferReason,
    pub five_hour: Option<LimitWindow>,
    pub seven_day: Option<LimitWindow>,
    pub created_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityOfferState {
    Ready,
    Asked,
    Accepted,
    Declined,
    Completed,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityOfferStatus {
    #[serde(flatten)]
    pub offer: ContinuityOffer,
    pub state: ContinuityOfferState,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityStatus {
    pub enabled: bool,
    pub source: Option<Harness>,
    pub target: Option<Harness>,
    pub five_hour_threshold: Option<u8>,
    pub seven_day_threshold: Option<u8>,
    pub target_available: bool,
    pub latest_observation: Option<ContinuityObservation>,
    pub offers: Vec<ContinuityOfferStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityInstallation {
    pub enabled: bool,
    pub source: Harness,
    pub target: Harness,
    pub five_hour_threshold: u8,
    pub seven_day_threshold: u8,
    pub settings_path: PathBuf,
    pub plugin_path: PathBuf,
}

#[derive(Debug)]
pub struct StatusLineRender {
    pub output: Vec<u8>,
    pub exit_code: u8,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeStatusLineInput {
    session_id: String,
    cwd: PathBuf,
    transcript_path: Option<PathBuf>,
    rate_limits: Option<ClaudeRateLimits>,
}

#[derive(Debug, Deserialize)]
struct ClaudeRateLimits {
    five_hour: Option<ClaudeLimitWindow>,
    seven_day: Option<ClaudeLimitWindow>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ClaudeLimitWindow {
    used_percentage: f64,
    resets_at: u64,
}

impl From<ClaudeLimitWindow> for LimitWindow {
    fn from(window: ClaudeLimitWindow) -> Self {
        Self {
            used_percentage: window.used_percentage,
            resets_at: window.resets_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClaudeHookInput {
    session_id: String,
    cwd: PathBuf,
    hook_event_name: String,
}

/// Install Rebinder's personal Claude Code plugin and status-line observer.
pub fn enable_claude_continuity(
    source: Harness,
    target: Harness,
    five_hour_threshold: u8,
    seven_day_threshold: u8,
) -> Result<ContinuityInstallation, ContinuityError> {
    validate_route(source, target)?;
    validate_threshold(five_hour_threshold)?;
    validate_threshold(seven_day_threshold)?;
    if !target_is_available(target) {
        return Err(ContinuityError::TargetUnavailable);
    }

    let settings_path = claude_config_directory()?.join("settings.json");
    let plugin_path = claude_config_directory()?
        .join("skills")
        .join("rebinder-continuity");
    let mut settings = read_settings(&settings_path)?;
    let existing_receipt = read_receipt()?;
    let previous_status_line = if let Some(receipt) = existing_receipt.as_ref() {
        if receipt.claude_settings_path != settings_path || receipt.plugin_path != plugin_path {
            return Err(ContinuityError::StatusLineOwnershipLost);
        }
        let current = settings.get("statusLine");
        let expected_wrapper = statusline_wrapper(receipt.previous_status_line.as_ref());
        if current != Some(&expected_wrapper) && current != receipt.previous_status_line.as_ref() {
            return Err(ContinuityError::StatusLineOwnershipLost);
        }
        receipt.previous_status_line.clone()
    } else {
        if is_statusline_wrapper(settings.get("statusLine")) {
            return Err(ContinuityError::StatusLineOwnershipLost);
        }
        settings.get("statusLine").cloned()
    };

    install_plugin(&plugin_path)?;
    let receipt = ContinuityReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        source,
        target,
        five_hour_threshold,
        seven_day_threshold,
        previous_status_line: previous_status_line.clone(),
        claude_settings_path: settings_path.clone(),
        plugin_path: plugin_path.clone(),
        enabled_at_unix_seconds: now_unix_seconds(),
    };
    write_json(&receipt_path()?, &receipt)?;

    let wrapper = statusline_wrapper(previous_status_line.as_ref());
    settings.insert("statusLine".to_owned(), wrapper);
    write_json(&settings_path, &Value::Object(settings))?;

    Ok(ContinuityInstallation {
        enabled: true,
        source,
        target,
        five_hour_threshold,
        seven_day_threshold,
        settings_path,
        plugin_path,
    })
}

/// Restore the prior Claude status line and remove Rebinder's managed plugin files.
pub fn disable_claude_continuity(
    source: Harness,
) -> Result<ContinuityInstallation, ContinuityError> {
    if source != Harness::Claude {
        return Err(ContinuityError::UnsupportedRoute {
            source_harness: source,
            target: Harness::Codex,
        });
    }
    let Some(receipt) = read_receipt()? else {
        let config = claude_config_directory()?;
        return Ok(ContinuityInstallation {
            enabled: false,
            source,
            target: Harness::Codex,
            five_hour_threshold: DEFAULT_FIVE_HOUR_THRESHOLD,
            seven_day_threshold: DEFAULT_SEVEN_DAY_THRESHOLD,
            settings_path: config.join("settings.json"),
            plugin_path: config.join("skills").join("rebinder-continuity"),
        });
    };
    if receipt.source != source {
        return Err(ContinuityError::UnsupportedRoute {
            source_harness: source,
            target: receipt.target,
        });
    }
    let mut settings = read_settings(&receipt.claude_settings_path)?;
    let current = settings.get("statusLine");
    let expected_wrapper = statusline_wrapper(receipt.previous_status_line.as_ref());
    if current != Some(&expected_wrapper) && current != receipt.previous_status_line.as_ref() {
        return Err(ContinuityError::StatusLineOwnershipLost);
    }
    match receipt.previous_status_line.clone() {
        Some(previous) => {
            settings.insert("statusLine".to_owned(), previous);
        }
        None => {
            settings.remove("statusLine");
        }
    }
    write_json(&receipt.claude_settings_path, &Value::Object(settings))?;
    remove_plugin(&receipt.plugin_path)?;
    remove_regular_file(&receipt_path()?)?;
    Ok(ContinuityInstallation {
        enabled: false,
        source: receipt.source,
        target: receipt.target,
        five_hour_threshold: receipt.five_hour_threshold,
        seven_day_threshold: receipt.seven_day_threshold,
        settings_path: receipt.claude_settings_path,
        plugin_path: receipt.plugin_path,
    })
}

/// Return the configured policy, latest provider observation, and offer ledger.
pub fn continuity_status() -> Result<ContinuityStatus, ContinuityError> {
    let receipt = read_receipt()?;
    let mut offers = read_offers()?
        .into_iter()
        .map(|offer| ContinuityOfferStatus {
            state: offer_state(&offer.id),
            offer,
        })
        .collect::<Vec<_>>();
    offers.sort_by_key(|status| status.offer.created_at_unix_seconds);
    let latest_observation = read_observations()?
        .into_iter()
        .max_by_key(|observation| observation.observed_at_unix_seconds);
    Ok(ContinuityStatus {
        enabled: receipt.is_some(),
        source: receipt.as_ref().map(|receipt| receipt.source),
        target: receipt.as_ref().map(|receipt| receipt.target),
        five_hour_threshold: receipt.as_ref().map(|receipt| receipt.five_hour_threshold),
        seven_day_threshold: receipt.as_ref().map(|receipt| receipt.seven_day_threshold),
        target_available: receipt
            .as_ref()
            .is_some_and(|receipt| target_is_available(receipt.target)),
        latest_observation,
        offers,
    })
}

/// Observe Claude's status-line payload while preserving any prior status-line command.
pub fn process_claude_statusline(input: &[u8]) -> Result<StatusLineRender, ContinuityError> {
    let receipt = read_receipt()?.ok_or(ContinuityError::NotEnabled)?;
    let previous = run_previous_statusline(&receipt, input)?;
    let mut render = StatusLineRender {
        output: previous.0,
        exit_code: previous.1,
        diagnostic: None,
    };
    let parsed = match serde_json::from_slice::<ClaudeStatusLineInput>(input) {
        Ok(parsed) => parsed,
        Err(error) => {
            render.diagnostic = Some(ContinuityError::InvalidStatusLine(error).to_string());
            return Ok(render);
        }
    };
    let observation = ContinuityObservation {
        session_id: parsed.session_id,
        cwd: parsed.cwd,
        transcript_path: parsed.transcript_path,
        launch_id: env::var(LAUNCH_ID_ENV)
            .ok()
            .filter(|value| !value.is_empty()),
        five_hour: parsed
            .rate_limits
            .as_ref()
            .and_then(|limits| limits.five_hour.as_ref())
            .copied()
            .map(LimitWindow::from)
            .and_then(|window| valid_window(Some(window))),
        seven_day: parsed
            .rate_limits
            .as_ref()
            .and_then(|limits| limits.seven_day.as_ref())
            .copied()
            .map(LimitWindow::from)
            .and_then(|window| valid_window(Some(window))),
        observed_at_unix_seconds: now_unix_seconds(),
    };
    write_observation(&observation)?;
    let offer = maybe_create_offer(&receipt, &observation)?;
    if render.output.is_empty() {
        render.output = default_status_line(&observation, offer.as_ref()).into_bytes();
    } else if let Some(offer) = offer.as_ref() {
        if !render.output.ends_with(b"\n") {
            render.output.push(b'\n');
        }
        render
            .output
            .extend_from_slice(offer_status_line(offer).as_bytes());
    }
    Ok(render)
}

/// Produce Claude hook JSON for the first unasked offer in the active session.
pub fn claude_hook_output(input: &[u8]) -> Result<Option<Value>, ContinuityError> {
    let receipt = read_receipt()?.ok_or(ContinuityError::NotEnabled)?;
    let hook: ClaudeHookInput =
        serde_json::from_slice(input).map_err(ContinuityError::InvalidHook)?;
    let mut offers = read_offers()?
        .into_iter()
        .filter(|offer| {
            offer.session_id == hook.session_id
                && paths_equivalent(&offer.cwd, &hook.cwd)
                && offer.target == receipt.target
                && offer_state(&offer.id) == ContinuityOfferState::Ready
        })
        .collect::<Vec<_>>();
    offers.sort_by_key(|offer| offer.created_at_unix_seconds);
    let Some(offer) = offers.pop() else {
        return Ok(None);
    };
    if !write_marker_if_absent("asked", &offer.id)? {
        return Ok(None);
    }
    let context = offer_context(&offer);
    Ok(Some(json!({
        "hookSpecificOutput": {
            "hookEventName": hook.hook_event_name,
            "additionalContext": context
        }
    })))
}

/// Record explicit approval for a pending continuity offer.
pub fn accept_continuity_offer(
    explicit_offer_id: Option<&str>,
) -> Result<ContinuityOffer, ContinuityError> {
    let offer = resolve_offer(explicit_offer_id)?;
    match offer_state(&offer.id) {
        ContinuityOfferState::Declined => {
            return Err(ContinuityError::OfferDeclined(offer.id));
        }
        ContinuityOfferState::Completed => {
            return Err(ContinuityError::OfferCompleted(offer.id));
        }
        ContinuityOfferState::Accepted => return Ok(offer),
        ContinuityOfferState::Ready | ContinuityOfferState::Asked => {}
    }
    write_marker_if_absent("accepted", &offer.id)?;
    Ok(offer)
}

/// Record an explicit rejection and suppress the offer for the current limit window.
pub fn decline_continuity_offer(
    explicit_offer_id: Option<&str>,
) -> Result<ContinuityOffer, ContinuityError> {
    let offer = resolve_offer(explicit_offer_id)?;
    if offer_state(&offer.id) == ContinuityOfferState::Completed {
        return Err(ContinuityError::OfferCompleted(offer.id));
    }
    write_marker_if_absent("declined", &offer.id)?;
    Ok(offer)
}

/// Find an accepted, unfinished handoff for a particular Rebinder-owned Claude process.
pub fn accepted_offer_for_launch(
    launch_id: &str,
) -> Result<Option<ContinuityOffer>, ContinuityError> {
    let mut offers = read_offers()?
        .into_iter()
        .filter(|offer| {
            offer.launch_id.as_deref() == Some(launch_id)
                && offer_state(&offer.id) == ContinuityOfferState::Accepted
        })
        .collect::<Vec<_>>();
    offers.sort_by_key(|offer| offer.created_at_unix_seconds);
    Ok(offers.pop())
}

/// Resolve an accepted offer for the manual continuity resume fallback.
pub fn accepted_continuity_offer(
    explicit_offer_id: Option<&str>,
) -> Result<ContinuityOffer, ContinuityError> {
    let offer = resolve_offer(explicit_offer_id)?;
    if offer_state(&offer.id) != ContinuityOfferState::Accepted {
        return Err(ContinuityError::OfferNotFound);
    }
    Ok(offer)
}

/// Mark an accepted offer completed after the target harness has actually started.
pub fn mark_continuity_offer_completed(offer_id: &str) -> Result<(), ContinuityError> {
    write_marker_if_absent("completed", offer_id)?;
    Ok(())
}

/// Create a process-scoped identifier propagated from `rebinder claude` to its plugin hooks.
pub fn new_continuity_launch_id() -> String {
    let seed = format!(
        "{}:{}:{}",
        std::process::id(),
        now_unix_nanos(),
        env::current_dir().map_or_else(|_| String::new(), |path| path.display().to_string())
    );
    digest(seed.as_bytes())
}

fn validate_route(source: Harness, target: Harness) -> Result<(), ContinuityError> {
    if source == Harness::Claude && target == Harness::Codex {
        Ok(())
    } else {
        Err(ContinuityError::UnsupportedRoute {
            source_harness: source,
            target,
        })
    }
}

fn validate_threshold(value: u8) -> Result<(), ContinuityError> {
    if (1..=100).contains(&value) {
        Ok(())
    } else {
        Err(ContinuityError::InvalidThreshold)
    }
}

fn target_is_available(target: Harness) -> bool {
    match target {
        Harness::Codex => Command::new("codex")
            .args(["login", "status"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success()),
        Harness::Claude => false,
    }
}

fn claude_config_directory() -> Result<PathBuf, ContinuityError> {
    if let Some(path) = env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    BaseDirs::new()
        .map(|directories| directories.home_dir().join(".claude"))
        .ok_or(ContinuityError::ClaudeConfigurationUnavailable)
}

fn continuity_directory() -> Result<PathBuf, ContinuityError> {
    ProjectDirs::from("", "", "rebinder")
        .map(|directories| directories.data_dir().join("continuity"))
        .ok_or(ContinuityError::DataDirectoryUnavailable)
}

fn receipt_path() -> Result<PathBuf, ContinuityError> {
    Ok(continuity_directory()?.join("claude.json"))
}

fn read_receipt() -> Result<Option<ContinuityReceipt>, ContinuityError> {
    read_json_optional(&receipt_path()?)
}

fn read_settings(path: &Path) -> Result<Map<String, Value>, ContinuityError> {
    match read_json_optional::<Value>(path)? {
        Some(Value::Object(settings)) => Ok(settings),
        Some(_) => Err(ContinuityError::InvalidSettings),
        None => Ok(Map::new()),
    }
}

fn statusline_wrapper(previous: Option<&Value>) -> Value {
    let mut wrapper = previous
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    wrapper.insert("type".to_owned(), Value::String("command".to_owned()));
    wrapper.insert(
        "command".to_owned(),
        Value::String(STATUSLINE_COMMAND.to_owned()),
    );
    Value::Object(wrapper)
}

fn is_statusline_wrapper(value: Option<&Value>) -> bool {
    value
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|command| command.trim() == STATUSLINE_COMMAND)
}

fn install_plugin(path: &Path) -> Result<(), ContinuityError> {
    validate_managed_plugin(path, false)?;
    write_text(&path.join(".rebinder-managed"), MANAGED_MARKER)?;
    write_text(
        &path.join(".claude-plugin").join("plugin.json"),
        PLUGIN_MANIFEST,
    )?;
    write_text(&path.join("hooks").join("hooks.json"), PLUGIN_HOOKS)?;
    write_text(
        &path.join("skills").join("handoff").join("SKILL.md"),
        PLUGIN_SKILL,
    )?;
    Ok(())
}

fn validate_managed_plugin(path: &Path, must_exist: bool) -> Result<(), ContinuityError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound && !must_exist => {
            return Ok(());
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ContinuityError::Inspect {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ContinuityError::UnsafePath(path.to_path_buf()));
    }
    let marker = path.join(".rebinder-managed");
    if fs::read_to_string(&marker).ok().as_deref() != Some(MANAGED_MARKER) {
        return Err(ContinuityError::UnmanagedPlugin(path.to_path_buf()));
    }
    let expected = BTreeSet::from([
        PathBuf::from(".rebinder-managed"),
        PathBuf::from(".claude-plugin"),
        PathBuf::from(".claude-plugin/plugin.json"),
        PathBuf::from("hooks"),
        PathBuf::from("hooks/hooks.json"),
        PathBuf::from("skills"),
        PathBuf::from("skills/handoff"),
        PathBuf::from("skills/handoff/SKILL.md"),
    ]);
    for entry in relative_entries(path, path)? {
        if !expected.contains(&entry) {
            return Err(ContinuityError::UnmanagedPlugin(path.to_path_buf()));
        }
    }
    Ok(())
}

fn relative_entries(root: &Path, directory: &Path) -> Result<Vec<PathBuf>, ContinuityError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| ContinuityError::Read {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ContinuityError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
        let kind = entry
            .file_type()
            .map_err(|source| ContinuityError::Inspect {
                path: entry.path(),
                source,
            })?;
        if kind.is_symlink() {
            return Err(ContinuityError::UnsafePath(entry.path()));
        }
        if kind.is_dir() {
            entries.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| ContinuityError::UnsafePath(entry.path()))?
                    .to_path_buf(),
            );
            entries.extend(relative_entries(root, &entry.path())?);
        } else if kind.is_file() {
            entries.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| ContinuityError::UnsafePath(entry.path()))?
                    .to_path_buf(),
            );
        } else {
            return Err(ContinuityError::UnsafePath(entry.path()));
        }
    }
    Ok(entries)
}

fn remove_plugin(path: &Path) -> Result<(), ContinuityError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ContinuityError::Inspect {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ContinuityError::UnsafePath(path.to_path_buf()));
    }
    validate_managed_plugin(path, true)?;
    for file in [
        path.join("skills/handoff/SKILL.md"),
        path.join("hooks/hooks.json"),
        path.join(".claude-plugin/plugin.json"),
        path.join(".rebinder-managed"),
    ] {
        remove_regular_file(&file)?;
    }
    for directory in [
        path.join("skills/handoff"),
        path.join("skills"),
        path.join("hooks"),
        path.join(".claude-plugin"),
        path.to_path_buf(),
    ] {
        fs::remove_dir(&directory).map_err(|source| ContinuityError::Write {
            path: directory,
            source,
        })?;
    }
    Ok(())
}

fn run_previous_statusline(
    receipt: &ContinuityReceipt,
    input: &[u8],
) -> Result<(Vec<u8>, u8), ContinuityError> {
    let Some(command) = receipt
        .previous_status_line
        .as_ref()
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .filter(|command| command.trim() != STATUSLINE_COMMAND)
    else {
        return Ok((Vec::new(), 0));
    };
    let mut process = shell_command(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(ContinuityError::PreviousStatusLine)?;
    if let Some(mut stdin) = process.stdin.take() {
        if let Err(error) = stdin.write_all(input) {
            if error.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(ContinuityError::PreviousStatusLine(error));
            }
        }
    }
    let output = process
        .wait_with_output()
        .map_err(ContinuityError::PreviousStatusLine)?;
    let code = output
        .status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1);
    Ok((output.stdout, code))
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let shell = env::var_os("SHELL").unwrap_or_else(|| "sh".into());
    let mut process = Command::new(shell);
    process.args(["-c", command]);
    process
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    if let Some(shell) = env::var_os("SHELL").filter(|shell| !shell.is_empty()) {
        let mut process = Command::new(shell);
        process.args(["-c", command]);
        return process;
    }
    let mut process = Command::new("cmd.exe");
    process.args(["/D", "/S", "/C", command]);
    process
}

fn valid_window(window: Option<LimitWindow>) -> Option<LimitWindow> {
    window.filter(|window| {
        window.used_percentage.is_finite()
            && (0.0..=100.0).contains(&window.used_percentage)
            && window.resets_at > 0
    })
}

fn write_observation(observation: &ContinuityObservation) -> Result<(), ContinuityError> {
    let path = continuity_directory()?.join("observations").join(format!(
        "{}.json",
        digest(observation.session_id.as_bytes())
    ));
    write_json(&path, observation)
}

fn maybe_create_offer(
    receipt: &ContinuityReceipt,
    observation: &ContinuityObservation,
) -> Result<Option<ContinuityOffer>, ContinuityError> {
    let seven_triggered = observation
        .seven_day
        .as_ref()
        .is_some_and(|window| window.used_percentage >= f64::from(receipt.seven_day_threshold));
    let five_triggered = observation
        .five_hour
        .as_ref()
        .is_some_and(|window| window.used_percentage >= f64::from(receipt.five_hour_threshold));
    let (reason, reset) = if seven_triggered {
        (
            ContinuityOfferReason::SevenDayLimit,
            observation
                .seven_day
                .as_ref()
                .map(|window| window.resets_at),
        )
    } else if five_triggered {
        (
            ContinuityOfferReason::FiveHourLimit,
            observation
                .five_hour
                .as_ref()
                .map(|window| window.resets_at),
        )
    } else {
        return Ok(None);
    };
    let Some(reset) = reset.filter(|reset| *reset > now_unix_seconds()) else {
        return Ok(None);
    };
    let id = digest(
        format!(
            "v{OFFER_SCHEMA_VERSION}:{}:{}:{}:{reset}",
            observation.session_id,
            receipt.target.executable(),
            reason.label()
        )
        .as_bytes(),
    );
    let offer = ContinuityOffer {
        schema_version: OFFER_SCHEMA_VERSION,
        id,
        source: receipt.source,
        target: receipt.target,
        session_id: observation.session_id.clone(),
        cwd: observation.cwd.clone(),
        transcript_path: observation.transcript_path.clone(),
        launch_id: observation.launch_id.clone(),
        reason,
        five_hour: observation.five_hour.clone(),
        seven_day: observation.seven_day.clone(),
        created_at_unix_seconds: now_unix_seconds(),
    };
    let path = offer_path(&offer.id)?;
    if path.exists() {
        let existing = read_json_optional::<ContinuityOffer>(&path)?;
        return Ok(existing.filter(|offer| {
            matches!(
                offer_state(&offer.id),
                ContinuityOfferState::Ready | ContinuityOfferState::Asked
            )
        }));
    }
    if !target_is_available(receipt.target) {
        return Ok(None);
    }
    match write_json_create_new(&path, &offer) {
        Ok(()) => Ok(Some(offer)),
        Err(ContinuityError::Write { source, .. })
            if source.kind() == std::io::ErrorKind::AlreadyExists =>
        {
            read_json_optional(&path)
        }
        Err(error) => Err(error),
    }
}

fn default_status_line(
    observation: &ContinuityObservation,
    offer: Option<&ContinuityOffer>,
) -> String {
    let five = observation
        .five_hour
        .as_ref()
        .map_or_else(|| "n/a".to_owned(), percentage);
    let seven = observation
        .seven_day
        .as_ref()
        .map_or_else(|| "n/a".to_owned(), percentage);
    let suffix = offer.map_or("", |_| " · handoff offer ready");
    format!("↪ rebinder continuity · 5h {five} · 7d {seven}{suffix}\n")
}

fn offer_status_line(offer: &ContinuityOffer) -> String {
    let usage = match offer.reason {
        ContinuityOfferReason::SevenDayLimit => offer.seven_day.as_ref(),
        ContinuityOfferReason::FiveHourLimit => offer.five_hour.as_ref(),
    }
    .map_or_else(|| "unknown".to_owned(), percentage);
    format!(
        "↪ rebinder: {} usage {usage}; {} handoff offer ready\n",
        offer.reason.label(),
        offer.target.executable()
    )
}

fn percentage(window: &LimitWindow) -> String {
    format!("{:.0}%", window.used_percentage)
}

fn offer_context(offer: &ContinuityOffer) -> String {
    let observed = match offer.reason {
        ContinuityOfferReason::SevenDayLimit => offer.seven_day.as_ref(),
        ContinuityOfferReason::FiveHourLimit => offer.five_hour.as_ref(),
    };
    let usage = observed.map_or_else(|| "unknown".to_owned(), percentage);
    let reset = observed.map_or(0, |window| window.resets_at);
    let continuation = if offer.launch_id.is_some() {
        "After acceptance, the user exits Claude Code with `/exit`; the enclosing `rebinder claude` process then prepares and opens Codex automatically."
    } else {
        "After acceptance, the user exits Claude Code and runs the fallback command printed by Rebinder (`rebinder continuity resume --offer <id>`)."
    };
    format!(
        "Rebinder continuity offer `{id}` is ready for this session. Claude.ai {window} usage is {usage} and the window resets at Unix time {reset}. The authenticated {target} target was available when the offer was created. User authorization is required. Ask the user once whether they want to hand this exact Claude session off to {target} before the limit is exhausted. An explicit affirmative response authorizes `rebinder continuity accept --offer {id}`; an explicit negative response authorizes `rebinder continuity decline --offer {id}`. An ambiguous response authorizes neither action. Never invoke `rebinder transfer` from inside Claude Code. {continuation}",
        id = offer.id,
        window = offer.reason.label(),
        target = offer.target.executable(),
    )
}

fn resolve_offer(explicit_offer_id: Option<&str>) -> Result<ContinuityOffer, ContinuityError> {
    if let Some(id) = explicit_offer_id {
        validate_offer_id(id)?;
        return read_json_optional(&offer_path(id)?)?.ok_or(ContinuityError::OfferNotFound);
    }
    let launch_id = env::var(LAUNCH_ID_ENV).ok();
    let mut offers = read_offers()?
        .into_iter()
        .filter(|offer| {
            offer_state(&offer.id) != ContinuityOfferState::Completed
                && launch_id
                    .as_deref()
                    .is_none_or(|launch_id| offer.launch_id.as_deref() == Some(launch_id))
        })
        .collect::<Vec<_>>();
    offers.sort_by_key(|offer| offer.created_at_unix_seconds);
    offers.pop().ok_or(ContinuityError::OfferNotFound)
}

fn validate_offer_id(id: &str) -> Result<(), ContinuityError> {
    if id.len() == 64 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ContinuityError::OfferNotFound)
    }
}

fn offer_path(id: &str) -> Result<PathBuf, ContinuityError> {
    validate_offer_id(id)?;
    Ok(continuity_directory()?
        .join("offers")
        .join(format!("{id}.json")))
}

fn read_offers() -> Result<Vec<ContinuityOffer>, ContinuityError> {
    read_json_directory(&continuity_directory()?.join("offers"))
}

fn read_observations() -> Result<Vec<ContinuityObservation>, ContinuityError> {
    read_json_directory(&continuity_directory()?.join("observations"))
}

fn read_json_directory<T>(directory: &Path) -> Result<Vec<T>, ContinuityError>
where
    T: for<'de> Deserialize<'de>,
{
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ContinuityError::Inspect {
                path: directory.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ContinuityError::UnsafePath(directory.to_path_buf()));
    }
    let mut values = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| ContinuityError::Read {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ContinuityError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
        if entry.path().extension() != Some(OsStr::new("json")) {
            continue;
        }
        if let Some(value) = read_json_optional(&entry.path())? {
            values.push(value);
        }
    }
    Ok(values)
}

fn offer_state(id: &str) -> ContinuityOfferState {
    let Ok(directory) = continuity_directory() else {
        return ContinuityOfferState::Ready;
    };
    if directory
        .join("completed")
        .join(format!("{id}.json"))
        .is_file()
    {
        ContinuityOfferState::Completed
    } else if directory
        .join("declined")
        .join(format!("{id}.json"))
        .is_file()
    {
        ContinuityOfferState::Declined
    } else if directory
        .join("accepted")
        .join(format!("{id}.json"))
        .is_file()
    {
        ContinuityOfferState::Accepted
    } else if directory.join("asked").join(format!("{id}.json")).is_file() {
        ContinuityOfferState::Asked
    } else {
        ContinuityOfferState::Ready
    }
}

fn write_marker_if_absent(kind: &str, id: &str) -> Result<bool, ContinuityError> {
    validate_offer_id(id)?;
    let path = continuity_directory()?
        .join(kind)
        .join(format!("{id}.json"));
    let marker = json!({
        "offerId": id,
        "recordedAtUnixSeconds": now_unix_seconds()
    });
    match write_json_create_new(&path, &marker) {
        Ok(()) => Ok(true),
        Err(ContinuityError::Write { source, .. })
            if source.kind() == std::io::ErrorKind::AlreadyExists =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

fn read_json_optional<T>(path: &Path) -> Result<Option<T>, ContinuityError>
where
    T: for<'de> Deserialize<'de>,
{
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ContinuityError::Inspect {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ContinuityError::UnsafePath(path.to_path_buf()));
    }
    let mut file = fs::File::open(path).map_err(|source| ContinuityError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|source| ContinuityError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::from_slice(&contents)
        .map(Some)
        .map_err(|source| ContinuityError::Decode {
            path: path.to_path_buf(),
            source,
        })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ContinuityError> {
    let mut contents = serde_json::to_vec_pretty(value)?;
    contents.push(b'\n');
    write_bytes(path, &contents, false)
}

fn write_json_create_new(path: &Path, value: &impl Serialize) -> Result<(), ContinuityError> {
    let mut contents = serde_json::to_vec_pretty(value)?;
    contents.push(b'\n');
    write_bytes(path, &contents, true)
}

fn write_text(path: &Path, value: &str) -> Result<(), ContinuityError> {
    write_bytes(path, value.as_bytes(), false)
}

fn write_bytes(path: &Path, contents: &[u8], create_new: bool) -> Result<(), ContinuityError> {
    let parent = path
        .parent()
        .ok_or_else(|| ContinuityError::UnsafePath(path.to_path_buf()))?;
    ensure_directory(parent)?;
    if create_new {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options
            .open(path)
            .map_err(|source| ContinuityError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        if let Err(error) = set_private_file_permissions(path) {
            let _ = fs::remove_file(path);
            return Err(error);
        }
        if let Err(source) = file.write_all(contents).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(ContinuityError::Write {
                path: path.to_path_buf(),
                source,
            });
        }
        return Ok(());
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ContinuityError::UnsafePath(path.to_path_buf()));
        }
    }
    let temporary = parent.join(format!(
        ".rebinder-{}-{}-{}.tmp",
        path.file_name().and_then(OsStr::to_str).unwrap_or("state"),
        std::process::id(),
        now_unix_nanos()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|source| ContinuityError::Write {
            path: temporary.clone(),
            source,
        })?;
    if let Err(error) = set_private_file_permissions(&temporary) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(source) = file.write_all(contents).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ContinuityError::Write {
            path: temporary.clone(),
            source,
        });
    }
    replace_file(&temporary, path)
}

fn replace_file(temporary: &Path, target: &Path) -> Result<(), ContinuityError> {
    if !target.exists() {
        return fs::rename(temporary, target).map_err(|source| ContinuityError::Write {
            path: target.to_path_buf(),
            source,
        });
    }
    let backup = target.with_extension(format!(
        "rebinder-backup-{}-{}",
        std::process::id(),
        now_unix_nanos()
    ));
    fs::rename(target, &backup).map_err(|source| ContinuityError::Write {
        path: target.to_path_buf(),
        source,
    })?;
    match fs::rename(temporary, target) {
        Ok(()) => remove_regular_file(&backup),
        Err(source) => {
            let _ = fs::rename(&backup, target);
            Err(ContinuityError::Write {
                path: target.to_path_buf(),
                source,
            })
        }
    }
}

fn ensure_directory(path: &Path) -> Result<(), ContinuityError> {
    fs::create_dir_all(path).map_err(|source| ContinuityError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = fs::symlink_metadata(path).map_err(|source| ContinuityError::Inspect {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ContinuityError::UnsafePath(path.to_path_buf()));
    }
    set_private_directory_permissions(path)
}

fn remove_regular_file(path: &Path) -> Result<(), ContinuityError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ContinuityError::Inspect {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ContinuityError::UnsafePath(path.to_path_buf()));
    }
    fs::remove_file(path).map_err(|source| ContinuityError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), ContinuityError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        ContinuityError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), ContinuityError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), ContinuityError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
        ContinuityError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), ContinuityError> {
    Ok(())
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(left, right)| left == right)
}

fn digest(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = Sha256::digest(value);
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_ids_are_scoped_to_the_limit_window() {
        let observation = ContinuityObservation {
            session_id: "session-1".to_owned(),
            cwd: PathBuf::from("/workspace"),
            transcript_path: None,
            launch_id: Some("launch-1".to_owned()),
            five_hour: Some(LimitWindow {
                used_percentage: 91.0,
                resets_at: u64::MAX - 1,
            }),
            seven_day: Some(LimitWindow {
                used_percentage: 86.0,
                resets_at: u64::MAX,
            }),
            observed_at_unix_seconds: 1,
        };
        let receipt = ContinuityReceipt {
            schema_version: 1,
            source: Harness::Claude,
            target: Harness::Codex,
            five_hour_threshold: 90,
            seven_day_threshold: 85,
            previous_status_line: None,
            claude_settings_path: PathBuf::from("settings.json"),
            plugin_path: PathBuf::from("plugin"),
            enabled_at_unix_seconds: 1,
        };
        let reason =
            if observation.seven_day.as_ref().is_some_and(|window| {
                window.used_percentage >= f64::from(receipt.seven_day_threshold)
            }) {
                ContinuityOfferReason::SevenDayLimit
            } else {
                ContinuityOfferReason::FiveHourLimit
            };
        let first = digest(
            format!(
                "v1:{}:{}:{}:{}",
                observation.session_id,
                receipt.target.executable(),
                reason.label(),
                observation
                    .seven_day
                    .as_ref()
                    .expect("weekly window")
                    .resets_at
            )
            .as_bytes(),
        );
        let second = digest(
            format!(
                "v1:{}:{}:{}:{}",
                observation.session_id,
                receipt.target.executable(),
                reason.label(),
                observation
                    .seven_day
                    .as_ref()
                    .expect("weekly window")
                    .resets_at
                    - 1
            )
            .as_bytes(),
        );
        assert_ne!(first, second);
    }

    #[test]
    fn invalid_limit_measurements_are_ignored() {
        assert!(
            valid_window(Some(LimitWindow {
                used_percentage: f64::NAN,
                resets_at: 1,
            }))
            .is_none()
        );
        assert!(
            valid_window(Some(LimitWindow {
                used_percentage: 50.0,
                resets_at: 1,
            }))
            .is_some()
        );
    }
}
