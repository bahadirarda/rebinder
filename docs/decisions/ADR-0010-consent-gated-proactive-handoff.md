---
type: adr
title: Consent-Gated Proactive Handoff
status: accepted
version: 0.1.0
---

# ADR-0010: Consent-Gated Proactive Handoff

## Context

A manual transfer command preserves agency but reacts too late when a source
harness is close to a subscription usage boundary. Claude Code exposes current
five-hour and seven-day Claude.ai consumption, reset times, session identity,
and workspace through its status-line JSON. Its plugin hooks can add factual
context to the active model turn. Those supported surfaces make a proactive
offer possible without scraping terminal text or estimating usage from tokens.

Starting a second interactive harness inside a Claude Code tool subprocess is
not a reliable terminal handoff. More importantly, approaching a limit does not
authorize a provider change, model spend, or process launch. The source agent
may ask, but only the user can approve the boundary crossing.

## Decision

Add a provider-neutral continuity policy with an initial Claude-to-Codex
adapter. `rebinder continuity enable claude --to codex` performs four explicit
actions:

1. verifies the target CLI has an active Codex authentication mode;
2. installs a personal Claude Code plugin under the configured Claude skills
   directory;
3. wraps the existing Claude status-line command while recording its exact
   prior value for restoration; and
4. stores 90% five-hour and 85% seven-day offer thresholds, both configurable.

The status-line bridge observes only the documented session, workspace, and
rate-limit fields. Absence of `rate_limits` is treated as no subscriber signal,
not as zero usage. A threshold crossing creates one immutable offer per source
session, target, limit kind, and provider reset window. The plugin hook injects
that offer once and asks Claude to request explicit consent. A decline suppresses
the same offer until its provider window changes.

Acceptance arms, but does not nest, a target process. When Claude was launched
through `rebinder claude`, a process-scoped launch ID binds the accepted offer
to that wrapper. The user exits the current interactive session with `/exit`;
the waiting Rebinder parent then prepares the normal transfer and opens the
bound Codex thread. A direct `claude` launch receives an explicit
`rebinder continuity resume --offer <id>` fallback. Both paths use the existing
transfer adapter, context guard, semantic deduplication, and workspace checks.

Continuity configuration, observations, offers, and state transitions live in
Rebinder's platform data directory with private Unix permissions. Offer state
uses immutable marker files so repeated status-line and hook processes do not
repeat an accepted, declined, or completed transition. Plugin files are
removed only when their managed marker and complete expected file set match.
Disable restores the prior status-line JSON only while Rebinder still owns the
configured wrapper; unexpected user changes fail closed.

## Consequences

Claude can notice an authoritative weekly or five-hour boundary and offer a
continuation before a request fails. The transfer remains user-authorized, and
the target TUI starts from Rebinder rather than inside a non-interactive hook or
tool process. Existing status-line output is retained and the plugin is
versioned with the product CalVer.

The first adapter requires a current Claude Code release that provides
subscriber `rate_limits`, an authenticated Codex CLI, and a restarted Claude
session after installation. Claude Code permits only one status-line command,
so Rebinder must wrap it. Enabling any custom status line changes Claude Code's
footer presentation. Direct Claude launches cannot receive an automatic parent
process switch and therefore use the explicit resume fallback.

Automatic provider selection, billing inference, background account polling,
and transfers without explicit consent remain out of scope. Other target
harnesses can implement the same policy contract after they provide a safe
availability probe and native transfer adapter.

## References

- [Claude Code status-line data](https://code.claude.com/docs/en/statusline)
- [Claude Code hooks reference](https://code.claude.com/docs/en/hooks)
- [Claude Code plugins](https://code.claude.com/docs/en/plugins)
- [Codex developer command reference](https://developers.openai.com/codex/cli/reference)
