use std::{
    ffi::OsString,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use dialoguer::{Select, theme::ColorfulTheme};
use rebinder::{
    CapabilitySupport, ClaudeContinuationState, ClaudeSession, ClaudeTransferStrategy,
    CompatibilityFindingSeverity, CompatibilityReport, ExportableSession, Harness, Inspection,
    ProviderCapabilities, ValidationReport, assess_package_compatibility, discover_claude_sessions,
    discover_exportable_sessions, export_session, inspect_package, launch_prepared_claude_session,
    launch_prepared_codex_session, prepare_claude_to_codex_with_strategy, prepare_codex_to_claude,
    prepare_continuation_artifact, provider_capabilities, run_harness, validate_package,
};

#[derive(Debug, Parser)]
#[command(
    name = "rebinder",
    version,
    about = "Cross-harness session continuity for coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a Codex CLI command through Rebinder.
    Codex(PassthroughArgs),
    /// Run a Claude CLI command through Rebinder.
    Claude(PassthroughArgs),
    /// Transfer a session to a different harness.
    Transfer(TransferArgs),
    /// List sessions available to a provider adapter.
    Sessions(SessionsArgs),
    /// Export a provider session into a new canonical Rebinder package.
    Export(ExportArgs),
    /// Show the continuation capabilities declared by a target adapter.
    Capabilities {
        /// Target harness whose adapter capabilities should be reported.
        #[arg(value_enum)]
        harness: HarnessArgument,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Report information preserved or lost when a package continues in a target harness.
    Compatibility(CompatibilityArgs),
    /// Produce a bounded provider-neutral continuation artifact from a valid package.
    Artifact(ArtifactArgs),
    /// Validate the structure and integrity of a session package.
    Validate {
        /// Path to an unpacked Rebinder session package.
        package: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect portable session state without starting an agent.
    Inspect {
        /// Path to an unpacked Rebinder session package.
        package: PathBuf,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
#[command(disable_help_flag = true, disable_version_flag = true)]
struct PassthroughArgs {
    /// Arguments passed unchanged to the selected harness.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    arguments: Vec<OsString>,
}

#[derive(Debug, Args)]
struct TransferArgs {
    /// Harness that owns the source session.
    #[arg(short = 'f', long, value_enum)]
    from: HarnessArgument,
    /// Harness in which the session should continue.
    #[arg(short = 't', long, value_enum)]
    to: HarnessArgument,
    /// Provider-scoped source session ID; omit for an interactive picker in a terminal.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,
    /// Transfer policy: choose automatically, import the full transcript, or use a bounded handoff.
    #[arg(long, value_enum, default_value_t = TransferStrategyArgument::Auto)]
    strategy: TransferStrategyArgument,
    /// Arguments passed to the target harness after migration.
    #[arg(last = true, value_name = "TARGET_ARGS", allow_hyphen_values = true)]
    target_arguments: Vec<OsString>,
}

#[derive(Debug, Args)]
struct SessionsArgs {
    /// Harness whose sessions should be discovered.
    #[arg(value_enum)]
    harness: HarnessArgument,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ExportArgs {
    /// Harness that owns the source session.
    #[arg(short = 'f', long, value_enum)]
    from: HarnessArgument,
    /// Provider-scoped source session ID; omit for a picker or current-workspace selection.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,
    /// New package directory; existing paths are never overwritten.
    #[arg(short, long)]
    output: PathBuf,
    /// Emit machine-readable result metadata.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CompatibilityArgs {
    /// Path to an unpacked Rebinder session package.
    package: PathBuf,
    /// Target harness that will consume the continuation state.
    #[arg(short = 't', long, value_enum)]
    to: HarnessArgument,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ArtifactArgs {
    /// Path to an unpacked Rebinder session package.
    package: PathBuf,
    /// Target harness that will consume the artifact.
    #[arg(short = 't', long, value_enum)]
    to: HarnessArgument,
    /// New artifact path; existing files are never overwritten.
    #[arg(short, long)]
    output: PathBuf,
    /// Emit machine-readable result metadata.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum HarnessArgument {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TransferStrategyArgument {
    Auto,
    Full,
    Handoff,
}

impl From<TransferStrategyArgument> for ClaudeTransferStrategy {
    fn from(value: TransferStrategyArgument) -> Self {
        match value {
            TransferStrategyArgument::Auto => Self::Auto,
            TransferStrategyArgument::Full => Self::Full,
            TransferStrategyArgument::Handoff => Self::Handoff,
        }
    }
}

impl From<HarnessArgument> for Harness {
    fn from(value: HarnessArgument) -> Self {
        match value {
            HarnessArgument::Codex => Self::Codex,
            HarnessArgument::Claude => Self::Claude,
        }
    }
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Codex(arguments) => launch_harness(Harness::Codex, arguments.arguments),
        Command::Claude(arguments) => launch_harness(Harness::Claude, arguments.arguments),
        Command::Transfer(arguments) => {
            if arguments.from == arguments.to {
                eprintln!(
                    "error: source and target harness are the same; use `rebinder {} resume ...` for a native resume",
                    Harness::from(arguments.to).executable()
                );
                return ExitCode::from(2);
            }
            if arguments.from == HarnessArgument::Claude && arguments.to == HarnessArgument::Codex {
                return transfer_claude_to_codex(arguments);
            }
            transfer_codex_to_claude(arguments)
        }
        Command::Sessions(arguments) => list_sessions(&arguments),
        Command::Export(arguments) => export_canonical(arguments),
        Command::Capabilities { harness, json } => {
            let capabilities = provider_capabilities(harness.into());
            if json {
                print_json(&capabilities);
            } else {
                print_capabilities(&capabilities);
            }
            ExitCode::SUCCESS
        }
        Command::Compatibility(arguments) => compatibility(&arguments),
        Command::Artifact(arguments) => artifact(&arguments),
        Command::Validate { package, json } => {
            let report = validate_package(package);
            if json {
                print_json(&report);
            } else {
                print_validation(&report);
            }
            validity_exit_code(report.valid)
        }
        Command::Inspect { package, json } => match inspect_package(package) {
            Ok(inspection) => {
                let valid = inspection.validation.valid;
                if json {
                    print_json(&inspection);
                } else {
                    print_inspection(&inspection);
                }
                validity_exit_code(valid)
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(2)
            }
        },
    }
}

fn transfer_claude_to_codex(arguments: TransferArgs) -> ExitCode {
    let mut session_id = arguments.session_id;
    if session_id.is_none() && io::stdin().is_terminal() && io::stderr().is_terminal() {
        session_id = match pick_claude_session() {
            Ok(Some(session_id)) => Some(session_id),
            Ok(None) => {
                eprintln!("rebinder: transfer cancelled");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(2);
            }
        };
    }

    let prepared = match prepare_claude_to_codex_with_strategy(
        session_id.as_deref(),
        arguments.strategy.into(),
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    match (prepared.strategy, prepared.imported) {
        (ClaudeTransferStrategy::Handoff, true) => eprintln!(
            "rebinder: created or updated context-safe Codex thread {} for Claude Code session {}",
            prepared.codex_thread_id, prepared.source_session_id
        ),
        (ClaudeTransferStrategy::Handoff, false) => eprintln!(
            "rebinder: reusing the context-safe Codex thread {} for Claude Code session {}",
            prepared.codex_thread_id, prepared.source_session_id
        ),
        (_, true) => eprintln!(
            "rebinder: imported Claude Code session {} as Codex thread {}",
            prepared.source_session_id, prepared.codex_thread_id
        ),
        (_, false) => eprintln!(
            "rebinder: Claude Code session {} is already bound to Codex thread {}",
            prepared.source_session_id, prepared.codex_thread_id
        ),
    }
    if prepared.strategy == ClaudeTransferStrategy::Handoff {
        let size = prepared
            .source_size_bytes
            .map_or_else(|| "unknown size".to_owned(), human_bytes);
        eprintln!("rebinder: bounded {size} source history and preserved conversation roles");
        if prepared.compacted {
            eprintln!("rebinder: compacted the updated handoff before opening Codex");
        }
        if prepared.activated {
            eprintln!(
                "rebinder: activated the transferred context with a visible continuation brief"
            );
        }
    }
    eprintln!("rebinder: opening Codex in {}", prepared.cwd.display());

    match launch_prepared_codex_session(&prepared, arguments.target_arguments) {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .map_or_else(|| ExitCode::from(1), ExitCode::from),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(127)
        }
    }
}

fn transfer_codex_to_claude(arguments: TransferArgs) -> ExitCode {
    if arguments.strategy != TransferStrategyArgument::Auto {
        eprintln!(
            "error: --strategy applies only to Claude-to-Codex transfer; Codex-to-Claude always uses a bounded canonical artifact"
        );
        return ExitCode::from(2);
    }
    let session_id = match resolve_export_session(Harness::Codex, arguments.session_id) {
        Ok(Some(session_id)) => session_id,
        Ok(None) => {
            eprintln!("rebinder: transfer cancelled");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    let prepared = match prepare_codex_to_claude(&session_id) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    match prepared.state {
        ClaudeContinuationState::New => eprintln!(
            "rebinder: prepared Codex session {} as new Claude session {}",
            prepared.source_session_id, prepared.claude_session_id
        ),
        ClaudeContinuationState::Updated => eprintln!(
            "rebinder: prepared a new canonical revision for Claude session {}",
            prepared.claude_session_id
        ),
        ClaudeContinuationState::Unchanged => eprintln!(
            "rebinder: canonical revision is already active in Claude session {}",
            prepared.claude_session_id
        ),
    }
    eprintln!(
        "rebinder: compatibility {} with {} declared information-loss finding(s)",
        prepared.compatibility.level.as_str(),
        prepared.compatibility.findings.len()
    );
    eprintln!(
        "rebinder: opening Claude Code in {}",
        prepared.cwd.display()
    );
    match launch_prepared_claude_session(&prepared, arguments.target_arguments) {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .map_or_else(|| ExitCode::from(1), ExitCode::from),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(127)
        }
    }
}

fn pick_claude_session() -> Result<Option<String>, String> {
    let sessions = discover_claude_sessions().map_err(|error| error.to_string())?;
    if sessions.is_empty() {
        return Err("no importable Claude Code sessions were found".to_owned());
    }
    let labels = sessions.iter().map(session_label).collect::<Vec<_>>();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Claude Code session to continue in Codex")
        .items(&labels)
        .default(0)
        .interact_opt()
        .map_err(|error| format!("cannot read session selection: {error}"))?;
    Ok(selection.map(|index| sessions[index].id.clone()))
}

fn session_label(session: &ClaudeSession) -> String {
    let state = match session.state {
        rebinder::ClaudeSessionState::ReadyToImport => "ready",
        rebinder::ClaudeSessionState::Imported => "imported",
    };
    let strategy = match session.recommended_strategy {
        ClaudeTransferStrategy::Handoff => "context-safe handoff",
        _ => "full import",
    };
    let size = session
        .source_size_bytes
        .map_or_else(|| "unknown size".to_owned(), human_bytes);
    let title = truncate(&single_line(&session.title), 58);
    let workspace = if session.cwd.is_dir() {
        truncate(&session.cwd.display().to_string(), 72)
    } else {
        format!(
            "{} (missing)",
            truncate(&session.cwd.display().to_string(), 62)
        )
    };
    format!("[{state}] {title} — {workspace} — {size}, {strategy}")
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let kept = max_chars.saturating_sub(1);
    format!("{}…", value.chars().take(kept).collect::<String>())
}

fn human_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        human_unit(bytes, MIB, "MiB")
    } else if bytes >= KIB {
        human_unit(bytes, KIB, "KiB")
    } else {
        format!("{bytes} B")
    }
}

fn human_unit(bytes: u64, unit: u64, suffix: &str) -> String {
    let whole = bytes / unit;
    let decimal = bytes % unit * 10 / unit;
    format!("{whole}.{decimal} {suffix}")
}

fn list_sessions(arguments: &SessionsArgs) -> ExitCode {
    match arguments.harness {
        HarnessArgument::Claude => match discover_claude_sessions() {
            Ok(sessions) => {
                if arguments.json {
                    print_json(&sessions);
                } else {
                    print_claude_sessions(&sessions);
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(2)
            }
        },
        HarnessArgument::Codex => match discover_exportable_sessions(Harness::Codex) {
            Ok(sessions) => {
                if arguments.json {
                    print_json(&sessions);
                } else {
                    print_exportable_sessions(&sessions);
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(2)
            }
        },
    }
}

fn export_canonical(arguments: ExportArgs) -> ExitCode {
    let harness = Harness::from(arguments.from);
    let session_id = match resolve_export_session(harness, arguments.session_id) {
        Ok(Some(session_id)) => session_id,
        Ok(None) => {
            eprintln!("rebinder: export cancelled");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    match export_session(harness, &session_id, &arguments.output) {
        Ok(exported) => {
            if arguments.json {
                print_json(&exported);
            } else {
                println!(
                    "exported {} session {} to {}",
                    exported.source_provider,
                    exported.source_session_id,
                    exported.path.display()
                );
                println!(
                    "{} conversation item(s), {} redacted value(s), package valid",
                    exported.conversation_items, exported.redacted_values
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn resolve_export_session(
    harness: Harness,
    explicit: Option<String>,
) -> Result<Option<String>, String> {
    if explicit.is_some() {
        return Ok(explicit);
    }
    let sessions = discover_exportable_sessions(harness).map_err(|error| error.to_string())?;
    if sessions.is_empty() {
        return Err(format!(
            "no exportable {} sessions were found",
            harness.executable()
        ));
    }
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let labels = sessions
            .iter()
            .map(exportable_session_label)
            .collect::<Vec<_>>();
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Select a {} session to export",
                harness.executable()
            ))
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|error| format!("cannot read session selection: {error}"))?;
        return Ok(selection.map(|index| sessions[index].id.clone()));
    }
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("cannot determine current directory: {error}"))?;
    sessions
        .into_iter()
        .find(|session| paths_equivalent_for_cli(&session.cwd, &current_dir))
        .map(|session| Some(session.id))
        .ok_or_else(|| {
            format!(
                "no {} session matches the current directory; pass an ID explicitly",
                harness.executable()
            )
        })
}

fn paths_equivalent_for_cli(left: &std::path::Path, right: &std::path::Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(left, right)| left == right)
}

fn print_exportable_sessions(sessions: &[ExportableSession]) {
    if sessions.is_empty() {
        println!("no exportable sessions found");
        return;
    }
    for session in sessions {
        println!(
            "{}\t{}\t{}\t{}",
            session.id,
            single_line(&session.title),
            session.cwd.display(),
            session.updated_at
        );
    }
}

fn exportable_session_label(session: &ExportableSession) -> String {
    let title = truncate(&single_line(&session.title), 62);
    let workspace = if session.cwd.is_dir() {
        truncate(&session.cwd.display().to_string(), 72)
    } else {
        format!(
            "{} (missing)",
            truncate(&session.cwd.display().to_string(), 62)
        )
    };
    let size = session
        .source_size_bytes
        .map(human_bytes)
        .map(|size| format!(" — {size}"))
        .unwrap_or_default();
    format!("{title} — {workspace} — {}{size}", session.updated_at)
}

fn print_claude_sessions(sessions: &[ClaudeSession]) {
    if sessions.is_empty() {
        println!("no importable or previously imported Claude Code sessions found");
        return;
    }
    for session in sessions {
        let state = match session.state {
            rebinder::ClaudeSessionState::ReadyToImport => "ready",
            rebinder::ClaudeSessionState::Imported => "imported",
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            session.id,
            state,
            single_line(&session.title),
            session.cwd.display(),
            session
                .source_size_bytes
                .map_or_else(|| "unknown".to_owned(), human_bytes),
            match session.recommended_strategy {
                ClaudeTransferStrategy::Handoff => "handoff",
                _ => "full",
            }
        );
    }
}

fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        })
        .collect()
}

fn compatibility(arguments: &CompatibilityArgs) -> ExitCode {
    match assess_package_compatibility(&arguments.package, arguments.to.into()) {
        Ok(report) => {
            let can_continue = report.can_continue;
            if arguments.json {
                print_json(&report);
            } else {
                print_compatibility(&report);
            }
            validity_exit_code(can_continue)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn artifact(arguments: &ArtifactArgs) -> ExitCode {
    match prepare_continuation_artifact(&arguments.package, arguments.to.into(), &arguments.output)
    {
        Ok(prepared) => {
            if arguments.json {
                print_json(&prepared);
            } else {
                println!(
                    "wrote {} continuation artifact to {} ({} bytes, sha256 {})",
                    prepared.target_provider,
                    prepared.path.display(),
                    prepared.bytes,
                    prepared.sha256
                );
                println!(
                    "compatibility: {} with {} declared information-loss finding(s)",
                    prepared.compatibility.level.as_str(),
                    prepared.compatibility.findings.len()
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn launch_harness(harness: Harness, arguments: Vec<OsString>) -> ExitCode {
    match run_harness(harness, arguments) {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .map_or_else(|| ExitCode::from(1), ExitCode::from),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(127)
        }
    }
}

fn print_json(value: &impl serde::Serialize) {
    match serde_json::to_string_pretty(value) {
        Ok(output) => println!("{output}"),
        Err(error) => eprintln!("error: cannot encode output: {error}"),
    }
}

fn print_validation(report: &ValidationReport) {
    if report.valid {
        println!(
            "valid Rebinder package (schema {})",
            report.schema_version.as_deref().unwrap_or("unknown")
        );
    } else {
        println!(
            "invalid Rebinder package: {} error(s), {} warning(s)",
            report.error_count(),
            report.warning_count()
        );
    }

    for issue in &report.issues {
        let severity = match issue.severity {
            rebinder::validation::IssueSeverity::Error => "error",
            rebinder::validation::IssueSeverity::Warning => "warning",
        };
        let path = issue
            .path
            .as_deref()
            .map(|path| format!(" [{path}]"))
            .unwrap_or_default();
        println!("{severity} {}{path}: {}", issue.code, issue.message);
    }
}

fn print_inspection(inspection: &Inspection) {
    let Some(summary) = &inspection.summary else {
        print_validation(&inspection.validation);
        return;
    };

    println!("Package:      {}", summary.package);
    println!("Schema:       {}", summary.schema_version);
    println!(
        "Source:       {} (adapter {})",
        summary.source.provider, summary.source.adapter_version
    );
    println!("Session:      {}", summary.session.title);
    println!("Updated:      {}", summary.session.updated_at);
    println!(
        "Conversation: {} item(s) ({})",
        summary.conversation.item_count,
        summary
            .conversation
            .roles
            .iter()
            .map(|(role, count)| format!("{role}: {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "Task:         {} ({}/{}, {} open question(s))",
        summary.task.status,
        summary.task.completed_steps,
        summary.task.plan_steps,
        summary.task.open_questions
    );
    println!("Intent:       {}", summary.task.intent);
    println!(
        "Workspace:    {} root(s), {} file(s), cwd {}",
        summary.workspace.roots, summary.workspace.files, summary.workspace.cwd
    );
    println!(
        "Repository:   {} repo(s), {} change(s)",
        summary.repository.repositories, summary.repository.changes
    );
    println!(
        "Provenance:   exported {}, {} transformation(s), {} redacted value(s)",
        summary.provenance.exported_at,
        summary.provenance.transformations,
        summary.provenance.redacted_values
    );
}

fn print_capabilities(capabilities: &ProviderCapabilities) {
    println!(
        "{} target adapter {} ({})",
        capabilities.provider, capabilities.adapter_version, capabilities.artifact_format
    );
    for capability in &capabilities.capabilities {
        println!(
            "{}\t{}\t{}",
            capability.capability,
            capability_support(capability.support),
            capability.details
        );
    }
}

fn print_compatibility(report: &CompatibilityReport) {
    println!(
        "compatibility: {} ({} -> {})",
        report.level.as_str(),
        report
            .source_provider
            .as_deref()
            .unwrap_or("invalid package"),
        report.target.provider
    );
    if report.findings.is_empty() {
        println!("no active information-loss findings");
        return;
    }
    for finding in &report.findings {
        let severity = match finding.severity {
            CompatibilityFindingSeverity::InformationLoss => "information_loss",
            CompatibilityFindingSeverity::Blocking => "blocking",
        };
        let path = finding
            .path
            .as_deref()
            .map(|path| format!(" [{path}]"))
            .unwrap_or_default();
        println!("{severity} {}{path}: {}", finding.code, finding.message);
    }
}

fn capability_support(support: CapabilitySupport) -> &'static str {
    match support {
        CapabilitySupport::Preserved => "preserved",
        CapabilitySupport::Summarized => "summarized",
        CapabilitySupport::Omitted => "omitted",
    }
}

fn validity_exit_code(valid: bool) -> ExitCode {
    if valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_label_surfaces_state_size_workspace_and_strategy() {
        let workspace = tempfile::tempdir().expect("workspace");
        let session = ClaudeSession {
            id: "session-1".to_owned(),
            title: "A title\nwith control characters".to_owned(),
            cwd: workspace.path().to_path_buf(),
            source_path: PathBuf::from("hidden.jsonl"),
            updated_at_unix_seconds: None,
            source_size_bytes: Some(46 * 1024 * 1024),
            recommended_strategy: ClaudeTransferStrategy::Handoff,
            state: rebinder::ClaudeSessionState::Imported,
            codex_thread_id: Some("thread-1".to_owned()),
        };

        let label = session_label(&session);
        assert!(label.contains("[imported]"));
        assert!(label.contains("46.0 MiB"));
        assert!(label.contains("context-safe handoff"));
        assert!(label.contains(&workspace.path().display().to_string()));
        assert!(!label.contains('\n'));
    }

    #[test]
    fn truncation_keeps_unicode_boundaries() {
        assert_eq!(truncate("şğüçö", 4), "şğü…");
    }
}
