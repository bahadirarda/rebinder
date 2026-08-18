---
type: product-status
title: Transfer MVP Status
status: draft
version: 0.1.0
---

# Transfer MVP Status

## Purpose

This document distinguishes working product behavior from reserved contracts.
README examples and release notes MUST preserve this distinction.

## Implemented

- Rust CLI and library crate
- Codex and Claude native command passthrough namespaces
- Canonical interchange schemas at schema version `0.1.0`
- Directory package validation and human/JSON inspection
- SHA-256 file inventory verification
- Relative-path confinement and symlink rejection
- Conversation graph and provenance invariants
- Claude Code session discovery through the Codex external-agent API
- Exact Claude session selection and current-workspace latest-session selection
- Claude-to-Codex native full import and bounded native-thread handoff paths
- Role-preserving handoff injection and semantic checkpoint revisions
- Visible, history-grounded continuation briefs for bounded handoffs
- Retry-safe injection/compaction/activation ledger and native repeat-update compaction
- Rebinder-owned Codex opening in the Claude session's recorded workspace or worktree
- Fail-closed handling for missing workspaces and unsupported directions
- Human and JSON session inventory output
- Codex and Claude target capability declarations
- Package-specific human and JSON compatibility reports
- Bounded provider-neutral Markdown continuation artifacts
- Calendar release identity `0.YYYYMMDD.REVISION`
- Changesets release-intent ledger and automated version pull requests
- Checksum-verifying Unix and Windows installers
- Five-target native GitHub Release workflow
- Release metadata, checksum manifest, and GitHub artifact attestations

## Operational transfer path

`rebinder transfer [session] --from claude --to codex [--strategy
auto|full|handoff] -- [target args]` is
operational for local Claude Code sessions detected by the installed Codex
app-server. Rebinder selects only the `SESSIONS` migration item, resolves a
native Codex thread ID through the full-import or bounded-handoff path, verifies
that the recorded workspace exists, and opens Codex on the bound thread in that
directory. The user stays in the `rebinder transfer` workflow and does not run
a separate Codex resume command.

Omitting the session ID in an interactive terminal opens an arrow-key session
picker. Escape cancels without importing. Non-interactive omission selects only
the newest session whose recorded working directory matches the current
directory. Explicit selection uses the provider session ID shown by `rebinder
sessions claude`.

The automatic strategy fully imports sources up to 512 KiB. Larger sources use
an append-only, context-safe handoff built from the latest compact summary and
bounded recent visible messages. Injection retains user and assistant roles.
Semantic revisions ignore unrelated source metadata changes; meaningful repeat
updates are compacted through Codex before Rebinder opens the existing thread.
For every new semantic revision, Rebinder starts one read-only Codex turn that
creates a visible continuation brief from the injected context. The turn cannot
write through its read-only sandbox and is instructed not to call tools. It
consumes normal Codex model tokens and is recovered rather than duplicated
after an interrupted ledger write. This keeps prior oversized imports intact
while creating or reusing a separate native Codex thread.

The role-preserving handoff format does not append onto a legacy flattened
handoff thread. Rebinder leaves that thread intact, creates one clean bounded
thread during the upgrade, and reuses the new binding afterward.

Rebinder never edits Claude transcript files or writes directly into Codex's
private session store. It stores bounded handoff input and its retry-safe
binding in its own platform data directory; Codex owns native thread creation,
history injection, activation, compaction, full-import conversion, and session
persistence.

## Operational package portability path

`rebinder capabilities <harness>` publishes the target adapter's preserved,
summarized, and omitted continuation capabilities. `rebinder compatibility
<package> --to <harness>` validates the package and reports only the
information-loss findings activated by its contents. Invalid structure is
blocking; declared information loss remains explicit but does not prevent
artifact preparation.

`rebinder artifact <package> --to <harness> --output <path>` creates a bounded
Markdown continuation artifact containing handoff guidance, task state,
workspace and repository facts, provenance, and recent visible conversation.
Tool-result payloads, attachments, environment values, and remote URLs are
omitted. Existing output paths are never overwritten and Unix output is private
to the current user.

## Pending MVP capabilities

- Codex session discovery and canonical export
- Claude Code canonical-package export independent of Codex
- End-to-end Codex-to-Claude transfer
- Missing-worktree reconstruction

No pending capability may be described as available in installation or release
documentation before its acceptance tests pass.
