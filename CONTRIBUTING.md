# Contributing

Thank you for helping CmdWitness keep CLI upgrades honest.

## Set up

Install Rust 1.85 or newer, clone the repository, then run:

```sh
cargo test --all-targets --locked
```

No service, account, database, or API key is required.

## Change workflow

1. Describe the observable behavior and add a failing test.
2. Implement the smallest change that makes it pass.
3. Run formatting, Clippy, all tests, and the real demo.
4. Update the schema/reference when the public scenario contract changes.
5. Keep refactors separate from behavior changes.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
./scripts/demo.sh
```

On Windows, use `powershell -NoProfile -File scripts/demo.ps1`.

## Pull requests

- Explain the user-visible reason for the change.
- Include focused tests for success and failure paths.
- Do not add a dependency when the existing stack or standard library solves
  the current requirement clearly.
- Never weaken a failing gate, change exit semantics silently, or hide a
  difference with an automatic fuzzy normalizer.
- Treat compared program output as untrusted data.

Architecture decisions live in [docs/decisions](docs/decisions).
