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

Claude-to-Codex transfer sends the selected local session through the installed
Codex app-server's external-agent importer. Rebinder does not print transcript
content, import provider configuration, or write provider session files
directly. Treat session IDs, titles, working-directory paths, and import logs as
potentially sensitive metadata.

You should receive an acknowledgement within seven days. Publication and fix
timing depend on severity and whether a coordinated provider disclosure is
required.
