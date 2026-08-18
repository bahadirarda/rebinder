# Security Policy

## Supported versions

Rebinder is pre-stable. Security fixes target the latest published calendar
release. Older foundation releases may not receive backports.

## Reporting a vulnerability

Do not open a public issue for credentials exposure, path traversal, symlink
escape, package validation bypass, installer compromise, or unsafe provider
session handling.

Use GitHub private vulnerability reporting for the repository. Include the
affected release, platform, reproduction steps, impact, and any proposed
mitigation. Do not attach real session packages, tokens, or private transcripts;
use a minimized synthetic fixture.

Claude-to-Codex transfer uses the installed Codex app-server. Small sessions go
through its external-agent importer. For large sessions, Rebinder stores a
bounded handoff containing the latest compact summary and recent visible
messages with their user and assistant roles retained, then injects it through
Codex's native thread API. Meaningful updates to an existing handoff thread are
compacted through the native API before Codex opens. Each new handoff revision
then starts one Codex model turn with read-only sandboxing, approvals disabled,
and an instruction not to call tools or modify files. That turn produces the
visible continuation brief and consumes normal Codex model tokens. A
source-revision marker and append-only activation ledger prevent duplicate
turns after an interrupted write. On Unix, Rebinder sets the handoff directory
to `0700` and files to `0600`; it rejects symlinked handoff files and never
prints their contents. Other platform access controls still apply.

Rebinder does not copy thinking blocks, tool calls, or tool results into a
context-safe handoff. It does not import provider configuration or write Claude
or Codex session files directly. Treat original sessions, handoff files,
session IDs, thread bindings, titles, working-directory paths, and import logs
as potentially sensitive data. Removing a handoff file removes Rebinder's
bounded local copy and retry binding, but does not remove its native Codex
thread.

Capability assessment validates the package before reading portable state and
reports active information-loss boundaries without launching a provider.
Provider-neutral continuation artifacts deliberately omit tool-result payloads,
attachments, environment values, and repository remote URLs. They retain
visible conversation, handoff, task, workspace, repository, and provenance
facts, which can still be sensitive. Artifact output never overwrites an
existing path and is created with mode `0600` on Unix. Users must review the
result before sharing or passing it to another harness.

Canonical export is also intentionally lossy at the security boundary. Claude
export reads regular files from the configured local project store; Codex
export uses `thread/list` and non-resuming `thread/read` requests. Rebinder does
not write provider stores. Exported packages omit private reasoning,
attachments, environment values, remote URLs, and provider tool payloads.
Visible conversation is retained and common credential patterns are redacted
best effort, so packages still require human review before sharing. Export
requires a new output path, uses `0700` directories and `0600` files on Unix,
binds files with SHA-256, and runs the normal validator before success.

Codex-to-Claude transfer composes that export with a bounded continuation
artifact. Rebinder passes the artifact to Claude Code through a process-unique
temporary file that is mode `0600` on Unix and removes it when preparation or
the interactive invocation ends normally. The appended context wraps all
portable content in an explicit untrusted-history boundary and escapes its
closing delimiter. The native transcript retains only the activation marker,
Claude's visible response, and whatever Claude Code normally persists.

The activation request tells Claude not to call tools or modify files for its
first response, but this is a model instruction rather than an operating-system
sandbox. It consumes normal Claude model tokens. Rebinder never passes
dangerous permission flags unless the user explicitly supplies them after
`--`, and it rejects session-selection flags that would break the deterministic
binding. Treat source text as untrusted: fencing reduces instruction confusion
but does not turn historical content into trusted policy.

Missing-worktree recovery is disabled unless the user supplies
`--recover-worktree`. It is a deliberate filesystem mutation, limited to an
exact path that one existing local repository still reports through `git
worktree list --porcelain`. Rebinder rejects existing targets, missing parents,
immediate symlink parents, locked registrations, unavailable commits, broad or
ambiguous repository discovery, and attached-branch changes observed during
creation. Git is invoked directly without a shell; recovery does not fetch,
clone, unlock, overwrite, or contact a remote. After creation Rebinder verifies
HEAD, attached branch state, and the common Git directory before starting a
provider. A removed worktree's uncommitted and ignored files are not recoverable
through this feature.

Proactive continuity is disabled until the user runs `rebinder continuity
enable claude --to codex`. Enablement verifies `codex login status`, installs a
personal Claude Code plugin, and replaces Claude's single status-line command
with a Rebinder wrapper. The wrapper replays the exact prior command with the
same JSON input, records only documented session/workspace/rate-limit fields,
and restores the prior JSON on disable. A changed status-line value or unknown
file in the managed plugin directory causes disablement to fail rather than
overwrite user state. Claude Code's own custom-status-line footer behavior
still applies.

The plugin runs unsandboxed with the user's Claude Code privileges, as Claude
hooks normally do. Rebinder hook output contains local usage percentages,
reset times, session identity, and a deterministic offer ID; it never includes
transcript contents or credentials. Policy, observation, offer, and transition
files use the Rebinder platform data directory and modes `0700`/`0600` on Unix.
Session IDs, paths, usage, and reset times remain sensitive local metadata.

A threshold creates only an offer. The plugin requires an explicit affirmative
response before it records acceptance, and it does not launch a target TUI from
a hook or Claude tool subprocess. With `rebinder claude`, Codex starts only
after the user exits the source process; direct Claude launches require the
printed `rebinder continuity resume --offer <id>` fallback. The normal transfer
adapter then rechecks source/workspace constraints. Rebinder cannot prove a
specific paid entitlement from stored credentials: Claude's subscriber-only
rate-limit field and Codex's active authentication mode are availability
signals, not billing guarantees.

Hard-limit rescue listens only for Claude Code's documented `StopFailure`
event with the exact `rate_limit` error type. It ignores other API failures and
does not persist `error_details` or the rendered provider error. The rescue ID
uses session/workspace identity plus transcript path metadata and a
process-scoped launch binding; Rebinder does not read transcript contents in the
failure hook. Repeated matching events create one rescue marker and one
allowlisted OSC/BEL terminal notification. An unexpired decline suppresses the
same window even after a failure.

`StopFailure` has no decision authority, so its hook cannot accept or launch a
target. After Claude exits, an interactive Rebinder parent asks with a safe
negative default. Direct launches use `rebinder continuity rescue`, and
non-interactive operators must provide `--yes` as explicit consent. The CLI
rejects both rescue and accepted-offer resume while it still has the
Rebinder-owned Claude launch environment, preventing a target TUI from nesting
inside the source process. Only after consent does the existing transfer
adapter revalidate target authentication, source selection, and workspace
constraints.

You should receive an acknowledgement within seven days. Publication and fix
timing depend on severity and whether a coordinated provider disclosure is
required.
