---
type: standards-profile
title: OKF 0.2 Project Profile
status: draft
version: 0.1.0
---

# OKF 0.2 Project Profile

This profile governs the Rebinder documentation bundle.

## Status of this document

This project follows the official Open Knowledge Format (OKF) v0.2 specification published by Google Cloud, with project-specific additions explicitly marked as profile requirements. OKF v0.2 is additive and backward-compatible with v0.1, while introducing optional trust, provenance, lifecycle, and attestation signals.

## Base OKF v0.2 rules adopted

- Knowledge documents use Markdown.
- Non-reserved Markdown concept files carry parseable YAML frontmatter.
- Every concept file has a non-empty `type` field.
- `index.md` is the directory index.
- `log.md` records dated changes.
- Links use ordinary Markdown links.
- Documents remain inspectable, diffable, and maintainable in Git.
- `type` remains the only always-required frontmatter field for concept documents.
- Custom frontmatter keys are preserved and consumers remain permissive.
- `generated` records how and when current content was produced.
- `verified` records independent confirmations; it is distinct from generation.
- `sources` records provenance and is the preferred mechanism for per-claim attribution through keyed Markdown footnotes.
- `status` expresses lifecycle, using `draft`, `stable`, or `deprecated`.
- `stale_after` provides an absolute freshness boundary.
- `Attested Computation` is used only where a sanctioned computation and runtime attestation are required.

## Project profile requirements

Every non-reserved concept document in this project MUST include:

- `type`
- `title`
- `status`
- `version`

The following OKF v0.2 fields SHOULD be used when applicable:

- `generated`
- `verified`
- `sources`
- `stale_after`
- `status`

The project MUST NOT add frontmatter to `log.md`. The bundle-root `index.md` uses only the OKF-defined `okf_version` declaration; its body is the authoritative directory listing.

Normative documents SHOULD additionally include:

- scope
- terminology
- requirements or invariants
- examples where applicable
- references
- change history through `log.md`

## Normative language

The terms `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` are interpreted according to common RFC-style usage. They are used only in normative documents.

## Validation policy

Structural validation and semantic validation are separate concerns. A document may be structurally valid while still requiring domain review. Future automation MAY validate frontmatter, required sections, links, and schema references.

## References

- [OKF specification status](https://openknowledgeformat.com/spec-status)
- [OKF v0.2 adds trust signals](https://cloud.google.com/blog/products/data-analytics/okf-v0-2-adds-trust-signals)
- [OKF v0.2 specification and reference implementations](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf)
- [Open Knowledge Format FAQ](https://okf.md/faq/)
- [Open Knowledge Foundation: What is open?](https://okfn.org/en/library/what-is-open/)
