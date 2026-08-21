# ADR-0002: Local command execution boundary

## Status

Accepted

## Date

2026-08-12

## Context

The core feature must execute arbitrary local CLI programs. Scenario files and
program output are untrusted. A temporary directory is useful isolation but is
not an operating-system sandbox.

## Decision

- Accept program paths separately from argument arrays; never parse or invoke a
  shell command string.
- Resolve relative program paths against the invocation directory before any
  scenario runs; delegate bare command names to the operating system's `PATH`.
- Create a different temporary workspace for every program/scenario pair and
  copy only declared fixture files.
- Inherit only the minimum launch-critical environment plus explicit scenario
  additions; never report environment values.
- Bound runtime, combined output bytes, scenario count, fixture bytes, JSON
  size/depth, and observed file count/bytes.
- On timeout, output overflow, invalid requested JSON, or launch failure, stop
  comparison and return unknown/exit 2.
- Never claim that this protects the host from a malicious executable. Users
  must use an OS sandbox or container for untrusted programs.

## Alternatives considered

- Shell command templates: rejected because quoting is platform-dependent and
  untrusted arguments create command-injection risk.
- Containers: rejected as a mandatory dependency; optional adapter support may
  be designed later.
- In-place execution: rejected because side effects would collide and could
  modify user fixtures.

## Consequences

- Common deterministic CLIs work without Docker or elevated privileges.
- Interactive TUI/PTY behavior and network isolation are out of scope for v1.
- Tests must exercise real processes, timeouts, output caps, path traversal,
  relative program paths, and fixture immutability on Windows and Unix.
