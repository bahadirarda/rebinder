# Changesets

Every user-visible CLI, format, adapter, installer, or compatibility change
requires a Changeset.

```bash
bun run changeset
bun run changeset:status
```

Select `rebinder`, record the compatibility impact, and write a concise
user-facing summary. The impact remains review metadata; the published number
is calculated independently as `0.YYYYMMDD.REVISION`.

Changesets do not publish anything. The automated version pull request consumes
them, synchronizes Cargo and Bun metadata, and writes the dated changelog entry.
An annotated tag and the protected release workflow are separate steps.
