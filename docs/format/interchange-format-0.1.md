---
type: format-specification
title: Interchange Format 0.1.0
status: draft
version: 0.1.0
---

# Interchange Format 0.1.0

## Scope

This document defines the initial directory representation of a Rebinder
session package. It covers structural validation and package integrity. Target
provider compatibility is evaluated separately by provider adapters.

## Required contents

Every package MUST contain:

- `manifest.json`
- `session.json`
- `conversation.jsonl`
- `task-state.json`
- `workspace-state.json`
- `repository-state.json`
- `handoff.md`
- `provenance.json`

Optional patch files MAY appear below `patches/` when they are declared in both
the manifest and the corresponding repository record.

## Manifest invariants

`manifest.json` identifies the format as `rebinder.session`, declares schema
version `0.1.0`, identifies the source harness adapter, and inventories every
other package file.

Each inventory entry MUST provide a media type and lowercase SHA-256 digest.
Paths MUST be relative, MUST remain within the package directory, MUST be
unique, and MUST resolve to regular files rather than symbolic links.
`manifest.json` is not included in its own inventory because doing so would
create a circular digest.

## Conversation representation

`conversation.jsonl` contains one canonical conversation item per line. An item
has a unique ID, a role, and one or more typed content blocks. Its optional
`parentId` MUST refer to another item in the same document. Blank lines are not
allowed.

The initial content block types are text, tool call, tool result, and attachment
reference. Private model reasoning is not part of the portable contract.

## Validation layers

Structural validation proceeds in this order:

1. Parse the manifest and enforce its JSON Schema.
2. Reject unsafe paths, missing files, non-regular files, symlinks, duplicates,
   and checksum mismatches.
3. Parse and schema-validate every canonical document and conversation item.
4. Enforce cross-document invariants such as source identity consistency and
   conversation parent references.

Structural validity does not imply that a target harness can represent every
field. Adapters MUST produce a separate compatibility report before import.

## Schemas

The normative machine-readable schemas are maintained in the repository-root
[`schemas/`](../../schemas/) directory and use JSON Schema Draft 2020-12.

## Stability

Version `0.1.0` is a draft implementation contract. Breaking changes require a
new schema version and corresponding migration notes.
