<p align="center">
  <img src="docs/assets/rebinder-hero.png" alt="Rebinder brand mark and wordmark" width="100%" />
</p>

<h1 align="center">Rebinder</h1>

<p align="center">
  Cross-harness session continuity for coding agents.
</p>

<p align="center">
  <a href="https://github.com/bahadirarda/rebinder/releases">Releases</a> ·
  <a href="index.md">Documentation</a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="CONTRIBUTING.md">Contributing</a> ·
  <a href="SUPPORT.md">Support</a>
</p>

<p align="center">
  <a href="https://github.com/bahadirarda/rebinder/actions/workflows/ci.yml"><img alt="ci" src="https://img.shields.io/github/actions/workflow/status/bahadirarda/rebinder/ci.yml?branch=main&style=flat-square&label=ci&labelColor=0b0f19&color=2563eb"></a>
  <a href="https://github.com/bahadirarda/rebinder/releases"><img alt="latest release" src="https://img.shields.io/github/v/release/bahadirarda/rebinder?display_name=tag&style=flat-square&label=release&labelColor=0b0f19&color=22d3ee"></a>
  <img alt="foundation mvp" src="https://img.shields.io/badge/status-foundation_MVP-2563eb?style=flat-square&labelColor=0b0f19">
  <img alt="calendar versioning" src="https://img.shields.io/badge/versioning-CalVer_0.YYYYMMDD.N-22d3ee?style=flat-square&labelColor=0b0f19">
  <img alt="rust 1.92" src="https://img.shields.io/badge/rust-1.92-e8e2d5?style=flat-square&labelColor=0b0f19">
</p>

<p align="center">
  Inspect, validate, and safely carry portable coding-agent session state across harness boundaries.
</p>

> [!IMPORTANT]
> Rebinder is a foundation MVP. Canonical package validation, inspection, and
> native Codex/Claude command passthrough are implemented. Cross-harness
> `transfer` has a stable CLI contract but remains intentionally unavailable
> until both provider adapters can report compatibility and information loss.

## Install

Published releases provide checksum-verified native binaries for Linux, macOS,
and Windows.

Linux or macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/bahadirarda/rebinder/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/bahadirarda/rebinder/releases/latest/download/install.ps1 | iex
```

Pin an exact calendar release with `REBINDER_VERSION=v0.20260817.0` on Unix or
`$env:REBINDER_VERSION='v0.20260817.0'` on Windows. Set
`REBINDER_INSTALL_DIR` to choose the destination.

Every installer downloads the platform archive and release-owned `SHA256SUMS`,
verifies the archive before extraction, validates its `release.json` identity,
stages the replacement, and checks the installed CLI version before committing
the update.

To build the current source:

```bash
git clone https://github.com/bahadirarda/rebinder.git
cd rebinder
cargo install --locked --path .
```

## Use

Run native harness commands through Rebinder without changing their arguments:

```bash
rebinder codex resume --last
rebinder claude --continue
```

Validate or inspect a portable session package without starting an agent:

```bash
rebinder validate ./session-package
rebinder inspect ./session-package
rebinder inspect ./session-package --json
```

The cross-harness command is explicit about direction and keeps target-native
arguments after `--`:

```bash
rebinder transfer SESSION_ID --from claude --to codex -- --full-auto
rebinder transfer SESSION_ID -f codex -t claude -- --permission-mode acceptEdits
```

`transfer` currently fails closed with exit code `2`; it will become operational
only after export, redaction, compatibility reporting, and target artifact
generation are implemented.

## What the foundation proves

| Boundary | Current behavior |
| --- | --- |
| Package structure | JSON Schema Draft 2020-12 validation for every canonical document |
| Integrity | SHA-256 inventory verification before inspection |
| Filesystem safety | Relative-path confinement, regular-file enforcement, and symlink rejection |
| Conversation graph | Unique IDs and valid parent references |
| Provenance | Source adapter identity, transformations, export time, and redactions |
| Harness commands | Native arguments, interactive streams, and process status are preserved |
| Compatibility | Contract defined; provider capability engines remain pending |
| Cross-harness transfer | CLI contract defined; execution remains pending |

The initial package format is documented in the
[Interchange Format 0.1.0](docs/format/interchange-format-0.1.md) specification.
Package schema versions are independent from product releases.

## Releases and versions

Rebinder uses the same calendar release system as pkgshift:

```text
0.YYYYMMDD.REVISION
```

`0.20260817.0` is the first release sourced on 2026-08-17;
`0.20260817.1` is another release from that date. A new day resets the revision
to `0`.

User-visible changes carry a Changeset. Merging the automated version pull
request synchronizes Cargo, Bun release metadata, `Cargo.lock`, and the
changelog. An annotated `v<version>` tag then builds five native archives,
`release.json`, verified installers, `SHA256SUMS`, and GitHub artifact
attestations. crates.io publication is a separate, manually confirmed workflow.

See the complete [release system](docs/governance/release-system.md).

## Develop

Requirements: Rust `1.92.0` and Bun `1.3.14`.

```bash
bun install --frozen-lockfile
bun run check
bun run build
```

Useful commands:

```bash
bun run changeset          # record user-visible release intent
bun run changeset:status   # inspect pending release intent
bun run version:next       # preview the next calendar identity
cargo run -- --help
sh scripts/test-installer.sh
```

## Security

Session packages may contain sensitive workspace and conversation state.
Rebinder fails closed on invalid structure, unsafe paths, symlinks, integrity
failures, and provenance mismatches. Credentials are never an automatic
transfer target. Report vulnerabilities through the private process in
[SECURITY.md](SECURITY.md), not a public issue.

## License

Rebinder is available under the [MIT License](LICENSE).
