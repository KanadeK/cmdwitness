# Scenario reference (schema version 1)

CmdWitness accepts one JSON object. Unknown fields fail fast. Run
`cmdwitness schema` for the exact machine-readable schema.

## Top level

| Field | Required | Meaning |
| --- | --- | --- |
| `schemaVersion` | yes | Must be `1`. |
| `scenarios` | yes | One to 128 scenario objects, run in declaration order. |
| `normalizers` | no | Up to 32 ordered normalizers applied to every scenario. |

## Scenario

| Field | Default | Meaning |
| --- | --- | --- |
| `id` | required | Unique `[A-Za-z0-9._-]` identifier, at most 80 bytes. |
| `args` | `[]` | Tokens appended after the target's repeated `--baseline-arg` or `--candidate-arg` values. |
| `stdin` | closed | UTF-8 text written to stdin. Without this field, stdin is closed so prompts cannot inherit the terminal. |
| `env` | `{}` | Explicit string environment additions. Secret values are not printed by CmdWitness. |
| `fixtures` | `[]` | Text files written into both isolated workspaces before execution. |
| `observe` | `exitCode`, `stdout`, `stderr` | Unique observation names. |
| `allow` | `[]` | Exact finding kinds, `category.*` wildcards, or complete finding IDs. |
| `normalizers` | `[]` | Up to 32 ordered scenario-local normalizers, after top-level normalizers. |

Fixture paths use `/`, are relative to the synthetic workspace, and cannot
contain empty, `.`, `..`, backslash, drive-prefix, NUL, or control components.
Fixtures total at most 8 MiB per scenario.

## Observations

- `exitCode`: compares numeric exit status (or missing status after a signal).
- `stdout`, `stderr`: compares normalized UTF-8 exactly.
- `jsonStdout`: parses normalized stdout and recursively compares JSON. Invalid
  requested JSON makes compatibility unknown (exit `2`).
- `help`: extracts conventional flags and commands from normalized stdout plus
  stderr.
- `files`: compares regular file paths and bytes after the command. Symlinks
  and special files are rejected rather than followed.

## Normalizers

Normalizers are opt-in and applied in declaration order. CmdWitness always
replaces its own synthetic workspace root with `<WORKDIR>` because the two
isolated paths are implementation noise.

```json
[
  { "kind": "ansi" },
  { "kind": "lineEndings" },
  { "kind": "slashes" },
  { "kind": "literal", "name": "version", "from": "v1.2.3", "to": "<VER>" }
]
```

- `ansi`: removes conventional CSI/OSC terminal escape sequences.
- `lineEndings`: converts CRLF and CR to LF.
- `slashes`: converts `\` to `/`.
- `literal`: exact, non-regex replacement. `name` appears in report evidence;
  `to` cannot be longer in UTF-8 bytes than `from`, preventing normalization
  from expanding already bounded command output.

Avoid broad literal normalizers: hiding real values can hide real breaks.

## Finding kinds

```text
exitCode.changed
stdout.changed
stderr.changed
json.added
json.removed
json.typeChanged
json.valueChanged
help.commandAdded
help.commandRemoved
help.flagAdded
help.flagRemoved
files.added
files.removed
files.changed
```

Finding IDs are deterministic: `<scenario>:<kind>:<path>`. An exact ID is the
narrowest possible allowance.
