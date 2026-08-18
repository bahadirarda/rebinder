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
starting Codex unless the user explicitly requests recovery that satisfies
FR-016.

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

For app-server activation, Rebinder MUST treat the final `item/completed`
`agentMessage` as the authoritative visible response and `turn/completed` as
the authoritative turn status. It MUST NOT require the completed turn payload
to repeat streamed agent items. Interactive and non-interactive transfer
commands MUST emit concise progress lines before potentially blocking provider,
import, compaction, and activation stages.

### FR-013 — Capability-aware continuation artifacts

Every target adapter MUST publish a machine-readable declaration that labels
canonical capabilities as preserved, summarized, or omitted. Compatibility
assessment MUST validate the package first, evaluate capabilities actually used
by that package, distinguish blocking invalidity from non-blocking information
loss, and support human and JSON output without launching a provider.

The provider-neutral artifact generator MUST retain handoff guidance, task
state, recorded workspace and repository facts, provenance, and bounded recent
visible text. It MUST NOT copy tool-result payloads, attachment payloads,
environment values, or repository remote URLs. It MUST NOT overwrite an
existing output path and MUST use private file permissions where the platform
supports them.

### FR-014 — Provider canonical export

The Claude source adapter MUST discover and parse locally stored Claude Code
project sessions without requiring Codex. The Codex source adapter MUST list
threads and read complete turn data through supported app-server methods
without resuming the selected thread or directly reading private rollout
files. Both adapters MUST produce the same canonical package contract.

Export MUST exclude private reasoning, attachment payloads, environment
values, repository remote URLs, and provider tool input/output payloads by
default. It MUST apply best-effort credential redaction to visible text and
record transformations and redaction counts in provenance. Output MUST use a
new directory, MUST NOT overwrite an existing path, MUST use private Unix
permissions where supported, and MUST pass normal package validation before
success is reported.

### FR-015 — Codex-to-Claude native continuation

Codex-to-Claude transfer MUST read the selected Codex thread through supported
app-server methods, produce and validate canonical state, assess Claude target
compatibility, and open Claude Code from Rebinder in the recorded workspace.
The adapter MUST create a deterministic native Claude session for the source
thread and MUST resume that session on repeat transfer. Users MUST NOT need to
run a separate Claude resume command.

The target context MUST be bounded, stored only in a private temporary file
while the target invocation needs it, and identified as untrusted historical
data. A semantic source revision MUST cover conversation, task, workspace,
repository, handoff, and session state. An already activated revision MUST NOT
be injected again; a changed revision MUST update the existing target session.
Activation MUST request a visible continuation brief without tool use and MUST
disclose target-model token consumption. Rebinder MUST reject target arguments
that conflict with its target session binding.

### FR-016 — Safe registered-worktree recovery

Both transfer directions MUST fail closed on a missing recorded workspace by
default. Recovery MUST require an explicit `--recover-worktree` option and MUST
operate only when a local Git repository still reports the exact missing path
from its worktree registry. Users MUST be able to identify the owning main
worktree with `--worktree-repository` when bounded automatic discovery is
insufficient or ambiguous.

The recovery adapter MUST reject relative targets, existing targets, missing
parents, immediate symlink parents, locked registrations, missing commits,
ambiguous repository matches, and branch movement during the operation. It
MUST NOT clone, fetch, unlock, overwrite, or claim to recover uncommitted state.
After `git worktree add`, it MUST verify the recorded commit, attached branch
when present, and common Git directory before a provider is launched.

### FR-017 — Consent-gated proactive handoff

Rebinder MUST be able to install an opt-in source-harness integration that
observes an authoritative provider usage signal and offers a supported target
harness before the source limit is exhausted. The initial adapter MUST use
Claude Code's documented status-line `rate_limits` fields for five-hour and
seven-day Claude.ai windows. It MUST NOT scrape terminal output, infer account
billing, or treat an absent subscriber signal as zero usage.

The initial policy MUST verify that Codex is authenticated before enablement and
again before creating an offer. Default thresholds MUST be configurable. An
offer MUST be deduplicated by source session, target, limit kind, and provider
reset window. A hook MUST surface the offer at most once, and a decline MUST
suppress it for that window.

No transfer may be prepared or target process launched without explicit user
acceptance. A Claude session launched through `rebinder claude` MUST bind offers
to that wrapper process and, after acceptance and source exit, MUST use the
normal Claude-to-Codex transfer adapter to open the target. A direct Claude
launch MUST receive an explicit resume fallback. Hook and tool subprocesses
MUST NOT nest an interactive target TUI.

Enablement MUST preserve an existing Claude Code status-line command and
disablement MUST restore its exact configuration. Plugin removal MUST refuse
unknown files, and status-line restoration MUST fail closed if ownership has
changed. Policy, observation, offer, and transition state MUST use private
local storage where supported and MUST remain inspectable through human and
JSON status output.

### FR-018 — Authoritative hard-limit rescue

The Claude continuity integration MUST record a rescue candidate when Claude
Code's documented `StopFailure` hook reports the exact `rate_limit` error type.
It MUST ignore other provider failures, MUST NOT treat provider error text as
authorization, and MUST NOT persist `error_details` or rendered API-error text.
A matching failure MUST reuse an actionable offer for the same active provider
window or create one transcript-revision-scoped rescue. Repeated hook delivery
MUST NOT create duplicate rescue records or notifications. An active decline
MUST remain authoritative.

Because `StopFailure` has no decision control, the hook MUST only record local
state and emit an allowlisted terminal notification. It MUST NOT start a target
process. After a `rebinder claude` source exits, the enclosing parent MUST ask
for explicit consent in an interactive terminal with a negative default. A
direct Claude launch MUST have an explicit `rebinder continuity rescue`
fallback. Non-interactive rescue MUST fail unless the operator supplies an
explicit consent flag.

Rescue MUST recheck target availability before asking or transferring and MUST
reuse the normal transfer adapter after acceptance. `resume` and `rescue` MUST
refuse to start a target TUI while invoked inside a Rebinder-owned Claude
process. Rescue state MUST remain private and visible through the normal
continuity status ledger.

## Quality requirements

- The interchange package SHOULD be human-readable and Git-friendly.
- Schema changes MUST be versioned.
- Import/export operations SHOULD be deterministic for the same input and configuration.
- The system MUST distinguish structural validation from semantic compatibility.
- A failed migration MUST NOT silently discard unsupported state.
