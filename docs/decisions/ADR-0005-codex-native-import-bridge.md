---
type: adr
title: Codex Native Transfer Bridge
status: accepted
version: 0.1.0
---

# ADR-0005: Codex Native Transfer Bridge

## Context

The canonical interchange model remains necessary for symmetric portability,
inspection, redaction, and compatibility reporting. Codex's supported
app-server APIs avoid direct writes to its private session store. The
external-agent importer remains the authoritative full-conversion boundary;
native thread APIs provide the bounded-handoff persistence boundary.

Codex app-server provides `externalAgentConfig/detect` and
`externalAgentConfig/import` for external sessions. It also provides
`thread/start`, `thread/resume`, and `thread/inject_items` for native thread
history. A native full import can flatten a very large Claude transcript,
including history before Claude compact summaries. Such a thread can exceed
the Codex context window before remote compaction can recover it. The external
session importer accepts only sessions returned by detection, so a derived
Rebinder handoff cannot be passed to it as an arbitrary source path.

## Decision

Implement the first operational direction as a target-native transfer bridge:

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
6. For a full transfer, import only the detected source session entry. Do not
   select settings, skills, plugins, hooks, commands, MCP servers, subagents,
   memory, or credentials.
7. For a handoff, create a native Codex thread with `thread/start`, or load its
   prior thread with `thread/resume`, then append the bounded checkpoint with
   `thread/inject_items` without starting a model turn.
8. Store pending and completed source-hash-to-thread bindings beside the
   append-only handoff so a failed attempt can safely reuse its thread.
9. Verify that the recorded workspace exists, then invoke `codex resume` in it.
10. On repeat transfer, reuse an unchanged handoff thread. When the source
    changes, inject one new bounded checkpoint into that same thread.

The bridge must fail closed when the app-server method is unavailable, session
selection is ambiguous, the source ID is unknown, a native API rejects the
operation, no target thread ID is returned, or the recorded workspace is
missing.

## Consequences

Users receive a working Claude-to-Codex path whose default behavior remains
within a conservative context budget. The bounded path depends on the minimal
Claude JSONL message and compact-summary records, but never mutates Claude's
store. Both full-import and handoff threads remain native Codex sessions that
can be resumed normally.

This path is asymmetric and depends on the installed Codex version's supported
import surface. It does not replace the canonical interchange pipeline,
compatibility reports, redaction policy, Codex-to-Claude transfer, or missing
worktree reconstruction.

## References

- [Import from another agent](https://learn.chatgpt.com/docs/import)
- [Codex app-server](https://learn.chatgpt.com/docs/app-server)
