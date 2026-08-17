use std::{
    ffi::OsString,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use dialoguer::{Select, theme::ColorfulTheme};
use rebinder::{
    ClaudeSession, ClaudeTransferStrategy, Harness, Inspection, ValidationReport,
    discover_claude_sessions, inspect_package, launch_prepared_codex_session,
    prepare_claude_to_codex_with_strategy, run_harness, validate_package,
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
            eprintln!(
                "error: {} to {} transfer is not implemented yet",
                Harness::from(arguments.from).executable(),
                Harness::from(arguments.to).executable(),
            );
            ExitCode::from(2)
        }
        Command::Sessions(arguments) => list_sessions(&arguments),
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
        eprintln!("rebinder: bounded {size} source history before native Codex injection");
    }
    eprintln!("rebinder: resuming in {}", prepared.cwd.display());

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
    if arguments.harness != HarnessArgument::Claude {
        eprintln!("error: only Claude Code session discovery is implemented");
        return ExitCode::from(2);
    }
    match discover_claude_sessions() {
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
    }
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
