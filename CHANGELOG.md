# Changelog

All notable changes to Rebinder are documented in this file.

Releases use the calendar identity `0.YYYYMMDD.REVISION`.

## [Unreleased]

## [0.20260817.2] - 2026-08-17

### Changed

- Point exact-version installation examples at a published, checksum-verified release.

## [0.20260817.1] - 2026-08-17

### Changed

- Keep release source validation deterministic when CI-generated dependency files are present, while auditing that they never enter the Rust crate.

## [0.20260817.0] - 2026-08-17

### Added

- Establish the Rust CLI, canonical session package schemas, integrity and
  structural validation, human and JSON inspection, provider command
  passthrough, and the cross-harness transfer command contract.
- Add the Calendar Versioning release ledger, native distribution foundation,
  and professional MVP documentation structure.

[Unreleased]: https://github.com/bahadirarda/rebinder/compare/v0.20260817.2...HEAD
[0.20260817.2]: https://github.com/bahadirarda/rebinder/compare/v0.20260817.1...v0.20260817.2
[0.20260817.1]: https://github.com/bahadirarda/rebinder/compare/v0.20260817.0...v0.20260817.1
[0.20260817.0]: https://github.com/bahadirarda/rebinder/releases/tag/v0.20260817.0
