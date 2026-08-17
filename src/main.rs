use std::{ffi::OsString, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand, ValueEnum};
use rebinder::{
    Harness, Inspection, ValidationReport, inspect_package, run_harness, validate_package,
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
    /// Provider-scoped source session ID; omit to let the adapter discover one.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,
    /// Arguments passed to the target harness after migration.
    #[arg(last = true, value_name = "TARGET_ARGS", allow_hyphen_values = true)]
    target_arguments: Vec<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum HarnessArgument {
    Codex,
    Claude,
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
            } else {
                let session = arguments.session_id.as_deref().unwrap_or("<auto-discover>");
                let target_argument_count = arguments.target_arguments.len();
                eprintln!(
                    "error: cross-harness transfer is not implemented yet ({} session {session} -> {}, {target_argument_count} target argument(s))",
                    Harness::from(arguments.from).executable(),
                    Harness::from(arguments.to).executable(),
                );
            }
            ExitCode::from(2)
        }
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
