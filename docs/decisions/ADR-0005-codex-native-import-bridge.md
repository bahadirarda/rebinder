---
type: adr
title: Codex Native Import Bridge
status: accepted
version: 0.1.0
---

# ADR-0005: Codex Native Import Bridge

## Context

The canonical interchange model remains necessary for symmetric portability,
inspection, redaction, and compatibility reporting. Codex's external-agent
migration API avoids direct writes to its private session store and remains the
authoritative conversion boundary.

Codex app-server provides `externalAgentConfig/detect` and
`externalAgentConfig/import`. Its session importer accepts selected Claude Code
sessions, creates or checkpoints a native Codex thread, records the source to
target binding, and returns the target thread ID. A native full import can
nevertheless flatten a very large Claude transcript, including history before
Claude compact summaries. Such a thread can exceed the Codex context window
before remote compaction can recover it.

## Decision

Implement the first operational direction as a target-native import bridge:

1. Start the installed Codex app-server over its local stdio JSON-RPC transport.
2. Detect Claude Code migration items with source `claude-code`.
3. Select exactly one `SESSIONS` entry by provider ID, an interactive picker,
   or the most recent entry whose recorded working directory matches the current
   directory in non-interactive use.
4. Resolve `auto` to a full import at or below 512 KiB and a context-safe handoff
   above that threshold. Permit explicit `full` and `handoff` overrides.
5. For a handoff, stream the Claude JSONL, retain only the latest compact
   summary and bounded visible text after it, and exclude thinking, tool calls,
   and tool results. Store the result append-only in Rebinder's private platform
   data directory.
6. Import only the selected original or derived session entry. Do not select settings, skills, plugins,
   hooks, commands, MCP servers, subagents, memory, or credentials.
7. Wait for the completed import result and obtain the native Codex thread ID.
8. Verify that the recorded workspace exists, then invoke `codex resume` in it.
9. On repeat transfer, use the import ledger and a stable derived source path so
   an unchanged source reuses its thread and a changed source appends a bounded
   checkpoint.

The bridge must fail closed when the app-server method is unavailable, session
selection is ambiguous, the source ID is unknown, the import reports a failure,
no target thread ID is returned, or the recorded workspace is missing.

## Consequences

Users receive a working Claude-to-Codex path whose default behavior remains
within a conservative context budget. The bounded path depends on the minimal
Claude JSONL message and compact-summary records, but never mutates Claude's
store. Imported threads remain native Codex sessions that can be resumed
normally.

This path is asymmetric and depends on the installed Codex version's supported
import surface. It does not replace the canonical interchange pipeline,
compatibility reports, redaction policy, Codex-to-Claude transfer, or missing
worktree reconstruction.

## References

- [Import from another agent](https://learn.chatgpt.com/docs/import)
- [Codex app-server external-agent import](https://learn.chatgpt.com/docs/app-server#detect-and-import-external-agent-config)
