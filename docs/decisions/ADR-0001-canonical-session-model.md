---
type: adr
title: Canonical Session Model
status: accepted
version: 0.1.0
---

# ADR-0001: Canonical Session Model

## Context

Provider harnesses use different session formats and expose different runtime capabilities. Transcript-only transfer is insufficient to preserve workspace, repository, task, and execution state.

## Decision

Define a provider-neutral canonical session model and place provider-specific behavior behind adapters.

The initial model contains conversation history, task state, workspace state, repository state, tool call records, checkpoints, handoff summary, and provenance.

## Consequences

Positive:

- New providers can be added without changing the core model.
- Unsupported fields can be reported explicitly.
- The interchange format can be versioned independently from provider formats.

Trade-offs:

- Some provider-specific semantics will be lossy.
- Canonicalization requires explicit semantic mapping rules.
- Exact continuation equivalence cannot be guaranteed across models or harnesses.

