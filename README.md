<p align="center">
  <img src="docs/assets/rebinder-hero.png" alt="Rebinder brand mark and wordmark" width="100%" />
</p>

<h1 align="center">Rebinder</h1>

<p align="center">
  Cross-harness session continuity for coding agents.
</p>

<p align="center">
  <a href="https://bahadirarda.github.io/rebinder/">Website</a> ·
  <a href="https://github.com/bahadirarda/rebinder/releases">Releases</a> ·
  <a href="index.md">Documentation</a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="CONTRIBUTING.md">Contributing</a> ·
  <a href="SUPPORT.md">Support</a>
</p>

<p align="center">
  <a href="https://github.com/bahadirarda/rebinder/actions/workflows/ci.yml"><img alt="ci" src="https://img.shields.io/github/actions/workflow/status/bahadirarda/rebinder/ci.yml?branch=main&style=flat-square&label=ci&labelColor=0b0f19&color=2563eb"></a>
  <a href="https://github.com/bahadirarda/rebinder/releases"><img alt="latest release" src="https://img.shields.io/github/v/release/bahadirarda/rebinder?display_name=tag&style=flat-square&label=release&labelColor=0b0f19&color=22d3ee"></a>
  <img alt="Claude to Codex transfer MVP" src="https://img.shields.io/badge/status-transfer_MVP-2563eb?style=flat-square&labelColor=0b0f19">
  <img alt="calendar versioning" src="https://img.shields.io/badge/versioning-CalVer_0.YYYYMMDD.N-22d3ee?style=flat-square&labelColor=0b0f19">
  <img alt="rust 1.92" src="https://img.shields.io/badge/rust-1.92-e8e2d5?style=flat-square&labelColor=0b0f19">
</p>

<p align="center">
  Inspect, validate, and safely carry portable coding-agent session state across harness boundaries.
</p>

> [!IMPORTANT]
> Rebinder can transfer a local Claude Code session into a native Codex thread
> and immediately resume it in the session's recorded workspace. The reverse
> Codex-to-Claude direction and provider-neutral compatibility reporting remain
> fail-closed. Missing worktrees are reported; Rebinder does not recreate them.

## Install

Published releases provide checksum-verified native binaries for Linux, macOS,
and Windows.

Visit the [Rebinder website](https://bahadirarda.github.io/rebinder/) for the
product overview, platform installers, current capability boundary, and project
documentation.

Linux or macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/bahadirarda/rebinder/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/bahadirarda/rebinder/releases/latest/download/install.ps1 | iex
```

Pin an exact calendar release with `REBINDER_VERSION=v0.20260817.5` on Unix or
`$env:REBINDER_VERSION='v0.20260817.5'` on Windows. Set
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

## Transfer Claude Code to Codex

Rebinder delegates conversion to Codex's external-agent session importer. It
selects only the session migration item, leaves existing Claude and Codex setup
unchanged, receives the imported Codex thread ID, and resumes that thread with
the native Codex CLI. Large transcripts are bounded before import so a resumed
thread does not immediately exhaust Codex's context window.

List the Claude sessions Codex can currently import, including their IDs,
recorded workspaces, states, and recommended transfer strategies:

```bash
rebinder sessions claude
rebinder sessions claude --json
```

Open the interactive session picker, move with the arrow keys, and press Enter:

```bash
rebinder transfer --from claude --to codex
```

Press Esc to cancel without importing. To bypass the picker, transfer a
specific session by ID:

```bash
rebinder transfer SESSION_ID --from claude --to codex
```

In a non-interactive shell, omitting the ID selects the most recently updated
session whose recorded workspace or Git worktree matches the current directory:

```bash
rebinder transfer --from claude --to codex
```

Arguments after `--` are passed to `codex resume` after the imported thread ID:

```bash
rebinder transfer SESSION_ID --from claude --to codex -- --search
```

The default `--strategy auto` uses Codex's native full import for source files
up to 512 KiB. Larger sources use a context-safe handoff containing the latest
Claude compact summary and at most 40,000 characters of recent visible user and
assistant text. Thinking, tool calls, and tool results are excluded. Override
the decision explicitly when diagnosing compatibility:

```bash
rebinder transfer SESSION_ID --from claude --to codex --strategy handoff
rebinder transfer SESSION_ID --from claude --to codex --strategy full
```

If an older full import fails with `Codex ran out of room in the model's context
window`, leave that thread in place and rerun the transfer with the default
strategy or `--strategy handoff`. Rebinder creates or reuses a separate bounded
Codex thread for that source session.

The transfer requires an installed Codex CLI, locally stored Claude Code
session data visible to Codex's importer, and the session's recorded workspace
to still exist. The current Codex import surface discovers up to 50 chats from
the last 30 days. Repeating a transfer resumes the existing imported Codex
thread. Context-safe handoffs are append-only and add a new bounded checkpoint
when the Claude source changes. Their local JSONL files live in Rebinder's
platform data directory and are private to the current user where the platform
supports file permissions.

## Other commands

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

Codex-to-Claude transfer remains unavailable and exits with code `2` rather
than pretending an incompatible target artifact was created.

## What the MVP delivers

| Boundary | Current behavior |
| --- | --- |
| Package structure | JSON Schema Draft 2020-12 validation for every canonical document |
| Integrity | SHA-256 inventory verification before inspection |
| Filesystem safety | Relative-path confinement, regular-file enforcement, and symlink rejection |
| Conversation graph | Unique IDs and valid parent references |
| Provenance | Source adapter identity, transformations, export time, and redactions |
| Harness commands | Native arguments, interactive streams, and process status are preserved |
| Claude discovery | Lists Codex-supported local Claude sessions, sizes, and recommended strategies without printing transcript content |
| Claude to Codex | Selects interactively or by ID, imports through Codex, and resumes the native thread in the recorded workspace |
| Context guard | Uses a bounded summary-and-recent-message handoff for source transcripts larger than 512 KiB |
| Repeat transfer | Reuses the strategy-specific thread and appends a bounded checkpoint when a handoff source changes |
| Worktrees | Reuses an existing recorded worktree; missing workspace paths fail closed |
| Compatibility | General provider capability and information-loss reports remain pending |
| Codex to Claude | Not implemented; exits closed with code `2` |

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

Session packages, provider session stores, and context-safe handoff files may
contain sensitive workspace and conversation state. Claude-to-Codex transfer
asks the local Codex app-server to import only the selected session; it does not
select settings, credentials, plugins, skills, or MCP configuration. Rebinder
never prints handoff content and rejects symlinked handoff targets. It fails
closed on invalid structure, unsafe paths, missing workspaces, integrity
failures, and provenance mismatches. Report vulnerabilities through the private
process in [SECURITY.md](SECURITY.md), not a public issue.

## License

Rebinder is available under the [MIT License](LICENSE).
