---
type: product-status
title: Foundation MVP Status
status: draft
version: 0.1.0
---

# Foundation MVP Status

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
- Calendar release identity `0.YYYYMMDD.REVISION`
- Changesets release-intent ledger and automated version pull requests
- Checksum-verifying Unix and Windows installers
- Five-target native GitHub Release workflow
- Release metadata, checksum manifest, and GitHub artifact attestations

## Contract defined, execution pending

`rebinder transfer --from <source> --to <target> [session] -- [target args]`
is parsed and documented but returns exit code `2`. This is intentional. It MUST
remain fail-closed until both provider adapters can:

1. discover and export source sessions;
2. apply redaction policy;
3. emit structurally valid canonical packages;
4. report target capability loss;
5. create a target continuation artifact; and
6. prove an end-to-end continuation fixture in both directions.

## Pending MVP capabilities

- Codex session discovery and canonical export
- Claude Code session discovery and canonical export
- Provider capability declarations and compatibility reports
- Target continuation artifact generation
- End-to-end Codex-to-Claude transfer
- End-to-end Claude-to-Codex transfer

No pending capability may be described as available in installation or release
documentation before its acceptance tests pass.
