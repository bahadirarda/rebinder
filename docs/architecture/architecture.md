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
                -> item/completed agentMessage as authoritative visible output
                -> turn/completed as authoritative final status
                -> thread/read only to recover an interrupted activation
    -> Native Codex thread ID
    -> Rebinder opens the bound thread in the recorded Claude workspace
```

The reverse direction composes the canonical pipeline with Claude's supported
interactive CLI instead of writing its transcript store:

```text
Rebinder
    -> Codex app-server thread/list + thread/read(includeTurns)
    -> Canonical package export + validation
    -> Claude compatibility report + bounded Markdown artifact
    -> Semantic revision over portable source state
    -> New: claude --session-id <stable UUID> <activation>
       Changed: claude --resume <stable UUID> <activation>
       Unchanged: claude --resume <stable UUID>
    -> Native Claude session in the recorded Codex workspace
```

For new and changed revisions, the artifact is placed in a private temporary
file and appended to the target invocation context. A security wrapper fences
the package content as untrusted historical data. The short user activation
prompt carries the source revision and asks for a visible no-tool continuation
brief. A repeat is considered activated only when the native Claude transcript
contains both the matching marker and a later visible assistant message.

The proactive continuity adapter sits before that manual transfer boundary and
does not create a second migration format:

```text
Claude Code status-line JSON
    -> continuity policy (5-hour / 7-day threshold)
    -> immutable session + reset-window offer
    -> Claude plugin hook adds one consent request
    -> accept: arm offer / decline: suppress offer for window
Claude Code StopFailure(error = rate_limit)
    -> reuse active offer or create transcript-revision rescue
    -> deduplicated terminal notification only
    -> source exits
    -> enclosing rebinder claude process
    -> local explicit-consent prompt (safe default: no)
    -> existing Claude-to-Codex transfer adapter
    -> native Codex thread
```

The status-line process inherits a process-scoped launch ID from `rebinder
claude`; the offer stores that ID so another concurrent Claude session cannot
claim it. When there is no enclosing wrapper, the same accepted state is
consumed by an explicit `rebinder continuity resume` command.
For a hard failure, the same parent asks outside the failed model turn. A direct
Claude launch uses `rebinder continuity rescue`; non-interactive rescue requires
an explicit `--yes` flag. Neither path starts a target while the source process
environment is still active.

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

The reverse module owns only target binding and launch. It derives a stable
UUID from the source Codex thread, rejects target CLI flags that could replace
that binding, and delegates all native transcript creation to Claude Code. Its
staging directories are process-unique, private on Unix, and removed after
preparation or after the interactive target exits.

The worktree module is shared by both transfer directions and runs only after
explicit `--recover-worktree` opt-in. It resolves an existing repository from
the supplied `--worktree-repository`, an ancestor, or a bounded direct-sibling
scan; parses `git worktree list --porcelain`; and accepts only one exact,
unlocked missing-path registration with a locally valid commit. It invokes
`git worktree add --force` without a shell or network operation, then verifies
HEAD, an attached branch when present, and the canonical common Git directory.
Existing targets, missing parents, immediate symlink parents, locked entries,
and ambiguous registrations stop before provider launch. The module restores
only committed checkout state.

The continuity module owns integration installation, status-line observation,
offer deduplication, hard-failure rescue, consent transitions, and post-exit routing. It stores a
receipt containing the exact previous Claude status-line JSON, installs only a
fixed managed plugin file set, and uses create-new marker files for asked,
accepted, declined, and completed transitions. It calls the existing transfer
module after consent rather than importing provider history itself. A missing
Claude.ai subscriber signal creates no observation-based offer; Codex
authentication is checked before policy enablement, offer creation, and rescue
consent. The failure hook accepts only the documented `rate_limit` error class,
does not store provider error text, and uses transcript metadata rather than
transcript contents to deduplicate a rescue revision.

## CLI command boundaries

The CLI has two distinct execution paths:

```text
rebinder <harness> <native arguments>
    -> transparent harness command facade
    -> provider process

rebinder transfer --from <source> --to <target> [session] [--strategy <strategy>] [--recover-worktree] -- [target arguments]
    -> direction-specific transfer adapter
    -> target-native continuation artifact
    -> target provider process

rebinder export --from <source> [session] --output <new-directory>
    -> source adapter read surface
    -> canonical package + self-validation

rebinder continuity enable claude --to codex
    -> reversible personal Claude plugin + status-line observer
    -> consent-gated offer ledger

rebinder continuity rescue [--offer <id>]
    -> resolve a provider-reported rate-limit rescue
    -> interactive consent, or explicit non-interactive --yes
    -> normal transfer adapter after source exit

rebinder claude [native arguments]
    -> provider process with continuity launch binding
    -> accepted offer after source exit
    -> normal transfer adapter + target provider process
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
Codex resume commands for a transfer. A missing recorded directory is an error
unless the user opts in to the shared exact registered-worktree recovery path.
