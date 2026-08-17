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
messages, then injects it through Codex's native thread API. On Unix, Rebinder
sets the handoff directory to `0700` and files to `0600`; it rejects symlinked
handoff files and never prints their contents. Other platform access controls
still apply.

Rebinder does not copy thinking blocks, tool calls, or tool results into a
context-safe handoff. It does not import provider configuration or write Claude
or Codex session files directly. Treat original sessions, handoff files,
session IDs, thread bindings, titles, working-directory paths, and import logs
as potentially sensitive data. Removing a handoff file removes Rebinder's
bounded local copy and retry binding, but does not remove its native Codex
thread.

You should receive an acknowledgement within seven days. Publication and fix
timing depend on severity and whether a coordinated provider disclosure is
required.
