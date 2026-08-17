---
type: product-charter
title: Project Charter
status: draft
version: 0.1.0
---

# Project Charter

## Product identity

**Rebinder** is an open, versioned interchange format and provider-adapter framework for cross-harness agent session portability.

## Purpose

Enable a user to continue an agent session in a different coding agent harness while preserving the task intent, relevant conversation history, workspace state, repository state, execution records, and handoff summary.

## Problem statement

Coding agent harnesses maintain provider-specific session representations. A session started in one harness cannot reliably be continued in another because its state, tool records, decisions, and workspace assumptions are not represented in a common interchange model.

## Product thesis

Session portability should be based on a structured, versioned representation of agent state rather than transcript copying alone.

## Scope

The initial scope is local-first cross-harness session transfer between Codex and Claude Code, using provider adapters around a canonical session model.

## Non-goals for MVP

- Real-time multi-agent orchestration
- Cloud session hosting
- Universal support for every harness feature
- Guaranteed semantic equivalence between different models
- Automatic transfer of secrets or credentials

## Success criteria

- A session package can be inspected and structurally validated without its source harness.
- A Codex-originated package can produce a usable continuation context for Claude Code, and vice versa.
- Unsupported provider capabilities are reported explicitly.
- The package is deterministic, versioned, and safe to share after redaction.
