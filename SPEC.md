# Spec: CmdWitness 0.1.1

## Objective

CmdWitness helps CLI maintainers and automation owners answer one question:
**will replacing this command with that version break observable behavior?**

The user supplies a baseline command, a candidate command, and a versioned JSON
scenario file. CmdWitness executes both commands in equivalent isolated
workspaces, normalizes declared volatile text, compares results, classifies
changes, and emits evidence in terminal, JSON, Markdown, or SARIF form.

## Tech stack

- Rust 2024 edition, MSRV 1.85.
- `serde`/`serde_json` for the versioned JSON boundary and `wait-timeout` for
  portable process timeouts. The shipped binary has no runtime dependencies.
- GitHub Actions builds and tests on Linux, macOS, and Windows.

## Commands

```text
Build:          cargo build --locked
Format check:   cargo fmt --all -- --check
Lint:           cargo clippy --all-targets --locked -- -D warnings
Unit/integration: cargo test --all-targets --locked
Demo (Windows): powershell -NoProfile -File scripts/demo.ps1
Demo (Unix):    sh scripts/demo.sh
Package:        powershell -NoProfile -File scripts/package.ps1 -Version 0.1.1
Release gate:   powershell -NoProfile -File scripts/release_check.ps1
```

## Public CLI contract

```text
cmdwitness compare --spec <file> --baseline <program> --candidate <program>
                   [--format text|json|markdown|sarif] [--output <file>]
                   [--timeout-ms <100..300000>] [--max-output-bytes <1024..16777216>]
cmdwitness schema
cmdwitness version
cmdwitness help
```

Exit codes:

- `0`: comparison completed with no breaking or unknown differences.
- `1`: comparison completed and found at least one breaking difference.
- `2`: input, execution, or report-generation error; compatibility is unknown.

## Scenario schema

The top level is an object with `schemaVersion: 1`, `scenarios`, and optional
global `normalizers`. Each scenario has:

- `id`: stable identifier.
- `args`: argument array appended to both programs.
- `stdin`: optional text.
- `env`: optional explicit environment additions; the process otherwise gets a
  small safe inherited environment required to launch programs.
- `fixtures`: optional files copied into each isolated workspace. Paths must be
  relative, may not traverse upward, and may not be absolute.
- `observe`: any of `exitCode`, `stdout`, `stderr`, `jsonStdout`, `help`,
  `files`; defaults to `exitCode`, `stdout`, and `stderr`.
- `allow`: optional list of expected difference selectors.
- `normalizers`: ordered replacements with literal or built-in modes.

Example style:

```json
{
  "schemaVersion": 1,
  "normalizers": [{"kind": "ansi"}],
  "scenarios": [{
    "id": "machine-output",
    "args": ["inspect", "--json"],
    "observe": ["exitCode", "jsonStdout", "files"],
    "allow": ["json.added"]
  }]
}
```

## Classification rules

- `breaking`: exit success becomes failure; a flag/command disappears; JSON
  keys disappear or change type; observed output changes without an allowance;
  an existing file disappears or changes type/content.
- `additive`: help adds a flag/command; JSON adds a key; a new side-effect file
  appears. Additive changes do not fail the default gate.
- `allowed`: a matching explicit allowance converts a known difference to
  allowed while retaining evidence.
- `unknown`: either command cannot start, times out, exceeds output limits,
  produces invalid JSON when JSON was requested, or violates isolation limits.
  Unknown makes the tool exit `2`; it is never reported as compatible.

## Project structure

```text
src/                 CLI, scenario model, runner, diff, reports
tests/               black-box integration and failure-path tests
examples/            deterministic fixture CLIs and runnable scenario data
research/            dated landscape and differentiation evidence
docs/decisions/      architecture and security decisions
tasks/               implementation plan and completion checklist
scripts/             packaging, checksums, secret scan, release gate
.github/workflows/   cross-platform CI and tag release automation
```

## Testing strategy

- Unit tests: scenario deserialization/validation, normalization,
  classification, help parsing, file inventory, and report escaping.
- Integration tests: real fixture processes for exit-code drift, stdout/stderr,
  JSON type/removal/addition, help flag changes, file changes, allowances,
  timeouts, output caps, traversal rejection, and exit semantics.
- Package smoke: build an archive, verify SHA-256, extract into a clean temp
  directory, then run `version` and `schema` from the packaged binary.
- Coverage proxy: every public comparison dimension and every error exit has a
  named test. Rust's standard toolchain has no built-in stable coverage gate;
  the release checklist records the behavioral matrix instead of claiming a
  percentage without instrumentation.

## Boundaries

Always:

- execute programs without a shell;
- use separate temporary directories and cap time/output/file traversal;
- validate all paths and schema fields before running a command;
- preserve concise evidence for every classified difference;
- keep deterministic reports except for an explicit tool version field.

Ask first:

- adding a third-party dependency;
- changing the versioned scenario schema;
- executing a command with network privileges or outside the isolated copy.

Never:

- download or install the compared programs;
- interpolate scenario strings into a shell command;
- inherit secrets by default or print environment values;
- label execution errors as compatible;
- modify the user's fixture source files.

## Success criteria

- A checked-in demo identifies at least four real breaking dimensions: removed
  flag/help item, exit-code drift, JSON type/removal, and file-content drift.
- JSON, Markdown, SARIF, and human text reports describe the same findings.
- A clean clone can build/test with only Rust 1.85+ and package release assets.
- CI is green on Windows, Linux, and macOS.
- Release contains platform archives plus `SHA256SUMS.txt`; downloaded hashes
  match the local manifest.
- README includes exact acceptance commands and a symptom-to-repair runbook.
- Public author, commit, contributor, tag, release, and asset checks pass.

## Open questions

None for 0.1.1. PTY/TUI capture, automatic package installation, and networked
commands are explicitly deferred beyond the current security boundary.
