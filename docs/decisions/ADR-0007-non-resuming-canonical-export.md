---
type: adr
title: Non-Resuming Canonical Provider Export
status: accepted
version: 0.1.0
---

# ADR-0007: Non-Resuming Canonical Provider Export

## Context

Portable continuation needs a provider-neutral source package before a target
adapter can assess or consume it. Directly copying private provider stores
would couple Rebinder to unstable persistence details, risk leaking private
model or tool data, and could make a read operation mutate or resume a session.
Claude Code exposes local project transcripts, while Codex exposes stored
thread history through its app-server.

## Decision

Add `rebinder export --from <claude|codex> [session-id] --output <directory>`.
Claude discovery and parsing reads regular JSONL files below
`CLAUDE_CONFIG_DIR/projects` or the default `~/.claude/projects`. It does not
depend on Codex. Codex discovery uses `thread/list`, and the selected thread is
read with `thread/read` and `includeTurns: true`; this method neither resumes
the thread nor subscribes the client to its events.

Both adapters map source identity, visible conversation, current intent,
recorded workspace, readable Git head and change metadata, a bounded handoff,
and provenance into interchange schema `0.1.0`. Private reasoning,
attachments, environment values, repository remote URLs, and provider tool
input/output payloads are excluded. Visible text receives best-effort common
credential redaction. Every transformation and redaction category is recorded.

The output path must not exist. On Unix the package root is mode `0700` and
files are mode `0600`. Rebinder calculates the manifest digests and runs the
same public package validator before reporting success. An ID-less interactive
run opens a picker; an ID-less non-interactive run selects only a session whose
recorded workspace matches the current directory.

## Consequences

Rebinder can now produce provider-neutral, inspectable input for either
transfer direction without launching an agent turn or editing provider state.
The export is deliberately not a provider backup: private and high-risk
payloads are omitted, and visible text can still contain project-sensitive
material that users must review before sharing.

Codex export depends on its supported app-server protocol. Claude export
depends on the locally stored project JSONL representation and therefore keeps
its parser isolated at the adapter boundary with synthetic and real-session
acceptance tests.
