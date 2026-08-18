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
`thread/start`, `thread/resume`, `thread/inject_items`, `thread/read`,
`turn/start`, and `thread/compact/start` for native thread history, turns, and
compaction. Injected response items are model-visible prompt history but are not
rendered as ordinary turns. A native full
import can flatten a very large Claude transcript,
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
   and tool results. Preserve the selected user and assistant roles, derive a
   semantic revision from this bounded material rather than raw JSONL bytes,
   and store the result append-only in Rebinder's private platform data
   directory.
6. For a full transfer, import only the detected source session entry. Do not
   select settings, skills, plugins, hooks, commands, MCP servers, subagents,
   memory, or credentials.
7. For a handoff, create a native Codex thread with `thread/start`, or load its
   prior thread with `thread/resume`, then append metadata, compact summary, and
   recent conversation as role-preserving items with `thread/inject_items`
   without starting a model turn.
8. After injection and any required compaction, start one `turn/start` activation
   with read-only sandboxing, approvals disabled, and a prompt that forbids tool
   use and file changes. The response MUST be a visible continuation brief
   grounded only in the transferred history. Read the authoritative visible
   `agentMessage` from its `item/completed` event and use `turn/completed` only
   for the final turn status; current app-server versions need not repeat the
   completed agent item inside the turn payload.
9. Store pending, injected, ready, activating, and completed
   semantic-revision-to-thread bindings beside the append-only handoff. A retry
   after injection continues without injecting the same items again. If an
   activation completed before its final ledger write, recover it with
   `thread/read` and an exact source-revision marker rather than starting a
   duplicate turn.
10. On repeat transfer, reuse an unchanged handoff thread. Ignore source changes
   that do not affect the bounded visible conversation. For a meaningful
   update, inject one checkpoint and complete `thread/compact/start` before
   activating and opening the thread.
11. Verify that the recorded workspace exists, then have Rebinder open the
    bound thread there through the installed Codex CLI. The internal resume
    invocation is not a separate user step.
12. When upgrading a legacy flattened handoff binding to the role-preserving
    format, leave the legacy thread untouched and create a fresh native thread.
    Subsequent updates reuse and compact the new binding.
13. Treat a role-preserving handoff completed before continuity activation was
    introduced as ready for in-place activation. Do not reinject its history or
    replace its native thread.

The bridge must fail closed when the app-server method is unavailable, session
selection is ambiguous, the source ID is unknown, a native API rejects the
operation, no target thread ID is returned, or the recorded workspace is
missing.

## Consequences

Users receive a working Claude-to-Codex path whose default behavior remains
within a conservative context budget. The bounded path depends on the minimal
Claude JSONL message and compact-summary records, but never mutates Claude's
store. Semantic revisions prevent unrelated Claude metadata writes from
duplicating history, while target-native compaction absorbs meaningful repeat
updates. A bounded transfer performs one normal Codex model request per new
semantic revision to make that context visible as a continuation brief; an
unchanged transfer performs none. Both full-import and handoff threads remain
native Codex sessions that Rebinder can open normally.

This path is asymmetric and depends on the installed Codex version's supported
import surface. It does not replace the canonical interchange pipeline,
compatibility reports, redaction policy, Codex-to-Claude transfer, or missing
worktree reconstruction.

## References

- [Import from another agent](https://learn.chatgpt.com/docs/import)
- [Codex app-server](https://learn.chatgpt.com/docs/app-server)
