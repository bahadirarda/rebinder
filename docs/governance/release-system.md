---
type: governance
title: Release System
status: draft
version: 0.1.0
---

# Release System

## Purpose

The release system gives each Rebinder release one identity across Cargo,
Changesets metadata, `Cargo.lock`, changelog, Git tags, binary archives,
installers, and build provenance.

## Product version

Rebinder uses a calendar identity:

```text
0.YYYYMMDD.REVISION
```

- `0` identifies the pre-stable product epoch.
- `YYYYMMDD` is the release source commit date.
- `REVISION` begins at `0` on a new day and increments for another release
  sourced on that date.
- Release dates MUST NOT move backward.

Cargo requires a three-number version field, so the calendar identity remains
machine-compatible with the Rust ecosystem without using compatibility-driven
version increments as the public release number.

The canonical value is `[package].version` in `Cargo.toml`. `package.json` is a
private Changesets proxy and `Cargo.lock` repeats the value only where their
formats require it. Repository validation rejects drift.

## Schema version independence

Product CalVer and interchange schema versions serve different purposes. A
product release may preserve schema `0.1.0`, and a schema revision does not
derive from a calendar date. Every package declares its schema version in
`manifest.json`; the CLI declares the schema versions it supports.

## Release intent

Every user-visible CLI, format, provider adapter, installer, or compatibility
change MUST include a Changeset. Patch, minor, and major declarations preserve
reviewed compatibility impact, but they do not select the published number.

The automated version pull request consumes pending Changesets, calculates the
next calendar identity from the release source commit, synchronizes release
metadata, updates the root changelog, and runs the full validation suite.

## Native artifacts

An annotated tag named `v<version>` triggers release builds. The tag MUST match
the canonical Cargo version and point to a commit reachable from `main`.

| Artifact | Platform |
| --- | --- |
| `rebinder-v<version>-x86_64-unknown-linux-gnu.tar.gz` | Linux x86-64 |
| `rebinder-v<version>-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 |
| `rebinder-v<version>-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `rebinder-v<version>-aarch64-apple-darwin.tar.gz` | macOS Apple silicon |
| `rebinder-v<version>-x86_64-pc-windows-msvc.zip` | Windows x86-64 |
| `install.sh` | Verified Linux and macOS installer |
| `install.ps1` | Verified Windows installer |
| `SHA256SUMS` | Digest manifest for every archive and installer |

Each platform archive contains the executable, README, license, normative
schemas, and `release.json`. Release metadata records the calendar version,
annotated tag, full commit, commit date, Rust target, and a build identity shaped
as `<version>+sha.<short-sha>`.

The release workflow creates GitHub artifact attestations for every downloadable
file and stages a draft release until all artifacts and release notes exist.

## Installation trust boundary

The Unix and Windows installers:

1. resolve an exact calendar tag;
2. download the target archive and release-owned checksum manifest;
3. verify SHA-256 before extraction;
4. validate embedded release name, version, tag, and target;
5. stage replacement without following destination symlinks or reparse points;
6. smoke-test the exact installed version; and
7. restore the previous binary if activation fails.

The installer scripts are convenience channels. The archive, checksum,
`release.json`, annotated tag, and GitHub attestation define release identity.

## Publication sequence

1. Merge changes with reviewed Changesets.
2. Merge the automated calendar version pull request after all gates pass.
3. Create an annotated tag: `git tag -a v<version> -m "Rebinder v<version>"`.
4. Push the tag and allow the protected release workflow to publish native
   artifacts.
5. Verify the GitHub Release and installer acceptance.
6. Optionally dispatch the environment-protected crates.io workflow with the
   same tag and explicit permanence confirmation.

crates.io publication is never implied by a GitHub Release because registry
versions are permanent.

## Failure policy

A malformed or lightweight tag, version drift, tag outside `main`, missing
changelog section, failed test, invalid package, installer regression, native
build failure, checksum failure, or attestation failure blocks publication. A
failed release does not rewrite an existing version or move its tag.
