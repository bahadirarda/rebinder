---
type: product-requirements
title: Product Requirements
status: draft
version: 0.1.0
---

# Product Requirements

## Functional requirements

### FR-001 — Export

The system MUST export a provider session into the canonical session model.

### FR-002 — Import

The system MUST import a canonical session package and produce a provider-compatible continuation artifact.

### FR-003 — Inspect

The system MUST expose session metadata, task state, repository state, and compatibility information without requiring an agent run.

### FR-004 — Validate

The system MUST validate package structure and schema version before import.

### FR-005 — Capability reporting

The system MUST report fields and behaviors that cannot be represented by the target provider adapter.

### FR-006 — Redaction

The system MUST support redaction of credentials, tokens, environment values, and provider-private data before export.

### FR-007 — Provenance

The system MUST record source harness, adapter version, schema version, export time, and transformation steps.

### FR-008 — Harness command facade

The CLI MUST expose provider namespaces such as `rebinder codex` and `rebinder
claude`. Arguments after the provider name, interactive standard streams, and
the provider process exit status MUST be preserved.

### FR-009 — Cross-harness transfer

The CLI MUST expose an unambiguous source-to-target transfer operation. The
initial command contract is `rebinder transfer --from <source> --to <target>
[session-id] -- [target arguments]`. Rebinder-owned flags MUST NOT alter the
meaning of native provider commands in the harness command facade.

## Quality requirements

- The interchange package SHOULD be human-readable and Git-friendly.
- Schema changes MUST be versioned.
- Import/export operations SHOULD be deterministic for the same input and configuration.
- The system MUST distinguish structural validation from semantic compatibility.
- A failed migration MUST NOT silently discard unsupported state.
