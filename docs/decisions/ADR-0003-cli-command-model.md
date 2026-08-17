---
type: adr
title: CLI Command Model
status: accepted
version: 0.1.0
---

# ADR-0003: CLI Command Model

## Context

Users need access to native Codex and Claude CLI commands through Rebinder, but
cross-harness transfer also needs Rebinder-owned source, target, validation,
and compatibility options. Adding migration flags inside provider namespaces
would risk collisions with present or future provider flags.

## Decision

Use two command forms.

Provider command facade:

```text
rebinder codex <native Codex arguments>
rebinder claude <native Claude arguments>
```

Cross-harness transfer:

```text
rebinder transfer --from <source> --to <target> [session-id] -- [target arguments]
```

The facade forwards arguments and interactive standard streams unchanged and
returns the provider process exit status. The cross-harness form owns all
arguments before `--`; arguments after `--` are forwarded to the target harness
after a successful migration.

## Consequences

Positive:

- Native provider CLI behavior remains familiar and collision-free.
- Migration direction is explicit and symmetrical.
- New harness namespaces can be added without changing the resume grammar.

Trade-offs:

- Native resume and cross-harness transfer have deliberately different command
  forms.
- Shell completion must combine Rebinder commands with provider-specific
  completion behavior.
