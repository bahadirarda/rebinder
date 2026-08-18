---
okf_version: 0.2
---

# Rebinder

Cross-harness session continuity for coding agents.

Rebinder rebinds an agent session's portable state to a different coding agent harness.

# Documents

- [Project Charter](docs/product/project-charter.md)
- [Product Requirements](docs/product/product-requirements.md)
- [MVP Status](docs/product/mvp-status.md)
- [Terminology](docs/terminology.md)
- [Architecture](docs/architecture/architecture.md)
- [Interchange Format 0.1.0](docs/format/interchange-format-0.1.md)
- [OKF 0.2 Project Profile](docs/standards/okf-0.2-project-profile.md)
- [Release System](docs/governance/release-system.md)
- [GitHub Repository Setup](docs/governance/github-setup.md)
- [Website Delivery](docs/governance/website-delivery.md)
- [Cut a Release](docs/workflows/cut-release.md)
- [ADR-0001: Canonical session model](docs/decisions/ADR-0001-canonical-session-model.md)
- [ADR-0002: Rust implementation](docs/decisions/ADR-0002-rust-implementation.md)
- [ADR-0003: CLI command model](docs/decisions/ADR-0003-cli-command-model.md)
- [ADR-0004: Calendar releases and native distribution](docs/decisions/ADR-0004-calendar-releases.md)
- [ADR-0005: Codex native import bridge](docs/decisions/ADR-0005-codex-native-import-bridge.md)
- [ADR-0006: Capability-aware continuation artifacts](docs/decisions/ADR-0006-capability-aware-continuation-artifacts.md)
- [ADR-0007: Non-resuming canonical provider export](docs/decisions/ADR-0007-non-resuming-canonical-export.md)

## Status

The project has an operational one-way transfer MVP. Local Claude Code sessions
can be selected interactively, imported into native Codex threads, and resumed
in their recorded workspace directly through Rebinder. Large transcripts use a
bounded, role-preserving context-safe handoff by default, and meaningful repeat
updates are compacted natively before a visible, history-grounded continuation
brief is created. Interchange schema `0.1.0`, Rust package validation and
inspection, verified native distribution, command passthrough, and the product
site are also implemented. Target capability declarations, package-specific
compatibility reports, and bounded provider-neutral continuation artifacts are
available. Claude and Codex source sessions can be exported into self-validated
canonical packages without resuming or mutating them. Codex-to-Claude native
transfer and missing-worktree reconstruction remain pending.
