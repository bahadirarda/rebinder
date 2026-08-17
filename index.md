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

## Status

The project is a foundation MVP. Interchange schema `0.1.0`, the Rust
validation and inspection core, verified native distribution pipeline, and
harness command passthrough are implemented. The dependency-free product site
is delivered through protected GitHub Pages automation. Provider adapters and
operational cross-harness transfer remain pending.
