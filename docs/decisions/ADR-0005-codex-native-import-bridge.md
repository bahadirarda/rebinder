---
type: adr
title: Codex Native Import Bridge
status: accepted
version: 0.1.0
---

# ADR-0005: Codex Native Import Bridge

## Context

The canonical interchange model remains necessary for symmetric portability,
inspection, redaction, and compatibility reporting. It is not necessary to
write a second Claude transcript parser or mutate Codex's private session store
when Codex already exposes an external-agent migration API.

Codex app-server provides `externalAgentConfig/detect` and
`externalAgentConfig/import`. Its session importer accepts selected Claude Code
sessions, creates or checkpoints a native Codex thread, records the source to
target binding, and returns the target thread ID.

## Decision

Implement the first operational direction as a target-native import bridge:

1. Start the installed Codex app-server over its local stdio JSON-RPC transport.
2. Detect Claude Code migration items with source `claude-code`.
3. Select exactly one `SESSIONS` entry by provider ID, or the most recent entry
   whose recorded working directory matches the current directory.
4. Import only that session entry. Do not select settings, skills, plugins,
   hooks, commands, MCP servers, subagents, memory, or credentials.
5. Wait for the completed import result and obtain the native Codex thread ID.
6. Verify that the recorded workspace exists, then invoke `codex resume` in it.
7. On repeat transfer, use Codex's import ledger and checkpoint behavior so a
   changed Claude transcript appends to its existing imported thread.

The bridge must fail closed when the app-server method is unavailable, session
selection is ambiguous, the source ID is unknown, the import reports a failure,
no target thread ID is returned, or the recorded workspace is missing.

## Consequences

Users receive a working Claude-to-Codex path without Rebinder depending on
undocumented provider file formats. Claude and Codex remain the owners of their
session stores, and imported threads are native Codex sessions that can be
resumed normally.

This path is asymmetric and depends on the installed Codex version's supported
import surface. It does not replace the canonical interchange pipeline,
compatibility reports, redaction policy, Codex-to-Claude transfer, or missing
worktree reconstruction.

## References

- [Import from another agent](https://learn.chatgpt.com/docs/import)
- [Codex app-server external-agent import](https://learn.chatgpt.com/docs/app-server#detect-and-import-external-agent-config)
