use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    harness::Harness,
    model::{
        ConversationItem, ConversationRole, Manifest, Provenance, RepositoryState, Session,
        TaskState, WorkspaceState,
    },
    validation::{ValidationReport, validate_package},
};

const RECENT_CONVERSATION_MAX_CHARS: usize = 40_000;
const MESSAGE_MAX_CHARS: usize = 12_000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Preserved,
    Summarized,
    Omitted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDeclaration {
    pub capability: String,
    pub support: CapabilitySupport,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub provider: String,
    pub adapter_version: String,
    pub artifact_format: String,
    pub capabilities: Vec<CapabilityDeclaration>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityLevel {
    Compatible,
    CompatibleWithLoss,
    Incompatible,
}

impl CompatibilityLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::CompatibleWithLoss => "compatible_with_loss",
            Self::Incompatible => "incompatible",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityFindingSeverity {
    InformationLoss,
    Blocking,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityFinding {
    pub severity: CompatibilityFindingSeverity,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityReport {
    pub can_continue: bool,
    pub level: CompatibilityLevel,
    pub validation: ValidationReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider: Option<String>,
    pub target: ProviderCapabilities,
    pub findings: Vec<CompatibilityFinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedContinuationArtifact {
    pub path: PathBuf,
    pub target_provider: String,
    pub media_type: String,
    pub sha256: String,
    pub bytes: usize,
    pub compatibility: CompatibilityReport,
}

#[derive(Debug, Error)]
pub enum CompatibilityError {
    #[error("cannot read canonical package document `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot decode canonical package document `{path}`: {source}")]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("cannot prepare an artifact from an invalid package ({errors} validation error(s))")]
    InvalidPackage { errors: usize },
    #[error("cannot inspect package compatibility: {0}")]
    Compatibility(#[from] CompatibilityError),
    #[error("continuation artifact already exists: `{0}`")]
    AlreadyExists(PathBuf),
    #[error("cannot write continuation artifact `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

struct CanonicalPackage {
    manifest: Manifest,
    session: Session,
    conversation: Vec<ConversationItem>,
    task: TaskState,
    workspace: WorkspaceState,
    repository: RepositoryState,
    handoff: String,
    provenance: Provenance,
}

#[derive(Debug, Default)]
struct PackageFeatures {
    role_counts: BTreeMap<ConversationRole, usize>,
    tool_calls: usize,
    tool_results: usize,
    attachments: usize,
    visible_text_chars: usize,
    truncated_messages: usize,
    environment_entries: usize,
    workspace_files: usize,
    remote_urls: usize,
    patch_files: usize,
}

pub fn provider_capabilities(target: Harness) -> ProviderCapabilities {
    let provider = provider_name(target);
    ProviderCapabilities {
        provider: provider.to_owned(),
        adapter_version: env!("CARGO_PKG_VERSION").to_owned(),
        artifact_format: "text/markdown; profile=rebinder.continuation.v1".to_owned(),
        capabilities: vec![
            capability(
                "conversation.text",
                CapabilitySupport::Preserved,
                "Visible user and assistant text is retained within the artifact context budget.",
            ),
            capability(
                "conversation.roles",
                CapabilitySupport::Summarized,
                "User and assistant roles become explicit Markdown sections; system roles become annotations.",
            ),
            capability(
                "conversation.tool_calls",
                CapabilitySupport::Summarized,
                "Tool execution is represented by counts; provider-specific inputs are not copied.",
            ),
            capability(
                "conversation.tool_results",
                CapabilitySupport::Omitted,
                "Tool output is not copied into a provider-neutral continuation artifact.",
            ),
            capability(
                "conversation.attachments",
                CapabilitySupport::Omitted,
                "Attachment payloads and provider-private references are not embedded.",
            ),
            capability(
                "task_state",
                CapabilitySupport::Preserved,
                "Intent, status, plan, decisions, constraints, and open questions are rendered explicitly.",
            ),
            capability(
                "workspace_state",
                CapabilitySupport::Summarized,
                "Recorded cwd and roots are retained; environment values and file inventories are not copied.",
            ),
            capability(
                "repository_state",
                CapabilitySupport::Summarized,
                "Git head and working-tree change metadata are retained without remote URLs.",
            ),
            capability(
                "patches",
                CapabilitySupport::Summarized,
                "Declared patch paths are referenced but patches are not embedded or applied.",
            ),
            capability(
                "handoff",
                CapabilitySupport::Preserved,
                "The package handoff is retained as the primary continuation guidance.",
            ),
            capability(
                "provenance",
                CapabilitySupport::Preserved,
                "Source adapter, export time, transformations, and redaction counts are retained.",
            ),
        ],
    }
}

pub fn assess_package_compatibility(
    package_root: impl AsRef<Path>,
    target: Harness,
) -> Result<CompatibilityReport, CompatibilityError> {
    let root = package_root.as_ref();
    let validation = validate_package(root);
    let target = provider_capabilities(target);
    if !validation.valid {
        let findings = validation
            .issues
            .iter()
            .filter(|issue| issue.severity == crate::validation::IssueSeverity::Error)
            .map(|issue| CompatibilityFinding {
                severity: CompatibilityFindingSeverity::Blocking,
                code: format!("package.{}", issue.code),
                path: issue.path.clone(),
                message: issue.message.clone(),
            })
            .collect();
        return Ok(CompatibilityReport {
            can_continue: false,
            level: CompatibilityLevel::Incompatible,
            validation,
            source_provider: None,
            target,
            findings,
        });
    }

    let package = read_package(root)?;
    let features = package_features(&package);
    let findings = feature_findings(&features);
    let level = if findings.is_empty() {
        CompatibilityLevel::Compatible
    } else {
        CompatibilityLevel::CompatibleWithLoss
    };
    Ok(CompatibilityReport {
        can_continue: true,
        level,
        validation,
        source_provider: Some(package.manifest.source.provider.clone()),
        target,
        findings,
    })
}

pub fn prepare_continuation_artifact(
    package_root: impl AsRef<Path>,
    target: Harness,
    output: impl AsRef<Path>,
) -> Result<PreparedContinuationArtifact, ArtifactError> {
    let root = package_root.as_ref();
    let output = output.as_ref();
    let compatibility = assess_package_compatibility(root, target)?;
    if !compatibility.can_continue {
        return Err(ArtifactError::InvalidPackage {
            errors: compatibility.validation.error_count(),
        });
    }
    let package = read_package(root)?;
    let rendered = render_artifact(&package, target);

    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(output) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(ArtifactError::AlreadyExists(output.to_path_buf()));
        }
        Err(source) => {
            return Err(ArtifactError::Write {
                path: output.to_path_buf(),
                source,
            });
        }
    };
    file.write_all(rendered.as_bytes())
        .and_then(|()| file.flush())
        .map_err(|source| ArtifactError::Write {
            path: output.to_path_buf(),
            source,
        })?;

    Ok(PreparedContinuationArtifact {
        path: output.to_path_buf(),
        target_provider: provider_name(target).to_owned(),
        media_type: "text/markdown; profile=rebinder.continuation.v1".to_owned(),
        sha256: sha256_hex(rendered.as_bytes()),
        bytes: rendered.len(),
        compatibility,
    })
}

fn capability(
    capability: &str,
    support: CapabilitySupport,
    details: &str,
) -> CapabilityDeclaration {
    CapabilityDeclaration {
        capability: capability.to_owned(),
        support,
        details: details.to_owned(),
    }
}

fn provider_name(target: Harness) -> &'static str {
    match target {
        Harness::Codex => "codex",
        Harness::Claude => "claude",
    }
}

fn read_package(root: &Path) -> Result<CanonicalPackage, CompatibilityError> {
    Ok(CanonicalPackage {
        manifest: read_json(root.join("manifest.json"))?,
        session: read_json(root.join("session.json"))?,
        conversation: read_conversation(root.join("conversation.jsonl"))?,
        task: read_json(root.join("task-state.json"))?,
        workspace: read_json(root.join("workspace-state.json"))?,
        repository: read_json(root.join("repository-state.json"))?,
        handoff: read_text(root.join("handoff.md"))?,
        provenance: read_json(root.join("provenance.json"))?,
    })
}

fn read_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, CompatibilityError> {
    let contents = read_text(path.clone())?;
    serde_json::from_str(&contents).map_err(|source| CompatibilityError::Decode { path, source })
}

fn read_conversation(path: PathBuf) -> Result<Vec<ConversationItem>, CompatibilityError> {
    let contents = read_text(path.clone())?;
    contents
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CompatibilityError::Decode { path, source })
}

fn read_text(path: PathBuf) -> Result<String, CompatibilityError> {
    fs::read_to_string(&path).map_err(|source| CompatibilityError::Read { path, source })
}

fn package_features(package: &CanonicalPackage) -> PackageFeatures {
    let mut features = PackageFeatures {
        environment_entries: package.workspace.environment.len(),
        workspace_files: package.workspace.files.len(),
        remote_urls: package
            .repository
            .repositories
            .iter()
            .flat_map(|repository| &repository.remotes)
            .filter(|remote| remote.url.is_some())
            .count(),
        patch_files: package
            .repository
            .repositories
            .iter()
            .filter(|repository| repository.patch_file.is_some())
            .count(),
        ..PackageFeatures::default()
    };
    for item in &package.conversation {
        *features.role_counts.entry(item.role).or_insert(0) += 1;
        for content in &item.content {
            match content.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = content.get("text").and_then(Value::as_str) {
                        let chars = text.chars().count();
                        features.visible_text_chars =
                            features.visible_text_chars.saturating_add(chars);
                        if chars > MESSAGE_MAX_CHARS {
                            features.truncated_messages += 1;
                        }
                    }
                }
                Some("tool_call") => features.tool_calls += 1,
                Some("tool_result") => features.tool_results += 1,
                Some("attachment") => features.attachments += 1,
                _ => {}
            }
        }
    }
    features
}

