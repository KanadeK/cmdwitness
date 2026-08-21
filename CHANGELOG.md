# Changelog

All notable user-facing changes are documented here. This project follows
[Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-08-21

### Fixed

- Release publication now checks out the tagged repository before verifying
  the tag and creating the GitHub Release.

## [0.1.0] - 2026-08-21

### Added

- Black-box baseline/candidate execution in separate fixture workspaces.
- Exit-code, stdout, stderr, structural JSON, help surface, and file side-effect
  comparison.
- Explicit ANSI, line-ending, slash, and literal normalization with visible
  evidence.
- Narrow expected-difference allowances that retain findings.
- Text, JSON, Markdown, and SARIF reports with stable CI exit codes.
- Versioned JSON Schema, deterministic example CLIs, cross-platform CI,
  release archives, and a consolidated release gate.
