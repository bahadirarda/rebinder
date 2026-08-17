---
type: architecture
title: Initial Architecture
status: draft
version: 0.1.0
---

# Initial Architecture

## Architectural style

The system uses a provider-adapter architecture around a canonical session model. Provider-specific concerns remain at the adapter boundary; core validation and package semantics remain provider-neutral.

## Data flow

The provider-neutral path remains the long-term interoperability architecture:

```text
Source Harness
    -> Source Provider Adapter
    -> Canonical Session Model
    -> Interchange Package
    -> Target Provider Adapter
    -> Target Harness Continuation Artifact
```

The operational Claude-to-Codex MVP uses a target-native bridge exposed by
Codex instead of writing Codex's private session representation:

```text
Rebinder
    -> Codex app-server externalAgentConfig/detect
    -> Select one Claude Code SESSIONS migration
    -> Codex app-server externalAgentConfig/import
    -> Native Codex thread ID
    -> codex resume in the recorded Claude workspace
```

## Core boundaries

### Core

- canonical model
- schema validation
- package manifest
- compatibility evaluation
- provenance
- redaction policy

### Provider adapter

- provider session discovery
- provider parsing
- provider export mapping
- provider capability declaration
- provider-specific error handling

## Initial package contents

- `manifest.json`
- `session.json`
- `conversation.jsonl`
- `task-state.json`
- `workspace-state.json`
- `repository-state.json`
- `handoff.md`
- `provenance.json`
- optional `patches/`

## Important constraint

The system provides portable state and a continuation artifact. It does not claim that a different model or harness can reproduce the exact internal reasoning of the source harness.

## Initial implementation

The reference implementation is a Rust 2024 crate with a library boundary and
a thin `rebinder` CLI. Checked-in JSON Schema Draft 2020-12 documents define the
serialized `0.1.0` contract. The Rust core validates package containment,
regular-file constraints, SHA-256 integrity, individual documents, and
cross-document invariants before exposing inspection data.

Provider discovery and parsing MUST NOT be introduced into the canonical
validation modules. The Claude-to-Codex native bridge is isolated in the
transfer and handoff modules and communicates with Codex over newline-delimited
JSON-RPC. Full import leaves parsing to Codex. Context-safe import streams only
Claude message text and compact-summary records into a bounded Rebinder-owned
handoff; neither path writes Codex session files.

## CLI command boundaries

The CLI has two distinct execution paths:

```text
rebinder <harness> <native arguments>
    -> transparent harness command facade
    -> provider process

rebinder transfer --from <source> --to <target> [session] [--strategy <strategy>] -- [target arguments]
    -> direction-specific transfer adapter
    -> target-native continuation artifact
    -> target provider process
```

The command facade MUST pass native provider arguments without interpreting
them and MUST inherit interactive standard streams. The cross-harness transfer
path owns its source and target flags; arguments following `--` belong only to
the target provider.

For Claude-to-Codex, omission of the session ID opens an interactive picker when
standard streams are terminals. In non-interactive use it selects only the
latest session whose recorded `cwd` resolves to the current directory. An
explicit ID can select another discovered session. The auto strategy routes
large sources through a bounded, persistent handoff before the target-native
import. A missing recorded directory is an error; the MVP reuses existing
worktrees but does not recreate them.
