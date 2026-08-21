# ADR-0001: Small Rust binary and JSON scenario schema

## Status

Accepted

## Date

2026-08-12

## Context

CmdWitness executes user-selected local programs and must ship as a small,
auditable, cross-platform binary. The scenario format needs arrays and nested
objects, strict boundary validation, deterministic serialization, and a
version field.

## Decision

Use Rust 2024 with `serde`/`serde_json` for the public JSON boundary and
`wait-timeout` for cross-platform process deadlines. Reject unknown schema
fields and validate scenario identifiers, fixture paths, and numeric limits
once at the CLI boundary. Internal typed data is trusted after validation.

## Alternatives considered

- A project-owned JSON parser: rejected as unnecessary security-critical code.
- YAML: nicer for hand authoring, but adds another parser and has a broader,
  more surprising data model than this v1 contract needs.
- TOML: the array-of-tables representation is less direct for scenario data.

## Consequences

- Release binaries remain self-contained; source builds fetch three small,
  established crates and their transitive build dependencies.
- Project code stays focused on validation and compatibility semantics rather
  than reimplementing JSON or portable timeout behavior.
- YAML may be added later only through an explicit dependency review and an
  additive import path; JSON v1 remains the stable interchange contract.
