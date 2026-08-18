//! Session transfer and provider-neutral package primitives for Rebinder.

pub mod compatibility;
pub mod continuity;
pub mod export;
mod handoff;
pub mod harness;
pub mod inspection;
pub mod model;
pub mod reverse;
pub mod transfer;
pub mod validation;
pub mod worktree;

pub use compatibility::{
    ArtifactError, CapabilityDeclaration, CapabilitySupport, CompatibilityError,
    CompatibilityFinding, CompatibilityFindingSeverity, CompatibilityLevel, CompatibilityReport,
    PreparedContinuationArtifact, ProviderCapabilities, assess_package_compatibility,
    prepare_continuation_artifact, provider_capabilities,
};
pub use continuity::{
    ContinuityError, ContinuityInstallation, ContinuityObservation, ContinuityOffer,
    ContinuityOfferReason, ContinuityOfferState, ContinuityOfferStatus, ContinuityStatus,
    DEFAULT_FIVE_HOUR_THRESHOLD, DEFAULT_SEVEN_DAY_THRESHOLD, LimitWindow, StatusLineRender,
    accept_continuity_offer, accepted_continuity_offer, accepted_offer_for_launch,
    claude_hook_output, continuity_status, decline_continuity_offer, disable_claude_continuity,
    enable_claude_continuity, mark_continuity_offer_completed, new_continuity_launch_id,
    process_claude_statusline,
};
pub use export::{
    ExportError, ExportableSession, ExportedPackage, discover_exportable_sessions, export_session,
};
pub use harness::{Harness, HarnessLaunchError, run_harness, run_harness_with_environment};
pub use inspection::{Inspection, PackageSummary, inspect_package};
pub use reverse::{
    ClaudeContinuationState, PreparedClaudeSession, ReverseTransferError,
    launch_prepared_claude_session, prepare_codex_to_claude, prepare_codex_to_claude_with_recovery,
};
pub use transfer::{
    ClaudeSession, ClaudeSessionState, ClaudeTransferStrategy, PreparedCodexSession, TransferError,
    discover_claude_sessions, launch_prepared_codex_session, prepare_claude_to_codex,
    prepare_claude_to_codex_with_strategy, prepare_claude_to_codex_with_strategy_and_recovery,
};
pub use validation::{ValidationIssue, ValidationReport, validate_package};
pub use worktree::{
    RecoveredWorktree, WorktreeRecovery, WorktreeRecoveryError, recover_registered_worktree,
};
