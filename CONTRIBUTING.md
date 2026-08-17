# Contributing to Rebinder

## Development setup

Install Rust `1.92.0` and Bun `1.3.14`, then run:

```bash
bun install --frozen-lockfile
bun run check
```

## Change workflow

1. Create a focused branch from `main`.
2. Add tests for behavior changes.
3. Update the canonical documentation when a contract changes.
4. Add a Changeset for every user-visible CLI, format, adapter, installer, or
   compatibility change with `bun run changeset`.
5. Run `bun run check` before opening a pull request.

Pull request titles follow Conventional Commits, for example
`feat(adapter): discover Codex sessions`. Keep commits reviewable and do not mix
unrelated refactors with behavior changes.

## Compatibility and safety

- Structural package validation and target compatibility are separate layers.
- Unsupported provider state must be reported rather than silently discarded.
- New filesystem operations require confinement and symlink tests.
- New redaction behavior requires positive and negative fixtures.
- Schema-breaking changes require a new schema version and migration notes;
  product CalVer changes alone do not version the interchange format.

## Release policy

Contributors add Changesets but do not edit release numbers directly. The
automated version pull request calculates `0.YYYYMMDD.REVISION`. Maintainers
create annotated tags only after that pull request and every required check pass.
