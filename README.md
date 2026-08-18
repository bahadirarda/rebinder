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
> and immediately open it in the session's recorded workspace. Large-session
> handoffs first create a visible continuation brief from the transferred
> context, so the opened thread has an explicit current objective and next
> action. The user stays in Rebinder for the whole operation. The reverse
> Codex and Claude sessions can also be exported into validated canonical
> packages without resuming the source session. Codex-to-Claude native launch
> remains fail-closed. Missing worktrees are reported; Rebinder does not
> recreate them.

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

Pin an exact calendar release with `REBINDER_VERSION=v0.20260818.0` on Unix or
`$env:REBINDER_VERSION='v0.20260818.0'` on Windows. Set
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

Rebinder discovers Claude sessions through Codex's external-agent API and
leaves existing Claude and Codex setup unchanged. Small transcripts use the
native session importer. For large transcripts, Rebinder creates or resumes a
native Codex thread and injects a bounded, role-preserving checkpoint through
the Codex app-server, avoiding an oversized imported history. It then asks
Codex for a concise, visible continuation brief grounded in those injected
items before opening the thread. Both paths finish by opening Codex from
Rebinder in the source workspace; users do not need to run a separate `codex
resume` command.

List the Claude sessions Codex can currently detect, including their IDs,
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

Arguments after `--` are passed to the Codex process that Rebinder opens after
binding the target thread:

```bash
rebinder transfer SESSION_ID --from claude --to codex -- --search
```

The default `--strategy auto` uses Codex's native full import for source files
up to 512 KiB. Larger sources use a context-safe handoff containing the latest
Claude compact summary and at most 40,000 characters of recent visible user and
assistant text. User and assistant roles are retained instead of flattening the
history into one prompt. Thinking, tool calls, and tool results are excluded.
The first transfer of each handoff revision starts one read-only Codex model
turn to turn that hidden prompt history into a visible continuation brief. The
activation prompt forbids tool calls and file changes, but it consumes normal
Codex model tokens. Rebinder does not start it again for an unchanged revision.
Override the decision explicitly when diagnosing compatibility:

```bash
rebinder transfer SESSION_ID --from claude --to codex --strategy handoff
rebinder transfer SESSION_ID --from claude --to codex --strategy full
```

If an older full import fails with `Codex ran out of room in the model's context
window`, leave that thread in place and rerun the transfer with the default
strategy or `--strategy handoff`. Rebinder creates or reuses a separate bounded
Codex thread for that source session.

The transfer requires an installed Codex CLI, locally stored Claude Code
session data visible to Codex, and the session's recorded workspace to still
exist. The current Codex discovery surface returns up to 50 chats from the last
30 days. Repeating a transfer resumes the strategy-specific Codex thread.
Context-safe handoffs are append-only and inject a new bounded checkpoint only
when the visible conversation or compact summary changes. Updates to an
existing handoff thread are compacted through Codex's native API before
Rebinder creates the new continuation brief and opens Codex. Their local JSONL
files also hold Rebinder's retry-safe injection, compaction, and activation
ledger, live in the platform data directory, and are private to the current
user where the platform supports file permissions. An interrupted activation
is recovered by its source-revision marker instead of creating a duplicate
brief.
Legacy flattened handoff bindings are left intact and upgraded into a fresh
role-preserving thread the first time this format is used.

## Export canonical session packages

Export a provider session into the seven-document interchange format:

```bash
rebinder export --from claude SESSION_ID --output ./claude-session
rebinder export --from codex THREAD_ID --output ./codex-session --json
```

Omit the ID in a terminal to choose from an interactive provider-native list.
In a non-interactive shell, omission selects only the newest session whose
recorded workspace matches the current directory. Codex threads can also be
listed without resuming them:

```bash
rebinder sessions codex
rebinder sessions codex --json
```

Claude export reads the local Claude Code project store directly and does not
require Codex. Codex discovery uses `thread/list`; export uses
`thread/read(includeTurns: true)`, which does not resume or subscribe to the
thread. Rebinder never edits either provider store.

Every export captures visible user/assistant text, task intent, recorded
workspace, readable Git head/change facts, a bounded handoff, and provenance.
Private reasoning, attachment payloads, environment values, remote URLs, and
tool input/output payloads are excluded by default. Common credential shapes
in visible text are best-effort redacted. The output directory must be new;
Rebinder creates it as `0700` with `0600` files on Unix, calculates all
manifest digests, and validates the completed package before reporting
success. Review exported visible text before sharing it.

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

Review the target adapter contract and calculate the information-loss boundary
for the fields actually used by a package:

```bash
rebinder capabilities claude
rebinder compatibility ./session-package --to claude
rebinder compatibility ./session-package --to codex --json
```

Create a bounded provider-neutral continuation artifact after validation and
compatibility assessment:

```bash
rebinder artifact ./session-package --to claude --output ./continuation.md
```

Artifacts preserve the handoff, task state, repository facts, recorded
workspace, provenance, and recent visible conversation text. Tool outputs,
attachments, environment values, and remote URLs are excluded; every active
loss is reported before generation. Output files are created with private
permissions on Unix and are never overwritten.

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
| Canonical export | Reads Claude locally and Codex through its read-only app-server methods, emits a validated package, and never resumes or mutates the source session |
| Harness commands | Native arguments, interactive streams, and process status are preserved |
| Claude discovery | Lists Codex-supported local Claude sessions, sizes, and recommended strategies without printing transcript content |
| Claude to Codex | Selects interactively or by ID, uses Codex-native import or thread APIs, and opens the native thread from Rebinder in the recorded workspace |
| Context guard | Injects bounded compact-summary and recent-message items with their user/assistant roles preserved, then creates a visible continuation brief for source transcripts larger than 512 KiB |
| Repeat transfer | Reuses the strategy-specific thread, ignores metadata-only source churn, and performs compaction and visible activation once per meaningful handoff revision |
| Worktrees | Reuses an existing recorded worktree; missing workspace paths fail closed |
| Compatibility | Declares Codex and Claude continuation capabilities and reports package-specific preserved, summarized, omitted, or blocking state in human/JSON form |
| Continuation artifact | Produces bounded Markdown continuation state from a validated package without tool output, environment values, attachment payloads, or remote URLs |
| Codex to Claude | Native target launch is not implemented yet; exits closed with code `2` |

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
asks the local Codex app-server to import only the selected small session or to
inject the bounded role-preserving checkpoint for a large one; it does not
select settings, credentials, plugins, skills, or MCP configuration. Rebinder
starts one read-only, no-tool model turn to make a new handoff revision visible,
which consumes Codex model tokens. Rebinder never prints handoff content and
rejects symlinked handoff targets. It fails
closed on invalid structure, unsafe paths, missing workspaces, integrity
failures, and provenance mismatches. Report vulnerabilities through the private
process in [SECURITY.md](SECURITY.md), not a public issue.

Provider-neutral continuation artifacts are also sensitive. Rebinder validates
their package first, excludes tool-result payloads and environment values,
creates them without overwriting an existing path, and uses mode `0600` on
Unix. Review an artifact before sharing it because visible conversation and
handoff text may still contain private project information.

Canonical exports use the same sensitive-data boundary. Provider-private
reasoning and payloads are excluded and provenance records redaction counts,
but visible user and assistant text is intentionally portable and automated
credential redaction is best effort rather than a substitute for review.

## License

Rebinder is available under the [MIT License](LICENSE).
