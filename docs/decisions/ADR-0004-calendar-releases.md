---
type: adr
title: Calendar Releases and Native Distribution
status: accepted
version: 0.1.0
---

# ADR-0004: Calendar Releases and Native Distribution

## Context

Rebinder needs chronological pre-stable releases, reviewable release intent,
native installation without a Rust toolchain, and a release identity that can be
traced to source. Interchange schema compatibility must not be inferred from the
product release number.

## Decision

Use `0.YYYYMMDD.REVISION` as the product release identity. Retain Changesets as
the user-visible change ledger while calculating the published number from the
release source commit date.

Distribute native GitHub Release archives for five initial targets. Include
`release.json`, normative schemas, SHA-256 checksums, Unix and Windows installers,
and GitHub artifact attestations. Keep crates.io publication as a separate,
manually confirmed workflow.

Version the canonical interchange schema independently.

## Consequences

Positive:

- Release chronology is visible and deterministic.
- Users install a native binary without Cargo.
- Archives carry exact source and target provenance.
- Registry publication cannot happen as a side effect of ordinary release CI.
- Product release cadence does not make false schema-compatibility claims.

Trade-offs:

- Some dependency tools interpret the numeric calendar identity using their
  normal three-component comparison rules.
- Release automation requires Bun for Changesets even though the product is
  implemented in Rust.
- Installers and five native build targets increase the validation surface.