fn feature_findings(features: &PackageFeatures) -> Vec<CompatibilityFinding> {
    let mut findings = Vec::new();
    push_loss(
        &mut findings,
        features
            .role_counts
            .get(&ConversationRole::System)
            .copied()
            .unwrap_or(0),
        "conversation.system_roles_summarized",
        "conversation.jsonl",
        "system-role item(s) become annotated Markdown rather than target-native system messages",
    );
    push_loss(
        &mut findings,
        features.tool_calls,
        "conversation.tool_calls_summarized",
        "conversation.jsonl",
        "tool call(s) are represented only by aggregate execution metadata",
    );
    push_loss(
        &mut findings,
        features.tool_results,
        "conversation.tool_results_omitted",
        "conversation.jsonl",
        "tool result payload(s) are omitted from the continuation artifact",
    );
    push_loss(
        &mut findings,
        features.attachments,
        "conversation.attachments_omitted",
        "conversation.jsonl",
        "attachment reference(s) are omitted from the continuation artifact",
    );
    push_loss(
        &mut findings,
        features.environment_entries,
        "workspace.environment_omitted",
        "workspace-state.json/environment",
        "environment entry or redaction marker(s) are represented only by a count",
    );
    push_loss(
        &mut findings,
        features.workspace_files,
        "workspace.files_summarized",
        "workspace-state.json/files",
        "workspace file inventory item(s) are represented only by a count",
    );
    push_loss(
        &mut findings,
        features.remote_urls,
        "repository.remote_urls_omitted",
        "repository-state.json/repositories/remotes",
        "repository remote URL(s) are omitted from the continuation artifact",
    );
    push_loss(
        &mut findings,
        features.patch_files,
        "repository.patches_referenced",
        "repository-state.json/repositories/patchFile",
        "declared patch file(s) are referenced but not embedded or applied",
    );
    push_loss(
        &mut findings,
        features.truncated_messages,
        "conversation.messages_bounded",
        "conversation.jsonl",
        "visible message(s) exceed the per-message continuation budget and are head-tail bounded",
    );
    if features.visible_text_chars > RECENT_CONVERSATION_MAX_CHARS {
        findings.push(CompatibilityFinding {
            severity: CompatibilityFindingSeverity::InformationLoss,
            code: "conversation.history_bounded".to_owned(),
            path: Some("conversation.jsonl".to_owned()),
            message: format!(
                "visible conversation contains {} characters; only the latest {} characters fit the continuation budget",
                features.visible_text_chars, RECENT_CONVERSATION_MAX_CHARS
            ),
        });
    }
    findings
}

