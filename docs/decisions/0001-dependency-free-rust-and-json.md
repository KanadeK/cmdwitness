# ADR-0001: Dependency-free Rust binary and JSON scenario schema

## Status

Accepted

## Date

2026-08-12

## Context

CmdWitness executes user-selected local programs and must ship as a small,
auditable, cross-platform binary. The scenario format needs arrays and nested
objects, strict validation, deterministic serialization, and a version field.

## Decision

Use Rust 2024 with no third-party crates. Implement a bounded JSON reader and
writer that supports the subset required by a versioned public schema while
correctly handling standard JSON strings, numbers, arrays, objects, booleans,
and null. Reject duplicate object keys, excessive nesting, oversized input,
unknown fields where ambiguity would be unsafe, and non-finite numbers.

## Alternatives considered

- `serde` + `serde_json`: excellent ergonomics but adds a dependency graph and
  network/bootstrap surface to a small security-sensitive CLI.
- YAML: nicer for hand authoring, but a standards-correct parser is too complex
  to own and common YAML libraries add a larger dependency surface.
- TOML: dependency-free parsing still requires substantial custom code and the
  array-of-tables representation is less direct for scenario fixtures.

## Consequences

- Release builds require only the Rust toolchain and are easy to audit.
- The parser is project-owned security-critical code and therefore receives
  explicit depth, size, Unicode, number, duplicate-key, and malformed-input
  tests.
- YAML may be added later only through an explicit dependency review and an
  additive import path; JSON v1 remains the stable interchange contract.

