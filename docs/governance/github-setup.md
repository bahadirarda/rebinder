---
type: governance
title: GitHub Repository Setup
status: draft
version: 0.1.0
---

# GitHub Repository Setup

## Purpose

Workflow files define repository automation, but several trust controls live in
GitHub settings. A maintainer MUST complete this checklist before publishing the
first release.

## Repository features

Enable:

- Issues with the repository-owned forms
- Discussions for support and design questions
- Private vulnerability reporting
- Dependabot alerts and security updates
- Secret scanning and push protection where available
- Release immutability

Create the issue labels referenced by the forms: `bug`, `enhancement`, and
`triage`.

Set `docs/assets/rebinder-hero.png` as the starting point for repository social
preview artwork after producing a platform-appropriate crop.

## Main branch ruleset

Protect `main` with a repository ruleset that:

- requires pull requests;
- requires the current CI jobs;
- requires the branch to be up to date before merge;
- dismisses stale approvals after new commits;
- blocks force pushes and deletion;
- requires conversation resolution; and
- permits only maintainers to bypass for incident recovery.

The initial required checks are `release-metadata`, `rust`, `unix-installer`,
`windows-installer`, and `conventional-title` for pull requests. The
`changeset-status` check is required for ordinary user-visible pull requests but
is intentionally skipped for automated version pull requests.

## Tag ruleset

Protect tags matching `v0.*.*`. Only maintainers may create or delete release
tags. The release workflow additionally rejects lightweight tags, malformed
calendar identities, and commits outside `main`.

## Actions permissions

Use read-only workflow permissions by default. Allow GitHub Actions to create
pull requests so the pinned Changesets action can maintain the version pull
request. Individual publish jobs request write, identity-token, and attestation
permissions only where required.

Pin third-party actions to reviewed commit SHAs when they are not maintained by
GitHub. Dependabot monitors action revisions.

## Environments and secrets

Create a `crates-io` environment with required maintainer review. Store
`CARGO_REGISTRY_TOKEN` only in that environment and restrict deployment to
protected calendar tags.

GitHub native releases require no long-lived publishing secret. They use the
job-scoped token and GitHub's identity-token-based artifact attestation.

## First-release readiness

Before creating `v0.20260817.0`:

1. Confirm the canonical repository remote is
   `https://github.com/bahadirarda/rebinder`.
2. Merge the initial source through the protected `main` path.
3. Confirm every required CI job is green on the exact release commit.
4. Enable release immutability and private vulnerability reporting.
5. Create the annotated tag by following the release runbook.
6. Verify all five archives, installers, `SHA256SUMS`, and attestations are
   attached before announcing the release.