fn push_loss(
    findings: &mut Vec<CompatibilityFinding>,
    count: usize,
    code: &str,
    path: &str,
    message: &str,
) {
    if count == 0 {
        return;
    }
    findings.push(CompatibilityFinding {
        severity: CompatibilityFindingSeverity::InformationLoss,
        code: code.to_owned(),
        path: Some(path.to_owned()),
        message: format!("{count} {message}"),
    });
}

fn render_artifact(package: &CanonicalPackage, target: Harness) -> String {
    let features = package_features(package);
    let mut output = String::new();
    let _ = writeln!(output, "# Rebinder Continuation Artifact\n");
    let _ = writeln!(output, "- Target harness: `{}`", provider_name(target));
    let _ = writeln!(
        output,
        "- Source provider: `{}`",
        package.manifest.source.provider
    );
    let _ = writeln!(output, "- Schema: `{}`", package.manifest.schema_version);
    let _ = writeln!(
        output,
        "- Source session: `{}`",
        package
            .session
            .source_session_id
            .as_deref()
            .unwrap_or(&package.session.id)
    );
    let _ = writeln!(output, "- Updated: `{}`\n", package.session.updated_at);
    let _ = writeln!(
        output,
        "> Continue the recorded task from the verified state below. Re-check mutable repository and runtime state before changing files.\n"
    );

    let _ = writeln!(output, "## Handoff\n\n{}\n", package.handoff.trim());
    let _ = writeln!(output, "## Task\n");
    let _ = writeln!(output, "**Intent:** {}\n", package.task.intent);
    let _ = writeln!(output, "**Status:** `{}`\n", package.task.status.as_str());
    if !package.task.plan.is_empty() {
        let _ = writeln!(output, "### Plan\n");
        for step in &package.task.plan {
            let _ = writeln!(
                output,
                "- [{}] {}{}",
                plan_marker(step.status),
                step.description,
                step.notes
                    .as_deref()
                    .map_or_else(String::new, |notes| format!(" — {notes}"))
            );
        }
        output.push('\n');
    }
    write_list(
        &mut output,
        "Decisions",
        package.task.decisions.iter().map(|decision| {
            decision.rationale.as_deref().map_or_else(
                || decision.summary.clone(),
                |rationale| format!("{} — {}", decision.summary, rationale),
            )
        }),
    );
    write_list(
        &mut output,
        "Constraints",
        package.task.constraints.iter().cloned(),
    );
    write_list(
        &mut output,
        "Open questions",
        package.task.open_questions.iter().cloned(),
    );

    let _ = writeln!(output, "## Workspace\n");
    let _ = writeln!(output, "- Recorded cwd: `{}`", package.workspace.cwd);
    for root in &package.workspace.roots {
        let _ = writeln!(output, "- Root: `{}` ({})", root.path, root.kind);
    }
    let _ = writeln!(
        output,
        "- File inventory: {} item(s), summarized",
        features.workspace_files
    );
    let _ = writeln!(
        output,
        "- Environment: {} entry or redaction marker(s), values omitted\n",
        features.environment_entries
    );

    let _ = writeln!(output, "## Repository\n");
    if package.repository.repositories.is_empty() {
        output.push_str("No repository state was recorded.\n\n");
    }
    for repository in &package.repository.repositories {
        let branch = repository.head.branch.as_deref().unwrap_or("detached");
        let _ = writeln!(output, "### `{}`\n", repository.root);
        let _ = writeln!(
            output,
            "- HEAD: `{}` on `{}`",
            repository.head.commit, branch
        );
        let _ = writeln!(output, "- Detached: `{}`", repository.head.detached);
        for change in &repository.changes {
            let _ = writeln!(
                output,
                "- Change: `{}` — {}{}",
                change.path,
                change.status,
                if change.staged { " (staged)" } else { "" }
            );
        }
        if let Some(patch) = &repository.patch_file {
            let _ = writeln!(
                output,
                "- Declared patch: `{patch}` (reference only; not applied)"
            );
        }
        output.push('\n');
    }

    let _ = writeln!(output, "## Recent visible conversation\n");
    let recent = recent_visible_messages(&package.conversation);
    if recent.is_empty() {
        output.push_str("No visible conversation text was exported.\n\n");
    } else {
        for (role, text) in recent {
            let _ = writeln!(output, "### {}\n\n{}\n", role_heading(role), text.trim());
        }
    }

    let _ = writeln!(output, "## Portability notes\n");
    let _ = writeln!(output, "- Tool calls: {} summarized", features.tool_calls);
    let _ = writeln!(
        output,
        "- Tool result payloads: {} omitted",
        features.tool_results
    );
    let _ = writeln!(output, "- Attachments: {} omitted", features.attachments);
    let _ = writeln!(output, "- Remote URLs: {} omitted", features.remote_urls);
    let _ = writeln!(
        output,
        "- Patch files: {} referenced only\n",
        features.patch_files
    );

    let _ = writeln!(output, "## Provenance\n");
    let _ = writeln!(output, "- Exported: `{}`", package.provenance.exported_at);
    let _ = writeln!(
        output,
        "- Adapter: `{}`",
        package.provenance.source.adapter_version
    );
    let _ = writeln!(
        output,
        "- Transformations: {}",
        package.provenance.transformations.len()
    );
    let _ = writeln!(
        output,
        "- Redaction events: {}\n",
        package.provenance.redactions.len()
    );
    output
}

