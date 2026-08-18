---
type: terminology
title: Domain Terminology
status: draft
version: 0.1.0
---

# Domain Terminology

## OKF terms

### Knowledge Bundle

A self-contained, hierarchical collection of knowledge documents. In this project, the documentation directory is an OKF knowledge bundle.

### Concept

A single unit of knowledge represented by one Markdown document with YAML frontmatter.

### Concept ID

The bundle-relative path of a concept document without its `.md` suffix.

### Frontmatter

The YAML metadata block at the beginning of a concept document.

### Body

The Markdown content after the frontmatter block.

### Source

Material from which a concept derives, recorded in the `sources` frontmatter field.

### Credibility Signal

An objective per-source fact such as `author`, `usage_count`, or `last_modified`. OKF records signals; consumers infer trust rather than receiving a prescribed score.

### Actor

An identity used by `generated.by` and `verified[].by`. This project follows the OKF convention: `<producer>/<version>`, `human:<id>`, or `process:<id>`.

### Trust Tier

A consumer-derived classification based on `verified`: unverified, machine-confirmed, or human-reviewed.

### Attested Computation

A concept that defines a sanctioned computation and the means for a consumer to verify that a runtime execution produced the claimed result.

### Executor

The run instructions or code that executes an Attested Computation and returns a receipt.

### Receipt

Runtime evidence returned by an executor. It is not stored in the OKF bundle by default.

### Attester

Deterministic, non-LLM code that inspects a receipt and returns an attestation verdict.

## Coding Agent Harness

An execution environment that manages an agent's model interaction, tools, permissions, context, and runtime lifecycle.

## Agent Session

A provider-scoped sequence of interactions and state associated with a task and workspace.

## Session Resume

Continuation of a session within the same provider harness.

## Cross-Harness Session Transfer

Moving session information from one coding agent harness to another.

## Session Migration

Transformation of a provider-specific session representation into another representation, potentially with declared information loss.

## Session Portability

The property that session information can be transferred and used across harness implementations.

## Canonical Session Model

The provider-neutral data model defined by this project.

## Agent Session Interchange Format

The serialized representation of the canonical session model used for exchange.

## Provider Adapter

A component that imports provider-specific data into the canonical model and exports canonical data for a provider.

## Target-Native Transfer Bridge

A transfer adapter that delegates provider-specific conversion and session
store writes to supported target-provider APIs. Rebinder's initial
Claude-to-Codex path uses Codex app-server external-agent discovery, native full
import, and native thread APIs; it never writes Codex session files directly.

## Recorded Workspace

The working directory stored with the source session. It may be a repository
root, nested directory, or Git worktree. The initial transfer MVP reuses this
directory when it exists and fails closed when it does not.

## Harness Command Facade

The Rebinder CLI namespace that transparently launches a provider CLI and
forwards its native arguments, standard streams, and exit status. For example,
`rebinder codex resume --last` delegates to `codex resume --last`.

## Checkpoint

A snapshot of runtime or workflow state at a defined point in execution. A checkpoint is not synonymous with a portable session package.

## Handoff Summary

A structured summary intended to help another agent or operator continue work.

## Context-Safe Handoff

A bounded, derived source-session checkpoint containing a compact summary and
recent visible messages. Rebinder retains their user and assistant roles,
detects meaningful changes from the bounded semantic content, and injects them
through the target's native thread API when a complete transcript would risk
exhausting the target context window. Meaningful repeat updates are compacted
before the target opens. The handoff is an implementation strategy inside a
cross-harness transfer, not a synonym for the transfer itself.

## Continuity Activation

The single target-model turn that follows a new bounded handoff revision. It
uses only the injected source history to create a visible continuation brief
with the current objective, verified state, decisions, and next action.
Rebinder starts it with read-only sandboxing and no approval escalation, marks
it with the source semantic revision for retry recovery, and does not repeat it
for an unchanged revision.

## Provenance

Information describing the entities, activities, agents, tools, and transformations involved in producing session data.

## Compatibility Report

A report describing structural validity, supported capabilities, unsupported fields, and declared information loss for a target adapter.

## Provider Capability Declaration

A machine-readable target-adapter contract classifying each canonical state
category as preserved, summarized, or omitted by a specific continuation
artifact format.

## Continuation Artifact

A bounded target-facing representation produced from a validated canonical
package. The initial provider-neutral artifact is Markdown containing handoff,
task, workspace, repository, provenance, and recent visible conversation state.
It is not a native provider session and does not claim exact behavioral
equivalence.
