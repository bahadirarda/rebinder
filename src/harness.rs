use std::{ffi::OsStr, io, process::ExitStatus};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Codex,
    Claude,
}

impl Harness {
    pub const fn executable(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

#[derive(Debug, Error)]
#[error("cannot launch {harness}: {source}")]
pub struct HarnessLaunchError {
    harness: &'static str,
    #[source]
    source: io::Error,
}

/// Run a harness command with inherited stdin, stdout, and stderr.
///
/// Arguments are passed through without parsing so interactive commands and
/// harness-specific flags retain their native behavior.
pub fn run_harness<I, S>(harness: Harness, arguments: I) -> Result<ExitStatus, HarnessLaunchError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    std::process::Command::new(harness.executable())
        .args(arguments)
        .status()
        .map_err(|source| HarnessLaunchError {
            harness: harness.executable(),
            source,
        })
}

/// Run a harness command with inherited streams and additional environment entries.
pub fn run_harness_with_environment<I, S, E, K, V>(
    harness: Harness,
    arguments: I,
    environment: E,
) -> Result<ExitStatus, HarnessLaunchError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    E: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    std::process::Command::new(harness.executable())
        .args(arguments)
        .envs(environment)
        .status()
        .map_err(|source| HarnessLaunchError {
            harness: harness.executable(),
            source,
        })
}
