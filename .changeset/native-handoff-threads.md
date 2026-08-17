---
"rebinder": patch
---

Create context-safe handoffs with Codex's native thread APIs instead of passing
derived files to the external-session importer. Persist retry-safe bindings,
reuse the native thread when the source changes, and surface completed handoff
threads in session listings.