fn write_list(output: &mut String, heading: &str, values: impl Iterator<Item = String>) {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let _ = writeln!(output, "### {heading}\n");
    for value in values {
        let _ = writeln!(output, "- {value}");
    }
    output.push('\n');
}

fn plan_marker(status: crate::model::PlanStepStatus) -> &'static str {
    match status {
        crate::model::PlanStepStatus::Pending => " ",
        crate::model::PlanStepStatus::InProgress => "~",
        crate::model::PlanStepStatus::Completed => "x",
    }
}

fn recent_visible_messages(conversation: &[ConversationItem]) -> Vec<(ConversationRole, String)> {
    let mut selected = Vec::new();
    let mut used_chars = 0usize;
    for item in conversation.iter().rev() {
        let text = item
            .content
            .iter()
            .filter(|content| content.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|content| content.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            continue;
        }
        let text = bound_head_tail(&text, MESSAGE_MAX_CHARS);
        let chars = text.chars().count();
        if used_chars > 0 && used_chars.saturating_add(chars) > RECENT_CONVERSATION_MAX_CHARS {
            break;
        }
        used_chars = used_chars.saturating_add(chars);
        selected.push((item.role, text));
    }
    selected.reverse();
    selected
}

fn bound_head_tail(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_owned();
    }
    let marker = "\n\n[message bounded by Rebinder]\n\n";
    let marker_chars = marker.chars().count();
    let remaining = max_chars.saturating_sub(marker_chars);
    let head_chars = remaining / 2;
    let tail_chars = remaining.saturating_sub(head_chars);
    let head = value.chars().take(head_chars).collect::<String>();
    let tail = value
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{head}{marker}{tail}")
}

