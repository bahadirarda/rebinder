---
type: adr
title: Canonical Codex-to-Claude Continuation
status: accepted
version: 0.1.0
---

# ADR-0008: Canonical Codex-to-Claude Continuation

## Context

Claude Code does not expose a supported API for writing arbitrary native
transcript records. It does support starting an interactive session with an
initial prompt, assigning a UUID with `--session-id`, adding invocation context
from a file, and resuming a session by ID with an optional new prompt. Rebinder
already has a non-resuming Codex exporter, target compatibility assessment, and
a bounded provider-neutral artifact.

Directly generating Claude JSONL would couple Rebinder to private persistence
and could corrupt target state. Starting a fresh unrelated session on every
transfer would also lose continuity and duplicate context.

## Decision

Implement `rebinder transfer --from codex --to claude [session-id]` as a
composition of supported surfaces:

1. Read the Codex thread through `thread/read(includeTurns: true)` without
   resuming it.
2. Export and validate a canonical package, then assess and render the bounded
   Claude continuation artifact.
3. Derive a stable UUID from the source Codex thread ID.
4. Start Claude with `--session-id` when that target does not exist, resume it
   with a new activation prompt for a changed source revision, or resume without
   reinjection when the revision is already active.
5. Run Claude in the recorded Codex workspace and inherit interactive streams.

The semantic revision covers session, conversation, task, workspace,
repository, and handoff documents while excluding export-time provenance. An
activation is complete only when the target transcript contains the revision
marker and a later visible assistant message.

The artifact is written to a process-unique, private temporary file and passed
with Claude's supported `--append-system-prompt-file` flag. A wrapper marks the
entire artifact as untrusted historical data, escapes the wrapper's closing
delimiter, and reasserts the boundary afterward. The short user prompt asks for
a no-tool visible continuation brief, but does not claim sandbox enforcement.
Temporary staging is removed after preparation and when the target process
exits normally.

Rebinder rejects target arguments that could select a different native session
or worktree. Other Claude options remain pass-through. The `--strategy` flag is
specific to the opposite direction; the reverse adapter always uses the
bounded canonical path.

## Consequences

Users can move a selected Codex thread into Claude and stay entirely inside the
Rebinder command. Repeat transfers keep one native target session and avoid
duplicating unchanged history. Changed source state creates one new bounded
checkpoint in that same session.

The first response for a new revision consumes normal Claude model tokens. The
untrusted-history wrapper mitigates priority confusion but cannot guarantee
that model behavior is immune to prompt injection. Native session creation,
authentication, permissions, and persistence remain Claude Code's
responsibility.

## References

- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-usage)
