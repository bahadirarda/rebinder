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

## Target-Native Import Bridge

A transfer adapter that delegates provider-specific conversion and session
store writes to a supported target-provider API. Rebinder's initial
Claude-to-Codex path uses the Codex app-server external-agent migration API and
never writes Codex session files directly.

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
recent visible messages. Rebinder uses it as an input to the target-native
import bridge when a complete transcript would risk exhausting the target
context window. The handoff is an implementation strategy inside a
cross-harness transfer, not a synonym for the transfer itself.

## Provenance

Information describing the entities, activities, agents, tools, and transformations involved in producing session data.

## Compatibility Report

A report describing structural validity, supported capabilities, unsupported fields, and declared information loss for a target adapter.
