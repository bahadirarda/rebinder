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
- Claude Code session discovery through the Codex external-agent import API
- Exact Claude session selection and current-workspace latest-session selection
- Claude-to-Codex native session import and Codex thread resolution
- Repeat import checkpointing through Codex's session import ledger
- Native Codex resume in the Claude session's recorded workspace or worktree
- Fail-closed handling for missing workspaces and unsupported directions
- Human and JSON session inventory output
- Calendar release identity `0.YYYYMMDD.REVISION`
- Changesets release-intent ledger and automated version pull requests
- Checksum-verifying Unix and Windows installers
- Five-target native GitHub Release workflow
- Release metadata, checksum manifest, and GitHub artifact attestations

## Operational transfer path

`rebinder transfer [session] --from claude --to codex -- [target args]` is
operational for local Claude Code sessions supported by the installed Codex
importer. Rebinder selects only the `SESSIONS` migration item, waits for the
native Codex thread ID, verifies that the recorded workspace exists, and runs
`codex resume` in that directory.

Omitting the session ID is allowed only when discovery finds a session whose
recorded working directory matches the current directory. Explicit selection
uses the Claude provider session ID shown by `rebinder sessions claude`.

Rebinder never edits Claude transcript files or writes directly into Codex's
private session store. Codex owns conversion, import-ledger updates, and native
thread creation.

## Pending MVP capabilities

- Codex session discovery and canonical export
- Claude Code canonical-package export independent of Codex
- Provider capability declarations and compatibility reports
- Provider-neutral target continuation artifact generation
- End-to-end Codex-to-Claude transfer
- Missing-worktree reconstruction

No pending capability may be described as available in installation or release
documentation before its acceptance tests pass.
