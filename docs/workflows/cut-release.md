---
type: workflow
title: Cut a Release
status: draft
version: 0.1.0
---

# Cut a Release

## Preconditions

- User-visible changes have reviewed Changesets.
- The automated version pull request is merged.
- `main` is clean and every required check is green.
- `Cargo.toml`, `package.json`, `Cargo.lock`, and `CHANGELOG.md` carry the same
  calendar identity.

Validate locally:

```bash
bun install --frozen-lockfile
bun run check
cargo package --locked
sh scripts/test-installer.sh
```

Read the canonical version:

```bash
bun run version:check
cargo run --locked -- --version
```

## Create the release tag

Replace `<version>` with the exact version printed above:

```bash
git switch main
git pull --ff-only
git tag -a "v<version>" -m "Rebinder v<version>"
git push origin "v<version>"
```

Never reuse, move, or replace a published release tag.

## Verify publication

Wait for the `release` workflow. Confirm:

- all five platform archives exist;
- `install.sh`, `install.ps1`, and `SHA256SUMS` exist;
- every archive contains the expected binary, schemas, and `release.json`;
- the release notes match the dated changelog section;
- GitHub displays artifact attestations; and
- a clean Linux/macOS or Windows environment can install the exact tag.

When GitHub release immutability is enabled, verify with:

```bash
gh release verify "v<version>"
```

## Optional crates.io publication

Only after native release verification, manually dispatch `publish crates.io`
with the same annotated tag and set the permanence confirmation to true. The
`crates-io` environment requires maintainer review and owns the registry token.

If registry publication fails, diagnose and retry the same tag. Do not create a
new calendar version merely to retry an operational failure.
