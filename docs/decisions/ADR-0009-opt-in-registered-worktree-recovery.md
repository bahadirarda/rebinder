---
type: adr
title: Opt-in Registered Worktree Recovery
status: accepted
version: 0.1.0
---

# ADR-0009: Opt-in Registered Worktree Recovery

## Context

Agent sessions frequently record a Git worktree as their working directory.
The directory may later be removed while the owning repository still retains
its worktree administrative record, committed HEAD, and branch association.
Opening a transferred session in a nonexistent directory fails, but silently
cloning, guessing a branch, or recreating any missing path would be an unsafe
filesystem mutation and could misrepresent lost state.

Git's own worktree registry is the narrowest authoritative local recovery
surface. It can prove that a repository owns the exact path and identify the
committed checkout, but it cannot restore uncommitted or ignored files that
were present in the deleted directory.

## Decision

Keep missing workspaces fail-closed by default. Add `--recover-worktree` to the
cross-harness `transfer` command for both directions. Recovery proceeds only
when all of these conditions hold:

1. The recorded target is absolute, does not exist, and has an existing
   non-symlink immediate parent.
2. An explicitly supplied repository, an existing ancestor repository, or one
   repository from a bounded direct-sibling scan reports the exact target in
   `git worktree list --porcelain`.
3. Exactly one repository matches, the entry is unlocked, and its recorded
   commit exists locally.
4. An attached branch still resolves to the registry HEAD at creation time.
5. `git worktree add --force` succeeds, after which Rebinder verifies HEAD,
   attached branch state, and the common Git directory.

`--worktree-repository PATH` is available as an explicit owner hint and
requires `--recover-worktree`. Rebinder never invokes a shell for Git
arguments, fetches or clones, creates a missing parent, unlocks a registration,
overwrites an existing path, or claims to restore uncommitted state.

For Codex-to-Claude, the canonical export is refreshed after successful
recovery so its repository snapshot and semantic revision describe the
recreated checkout before Claude receives the artifact.

## Consequences

Users can continue a session whose registered committed checkout was removed
without manually reconstructing a worktree, while ordinary transfer behavior
remains non-mutating until the explicit flag is present. Sibling layouts that
cannot be discovered safely remain usable with the repository hint.

Locked, pruned, unregistered, remote-only, ambiguous, or path-conflicting
worktrees remain errors. Changes that existed only in the deleted working tree
are unrecoverable and must come from another backup or source-control object.

## References

- [Git worktree documentation](https://git-scm.com/docs/git-worktree)
