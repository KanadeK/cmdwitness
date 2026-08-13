# Research landscape: CLI behavior compatibility

Research date: 2026-08-12

## Decision

Build **CmdWitness**, a black-box compatibility checker that runs the same
declared scenarios against two local CLI executables and explains observable
behavior drift.

This is a searched capability gap, not a claim that no implementation can
possibly exist. GitHub stars cannot be guaranteed. The project is justified by
real breakage, adjacent tools with demonstrated interest, and a distinct
workflow not found in the searches below.

## Demand signals

| Signal | What it establishes |
| --- | --- |
| [AWS CLI v2 migration changes](https://docs.aws.amazon.com/cli/latest/userguide/cliv2-migration-changes.html) | A major CLI upgrade changed return codes and pagination behavior, requiring scripts to be reviewed. |
| [Terraform v1.x compatibility promises](https://developer.hashicorp.com/terraform/language/v1-compatibility-promises) | JSON output and exit codes can be automation contracts while human-readable output may change. |
| [GitHub CLI issue #5721](https://github.com/cli/cli/issues/5721) | TTY detection and ANSI/prompt output can break non-interactive consumers. |
| [Salesforce CLI output and scripting](https://developer.salesforce.com/blogs/2020/02/using-salesforce-cli-output-and-scripting) | Human-readable output changes break scripts; machine-readable output needs deliberate stability. |
| [npm CLI changelog](https://github.com/npm/cli/blob/latest/CHANGELOG.md) | Real releases change unknown-flag handling, JSON shapes, and other externally observable behavior. |
| [CLI Spec](https://clispec.dev/) | Flags, piped output, prompts, and exit codes are all parts of a CLI's user-facing contract. |

## Closest tools found

Searches used GitHub repository search, GitHub's repository API, and Exa web
search with combinations of: `CLI snapshot`, `CLI testing`, `command line
testing`, `compare two CLI versions behavior`, `black box CLI compatibility
checker`, `stdout stderr exit code diff`, and `behavioral compatibility CLI`.

| Project | Observed stars | Why it is adjacent, not the same product |
| --- | ---: | --- |
| [Verify](https://github.com/VerifyTests/Verify) | 3,463 | General approval/snapshot testing, not an old-vs-new executable compatibility audit. |
| [TDDA](https://github.com/tdda/tdda) | 310 | Records characterization tests including command output, but does not provide a two-version semantic compatibility report. |
| [snapbox](https://github.com/assert-rs/snapbox) | 182 | Rust CLI snapshot assertions for a test suite, not a standalone cross-version comparator. |
| [cli-testing-library](https://github.com/crutchcorn/cli-testing-library) | 136 | Utilities for testing one CLI implementation. |
| [backspin](https://github.com/rsanheim/backspin) | 7 | Ruby stdout/stderr snapshot testing for one CLI. |
| [parrot](https://github.com/CharlyCst/parrot) | 4 | Single-version snapshot testing. |

Stars are a time-specific observation from 2026-08-12 and may change.

## Alternatives rejected

- **Test-order failure minimizer:** overlaps the existing local FlakeHarbor
  project and mature tools such as pytest-randomly, detect-test-pollution, and
  iDFlakies.
- **Reproducible-artifact root-cause finder:** useful, but adjacent local tools
  already cover archive metadata, release receipts, export verification, and
  package change tracing. It would also compete with diffoscope and reprotest.
- **Environment-variable drift:** multiple recent repositories already offer
  mature code/Docker/CI drift checks.
- **Redacted repro/support bundles:** several complete projects already package
  commands, logs, files, hashes, and redaction evidence.

## Product boundary

CmdWitness compares two user-provided local commands. It does not install
versions, download packages, emulate a terminal, test HTTP APIs or MCP, or
replace language-specific snapshot libraries. It executes argument arrays
directly without a shell and writes only to isolated temporary workspaces plus
explicit report paths.

