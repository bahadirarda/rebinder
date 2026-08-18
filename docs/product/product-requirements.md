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

### FR-010 — Claude-to-Codex native bridge

The first operational direction MUST discover local Claude Code sessions
through Codex's supported external-agent API. A full transfer MUST import only
the selected session. A context-safe handoff MUST create or resume a native
Codex thread and inject the bounded checkpoint through supported Codex thread
APIs. A bounded handoff MUST create a visible, history-grounded continuation
brief before opening the target. Both paths MUST obtain the target thread ID
and open that thread from Rebinder in the source session's recorded workspace;
a separate user-issued Codex resume command MUST NOT be required. Rebinder MUST
NOT directly mutate provider session stores or import credentials and
configuration as a side effect.

When the source transcript has changed since a prior transfer, the bridge MUST
use the target provider's checkpoint behavior rather than create an unrelated
duplicate. When the recorded workspace is missing, the bridge MUST stop before
starting Codex.

### FR-011 — Interactive source selection

When standard input and standard error are terminals and no session ID is
provided, Claude-to-Codex transfer MUST present the discovered sessions as an
interactive selector. Each choice MUST identify its state, title, recorded
workspace, source size, and recommended strategy. Escape MUST cancel without
importing. Non-interactive invocation MUST retain deterministic current-workspace
selection.

### FR-012 — Context-safe large-session handoff

The default transfer strategy MUST prevent a large imported transcript from
immediately exhausting the target context window. Sources larger than 512 KiB
MUST use a bounded handoff made from the latest Claude compact summary and
recent visible user and assistant text. Thinking, tool calls, and tool results
MUST NOT be copied into the handoff. Injected visible messages MUST retain their
user or assistant role. Users MAY explicitly request full or handoff behavior.

Derived handoffs MUST be append-only, keyed stably to the source path, private
to the current user where permissions are available, and rejected when their
target path is a symlink. Their semantic-revision-to-thread binding MUST survive a
failed attempt. An unchanged source MUST reuse its existing native Codex thread;
metadata-only changes MUST NOT create a new checkpoint. A meaningful changed
source MUST append and inject one new bounded checkpoint into that thread, then
complete target-native compaction before Codex opens. If compaction fails after
injection, a retry MUST NOT inject the same checkpoint again.

Each new semantic handoff revision MUST start at most one activation turn that
summarizes the current objective, verified state, important decisions, and next
action for visible target-native continuation. Activation MUST use read-only
sandboxing, disable approval escalation, and instruct the model not to call
tools or modify files. The CLI and security documentation MUST disclose that
activation consumes target-model tokens. If activation finishes before its
ledger completion record is persisted, a retry MUST detect the matching
completed turn and MUST NOT create a duplicate brief.

## Quality requirements

- The interchange package SHOULD be human-readable and Git-friendly.
- Schema changes MUST be versioned.
- Import/export operations SHOULD be deterministic for the same input and configuration.
- The system MUST distinguish structural validation from semantic compatibility.
- A failed migration MUST NOT silently discard unsupported state.
