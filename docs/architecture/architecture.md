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

The implemented package-to-artifact slice now crosses the target-independent
half of that pipeline:

```text
Validated Interchange Package
    -> Target capability declaration
    -> Package feature analysis
    -> Compatibility report (compatible | compatible_with_loss | incompatible)
    -> Bounded Markdown continuation artifact
```

Both source adapters now enter the canonical half of the pipeline:

```text
Claude Code local project JSONL -> Claude source adapter --+
                                                        +-> canonical model
Codex app-server thread/list + thread/read -------------+   -> package encoder
                                                            -> self-validation
```

The Claude adapter reads regular JSONL session files under the configured
project store. The Codex adapter uses the stable, non-resuming app-server read
surface and never parses rollout files. Both map visible messages, current
intent, workspace facts, readable Git metadata, a bounded handoff, and
provenance into the same schema `0.1.0` package.

The operational Claude-to-Codex MVP uses a target-native bridge exposed by
Codex instead of writing Codex's private session representation:

```text
Rebinder
    -> Codex app-server externalAgentConfig/detect
    -> Select one Claude Code SESSIONS migration
    -> Full: Codex app-server externalAgentConfig/import
       Handoff: Codex app-server thread/start or thread/resume
                -> thread/inject_items with role-preserved bounded items
                -> thread/compact/start for a meaningful repeat update
                -> turn/start for a read-only visible continuation brief
                -> thread/read only to recover an interrupted activation
    -> Native Codex thread ID
    -> Rebinder opens the bound thread in the recorded Claude workspace
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
JSON-RPC. Full import leaves parsing to Codex. The context-safe path streams only
Claude message text and compact-summary records into a bounded Rebinder-owned
handoff, preserves user and assistant roles in injected response items, and
uses a semantic hash so metadata-only source churn does not create another
checkpoint. A meaningful update to an existing handoff thread is compacted
through Codex before Rebinder starts one read-only, no-tool activation turn.
That turn converts the injected prompt history into a visible continuation
brief containing the objective, verified state, decisions, and next action.
The append-only binding ledger records `pending`, `injected`, `ready`,
`activating`, and `completed` phases. If the final ledger write is interrupted,
`thread/read` locates the completed turn by its semantic-revision marker rather
than producing a duplicate. Neither path writes Codex session files directly.

The compatibility module remains provider-neutral. It loads a package only
after structural validation, evaluates only capabilities used by that package,
and renders target-bound Markdown without launching either harness. Capability
declarations distinguish preserved, summarized, and omitted state. Invalid
packages are blocking; declared information loss is non-blocking and remains
machine-readable. Artifact creation uses create-new semantics, mode `0600` on
Unix, and a fixed visible-conversation budget. Provider-private tool inputs,
tool-result payloads, attachments, environment values, and remote URLs do not
enter the artifact.

The export module owns provider parsing, safe-default redaction, package
encoding, manifest hashing, and post-write self-validation. Provider-private
reasoning, attachment bodies, environment values, remote URLs, and tool
payloads do not cross the adapter boundary. Export creates a new package root
and private files rather than modifying source sessions or replacing an
existing package.

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

rebinder export --from <source> [session] --output <new-directory>
    -> source adapter read surface
    -> canonical package + self-validation
```

The command facade MUST pass native provider arguments without interpreting
them and MUST inherit interactive standard streams. The cross-harness transfer
path owns its source and target flags; arguments following `--` belong only to
the target provider.

For Claude-to-Codex, omission of the session ID opens an interactive picker when
standard streams are terminals. In non-interactive use it selects only the
latest session whose recorded `cwd` resolves to the current directory. An
explicit ID can select another discovered session. The auto strategy routes
large sources through a bounded, persistent handoff injected into a native
Codex thread. Rebinder owns selection and opening, so users do not manually run
Codex resume commands for a transfer. A missing recorded directory is an error;
the MVP reuses existing worktrees but does not recreate them.
