---
type: adr
title: Capability-Aware Continuation Artifacts
status: accepted
version: 0.1.0
---

# ADR-0006: Capability-Aware Continuation Artifacts

## Context

Structural validity proves that a canonical package is intact; it does not
prove that a target harness can represent every field. A direct transfer
adapter built without an explicit loss boundary can silently discard tool
state, attachments, environment assumptions, repository data, or task intent.
Native target session creation is also not required to produce useful,
reviewable continuation context.

## Decision

Introduce three target-independent CLI surfaces:

1. `rebinder capabilities <harness>` returns a versioned declaration whose
   capabilities are classified as `preserved`, `summarized`, or `omitted`.
2. `rebinder compatibility <package> --to <harness>` validates the package,
   detects features actually present, and reports blocking invalidity separately
   from non-blocking information loss.
3. `rebinder artifact <package> --to <harness> --output <path>` creates bounded
   Markdown using only a valid package and its compatibility assessment.

The initial artifact profile is
`text/markdown; profile=rebinder.continuation.v1`. It preserves handoff
guidance, task state, recorded workspace roots, repository head and change
metadata, provenance, and recent visible user/assistant text. It summarizes
tool calls, workspace file inventory, environment markers, and declared patch
paths. It omits tool-result payloads, attachment payloads, environment values,
and remote URLs.

Artifact output uses create-new semantics, is mode `0600` on Unix, and has a
40,000-character recent-conversation budget with a 12,000-character per-message
bound. The command does not launch a provider, apply patches, recreate a
workspace, or claim that Markdown is a native session.

## Consequences

Users and future transfer adapters receive a stable, machine-readable answer
about representability before target launch. Information loss becomes testable
and reviewable, while provider-neutral continuation is useful before native
Codex-to-Claude session creation exists.

The artifact deliberately favors safety and task continuity over transcript
completeness. Provider adapters may later add richer native formats, but they
must keep the same explicit compatibility boundary.
