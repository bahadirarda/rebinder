---
type: adr
title: Authoritative Hard-Limit Rescue
status: accepted
version: 0.1.0
---

# ADR-0011: Authoritative Hard-Limit Rescue

## Context

The proactive status-line path normally asks before Claude Code exhausts a
five-hour or seven-day usage window. That path may still miss: subscriber
fields can be absent, a threshold can be crossed between responses, or the
next provider request can fail before Claude completes the consent turn.

Claude Code documents `StopFailure` as a notification and recovery hook for API
errors. It exposes a typed `error`, including `rate_limit`, together with the
normal session, transcript-path, and working-directory fields. The event has no
decision control. Its output and exit code are ignored except for allowlisted
terminal sequences. Therefore it can authoritatively report a failure but
cannot authorize a target launch or ask the failed model to decide.

## Decision

Extend the managed Claude plugin with a `StopFailure` command hook matched only
to `rate_limit`. The hook performs a bounded local side effect:

1. verify continuity remains enabled and the configured Codex target is
   authenticated;
2. reuse an actionable proactive offer for the same active window, preserving
   any decline, or create a rescue keyed to the session, process launch, and
   transcript path metadata revision;
3. persist one private create-new rescue marker without storing provider error
   details or rendered error text; and
4. return one allowlisted OSC/BEL terminal notification.

The hook never launches Codex. After Claude exits, an enclosing `rebinder
claude` process resolves only a rescue bound to its process launch and asks a
local confirmation question whose default is no. A direct `claude` process
leaves the same rescue for `rebinder continuity rescue`. Non-interactive use
must add `--yes`, which is treated as the operator's explicit consent.

After acceptance, rescue calls the existing Claude-to-Codex transfer adapter.
That path retains source selection, context-size strategy, semantic binding,
workspace checks, and target-native opening. The public `resume` and `rescue`
commands reject the Rebinder launch environment so a Claude hook or tool
subprocess cannot nest an interactive target TUI.

## Consequences

A provider-reported rate limit now has a deterministic recovery route even if
the proactive model question could not run. Repeated matching hook events do
not create duplicate rescue state or terminal notifications. Existing declines
remain authoritative for their active reset window, and target availability is
checked again before the local question.

Rescue still depends on the user exiting Claude Code. Direct launches require
one explicit CLI command, and terminals that do not implement the allowlisted
notification sequence may only ring the bell or show Claude's original error.
Rebinder intentionally ignores overload, authentication, billing, model, and
other failure classes because they do not prove a usage-limit transition.

## References

- [Claude Code hooks reference](https://code.claude.com/docs/en/hooks)
- [Consent-gated proactive handoff](ADR-0010-consent-gated-proactive-handoff.md)
