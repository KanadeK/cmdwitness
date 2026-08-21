# CmdWitness

[![CI](https://github.com/KanadeK/cmdwitness/actions/workflows/ci.yml/badge.svg)](https://github.com/KanadeK/cmdwitness/actions/workflows/ci.yml)
[![MIT license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Your CLI passed its tests. Did it still break somebody's script?**

CmdWitness runs the same declared scenarios against two versions of any local
command and reports observable compatibility drift: exit codes, stdout,
stderr, JSON shape and types, help flags/commands, and file side effects.

It is a real black-box comparison, not a UI, a generated checklist, or a
single-version snapshot library.

```text
INCOMPATIBLE
3 breaking scenarios, 0 additive, 0 allowed, 1 compatible

[BREAKING] machine-json
- BREAKING json.typeChanged $.count: JSON value type changed ("number" -> "string")
- BREAKING json.removed $.legacy: JSON value was removed (removed "true")
- ALLOWED json.added $.metadata: JSON value was added
```

## Why CmdWitness exists

CLI behavior is an API even when it was never written down. Automation can
depend on an exit code, a JSON field type, which stream receives a warning, or
the exact file a command creates. Real upgrades have changed all of these.

CmdWitness makes that contract executable:

```text
scenario JSON
    -> isolated baseline workspace -> baseline observation
    -> isolated candidate workspace -> candidate observation
    -> declared normalization -> semantic diff -> text / JSON / Markdown / SARIF
```

See the dated [research landscape](research/landscape.md) for demand evidence,
closest projects, search limits, and rejected ideas. Stars cannot be promised;
the project targets a searched capability gap with a concrete user problem.

## 60-second demo

Requirements: Rust 1.85+ and PowerShell 5+ or a POSIX shell.

```powershell
git clone https://github.com/KanadeK/cmdwitness.git
cd cmdwitness
powershell -NoProfile -File scripts/demo.ps1
```

```sh
git clone https://github.com/KanadeK/cmdwitness.git
cd cmdwitness
./scripts/demo.sh
```

The demo compiles two fixture CLIs, compares four real scenarios, writes
`target/demo-report.md`, verifies the expected breaking exit code (`1`), and
prints the report. Its inputs are in [examples/scenarios.json](examples/scenarios.json).

## Install

Download the archive for your platform from
[GitHub Releases](https://github.com/KanadeK/cmdwitness/releases), or build from
source:

```sh
cargo install --locked --path .
cmdwitness version
```

The release binary is self-contained. Building from source uses the three
declared Rust dependencies pinned in `Cargo.lock`.

## Use

Compare two binaries:

```sh
cmdwitness compare \
  --spec cmdwitness.json \
  --baseline ./bin/tool-v1 \
  --candidate ./bin/tool-v2 \
  --format markdown \
  --output compatibility.md
```

Compare two scripts through the same interpreter. Fixed target arguments are
placed before each scenario's `args`:

```sh
cmdwitness compare \
  --spec cmdwitness.json \
  --baseline python --baseline-arg old/cli.py \
  --candidate python --candidate-arg new/cli.py \
  --format sarif --output cmdwitness.sarif
```

Full CLI help:

```sh
cmdwitness help
cmdwitness schema
```

### Scenario file

```json
{
  "schemaVersion": 1,
  "normalizers": [
    { "kind": "ansi" },
    { "kind": "lineEndings" }
  ],
  "scenarios": [
    {
      "id": "machine-output",
      "args": ["inspect", "--json"],
      "env": { "LANG": "C" },
      "fixtures": [
        { "path": "input/data.txt", "content": "alpha\n" }
      ],
      "observe": ["exitCode", "jsonStdout", "files"],
      "allow": ["json.added"]
    }
  ]
}
```

The authoritative schema is
[schema/cmdwitness-v1.schema.json](schema/cmdwitness-v1.schema.json). The
[scenario reference](docs/scenario-reference.md) documents every field,
normalizer, finding kind, and allowance rule.

## What it compares

| Observation | Breaking evidence | Additive evidence |
| --- | --- | --- |
| `exitCode` | Any changed exit code | — |
| `stdout`, `stderr` | Exact normalized text changed | — |
| `jsonStdout` | Key/index removed, type changed, scalar changed | Key/index added |
| `help` | Flag or command removed | Flag or command added |
| `files` | File removed or content changed | File added |

Additions do not fail the default gate. A scenario is breaking when it has any
unallowed breaking finding. Expected drift remains in the report with severity
`allowed`:

```json
"allow": ["json.added", "files.*", "contract:stdout.changed:$stdout"]
```

Allowances match an exact finding kind, a category wildcard ending in `.*`, or
a complete finding ID. They never delete evidence.

## Reports and CI exits

`--format` accepts `text`, `json`, `markdown`, and `sarif`. Every renderer uses
the same typed report and finding IDs.

| Exit | Meaning |
| ---: | --- |
| `0` | Comparison completed with no unallowed breaking differences. |
| `1` | Comparison completed and found breaking differences. |
| `2` | Input, launch, timeout, output, JSON, workspace, or report failure; compatibility is unknown. |

Execution errors are never relabeled as compatible.

## Safety boundary

CmdWitness treats scenario files and command output as untrusted data:

- programs and argument arrays are passed directly to the OS; no shell string
  is constructed;
- each side/scenario gets a separate temporary directory containing only the
  declared fixtures;
- environment inheritance is reduced to launch-critical variables plus the
  scenario's explicit `env`;
- every command has a deadline and stdout/stderr/file limits;
- fixture paths cannot be absolute or traverse upward;
- observed symlinks and special files make the comparison unknown.

**A temporary directory is not an OS sandbox.** CmdWitness intentionally runs
the local executable you name. Use a container or virtual machine for untrusted
programs. CmdWitness does not download versions, access the network itself, or
upload reports.

## Acceptance commands

From a clean checkout:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

Then run the platform demo and consolidated release gate:

```powershell
powershell -NoProfile -File scripts/demo.ps1
powershell -NoProfile -File scripts/release_check.ps1
```

```sh
./scripts/demo.sh
./scripts/release_check.sh
```

## When something fails

| Symptom | Cause to check | Repair |
| --- | --- | --- |
| Exit `1` | A real unallowed compatibility break | Read finding kind/path/evidence; restore behavior or add a narrow reviewed `allow` entry. |
| Exit `2` with `invalidJson` | A scenario requested `jsonStdout`, but one side emitted non-JSON | Make that mode produce JSON or observe `stdout` instead. Do not treat it as compatible. |
| `launch` | Program/path or fixed prefix arguments are wrong | Run each target directly, then pass the executable via `--baseline`/`--candidate` and prefix tokens via repeated `--*-arg`. |
| `timeout` | The command is interactive, hung, or legitimately slow | Make it non-interactive; otherwise raise `--timeout-ms` within the documented bound. |
| `outputLimit` / `fileLimit` | The scenario is too broad or the command is runaway | Narrow the scenario/fixtures first; only then raise `--max-output-bytes`. |
| `link.exe not found` on Windows | Rust MSVC is installed without Visual C++ Build Tools | Install the Visual Studio C++ Build Tools, or use an installed GNU toolchain: `cargo +stable-x86_64-pc-windows-gnu test --all-targets --locked`. |
| Demo expected exit `1` but got `0` | Fixture behavior or comparison semantics regressed | Run the report in `json` format and inspect `summary`/`findings`; do not weaken the demo assertion. |
| CI passes locally but fails on one OS | Path, line-ending, or executable-bit assumptions | Reproduce on that OS; use declared `lineEndings`/`slashes` normalizers only for intentional noise. |

More detail is in [CONTRIBUTING.md](CONTRIBUTING.md) and the
[release runbook](docs/release.md).

## Deliberate limits

- No PTY/TUI recording or interactive prompt automation.
- No automatic package download, installation, or registry resolution.
- No HTTP, OpenAPI, MCP, ABI, or language-specific API comparison.
- Help parsing extracts conventional `Commands:` sections and flag tokens; it
  does not guess arbitrary prose.
- Text observations must be UTF-8. Invalid text output returns unknown.

These limits keep CmdWitness focused on the smallest useful cross-version CLI
contract workflow.

## License

[MIT](LICENSE) © 2026 KanadeK
