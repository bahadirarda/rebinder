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

You should receive an acknowledgement within seven days. Publication and fix
timing depend on severity and whether a coordinated provider disclosure is
required.
