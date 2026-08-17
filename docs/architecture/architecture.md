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

```text
Source Harness
    -> Source Provider Adapter
    -> Canonical Session Model
    -> Interchange Package
    -> Target Provider Adapter
    -> Target Harness Continuation Artifact
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

Provider adapters will depend on the core library. Provider discovery and
parsing MUST NOT be introduced into the canonical validation modules.

## CLI command boundaries

The CLI has two distinct execution paths:

```text
rebinder <harness> <native arguments>
    -> transparent harness command facade
    -> provider process

rebinder transfer --from <source> --to <target> [session] -- [target arguments]
    -> source adapter export
    -> canonical validation and redaction
    -> target compatibility evaluation
    -> target adapter continuation artifact
    -> target provider process
```

The command facade MUST pass native provider arguments without interpreting
them and MUST inherit interactive standard streams. The cross-harness transfer
path owns its source and target flags; arguments following `--` belong only to
the target provider.