fn role_heading(role: ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "System note",
        ConversationRole::User => "User",
        ConversationRole::Assistant => "Assistant",
        ConversationRole::Tool => "Tool",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_package() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal-session")
    }

    #[test]
    fn reports_used_lossy_capabilities_without_blocking_continuation() {
        let report = assess_package_compatibility(example_package(), Harness::Claude)
            .expect("assess example package");

        assert!(report.can_continue);
        assert_eq!(report.level, CompatibilityLevel::CompatibleWithLoss);
        assert_eq!(report.source_provider.as_deref(), Some("codex"));
        assert_eq!(report.target.provider, "claude");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "conversation.tool_results_omitted")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "workspace.environment_omitted")
        );
    }

    #[test]
    fn prepares_a_private_bounded_artifact_without_tool_payloads() {
        let fixture = tempfile::tempdir().expect("artifact fixture");
        let output = fixture.path().join("continuation.md");
        let prepared = prepare_continuation_artifact(example_package(), Harness::Claude, &output)
            .expect("prepare artifact");

        let contents = fs::read_to_string(&output).expect("read artifact");
        assert!(contents.contains("Build the provider-neutral validation core."));
        assert!(contents.contains("Build the package validation core."));
        assert!(contents.contains("The validation boundary is ready to implement."));
        assert!(!contents.contains("Initial Architecture"));
        assert_eq!(prepared.target_provider, "claude");
        assert_eq!(prepared.sha256.len(), 64);
        assert_eq!(prepared.bytes, contents.len());
        assert!(matches!(
            prepare_continuation_artifact(example_package(), Harness::Claude, &output),
            Err(ArtifactError::AlreadyExists(path)) if path == output
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&output)
                    .expect("artifact metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
