//! Session transfer and provider-neutral package primitives for Rebinder.

pub mod harness;
pub mod inspection;
pub mod model;
pub mod transfer;
pub mod validation;

pub use harness::{Harness, HarnessLaunchError, run_harness};
pub use inspection::{Inspection, PackageSummary, inspect_package};
pub use transfer::{
    ClaudeSession, ClaudeSessionState, PreparedCodexSession, TransferError,
    discover_claude_sessions, launch_prepared_codex_session, prepare_claude_to_codex,
};
pub use validation::{ValidationIssue, ValidationReport, validate_package};
