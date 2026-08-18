---
name: handoff
description: Review and explicitly accept or decline a pending Rebinder continuity offer.
disable-model-invocation: true
---

Run `rebinder continuity status --json` and inspect the pending offer for the
current Claude Code session. Explain the source, target, observed usage window,
and reset time. Ask for explicit user consent before taking either action.

After an affirmative response, run `rebinder continuity accept` and report its
next-step message exactly. After a negative response, run `rebinder continuity
decline`. An ambiguous response is neither acceptance nor rejection; ask again.

Never invoke `rebinder transfer` directly from inside Claude Code. Rebinder
finishes an accepted handoff after the current Claude process exits, or through
the explicit `rebinder continuity resume` fallback printed by the CLI.

If Claude Code reports a provider rate-limit failure, do not claim that the
failed model turn can authorize a transfer. Rebinder records that supported
`StopFailure` signal separately. The user exits Claude Code and answers the
local `rebinder claude` rescue prompt, or runs `rebinder continuity rescue`.
