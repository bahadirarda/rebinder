---
type: adr
title: Rust Reference Implementation
status: accepted
version: 0.1.0
---

# ADR-0002: Rust Reference Implementation

## Context

Rebinder is a local-first CLI that reads provider session data, handles
potentially sensitive state, validates untrusted interchange packages, and
should be distributable without a language runtime dependency.

## Decision

Implement the reference CLI and core library in Rust. Use checked-in JSON
Schema documents as the serialized contract and strongly typed Rust models for
inspection and adapter boundaries.

The initial crate targets Rust edition 2024 with a declared MSRV of 1.85.

## Consequences

Positive:

- Package parsing and filesystem boundaries benefit from Rust's type and memory
  safety.
- The CLI can be distributed as a standalone native binary.
- Provider adapters share one typed canonical core.

Trade-offs:

- Adapter development must account for provider formats that originate in
  dynamic JSON or JSONL structures.
- Rust compile times and cross-platform release automation add build-system
  work.
