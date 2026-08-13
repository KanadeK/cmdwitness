# CmdWitness 0.1.0 tasks

- [ ] Task 1: Implement bounded JSON parser and validated v1 scenario model.
  - Acceptance: valid examples parse; invalid types, traversal, duplicate IDs,
    excessive depth/size, and unknown observation modes fail before execution.
  - Verify: `cargo test json scenario`.
- [ ] Task 2: Implement isolated fixture workspaces and bounded process runner.
  - Acceptance: argv is shell-free; stdin/env/cwd work; timeout/output caps and
    launch errors become typed unknown results; file inventory is deterministic.
  - Verify: `cargo test runner workspace`.
- [ ] Task 3: Implement observation normalization and semantic comparison.
  - Acceptance: exit/output/JSON/help/files classify breaking/additive/allowed;
    raw evidence hashes and normalizer names remain visible.
  - Verify: `cargo test compare normalize`.
- [ ] Task 4: Implement CLI and four reporters.
  - Acceptance: text/JSON/Markdown/SARIF share finding IDs/counts; exits are
    exactly 0/1/2 per spec; explicit output paths are atomic.
  - Verify: `cargo test cli report`.
- [ ] Task 5: Ship deterministic fixtures and a real end-to-end demo.
  - Acceptance: demo proves removed help flag, exit drift, JSON type/removal,
    additive key, output normalization, and file-content drift.
  - Verify: run the documented demo and inspect generated report.
- [ ] Task 6: Complete docs, ADRs, repair guide, CI, and packaging.
  - Acceptance: clean-clone commands, troubleshooting, threat model, platform
    archives, checksums, and package smoke are executable and documented.
  - Verify: `scripts/release_check.ps1` plus CI matrix.
- [ ] Task 7: Independent five-axis review and public release closure.
  - Acceptance: no critical/required review findings; clean Git history; public
    CI/tag/Release/assets/contributors verified; completion email sent.
  - Verify: remote API evidence and downloaded asset hash comparison.

