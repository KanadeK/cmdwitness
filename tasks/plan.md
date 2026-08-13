# Implementation plan: CmdWitness

## Architecture decisions

- Use a dependency-free Rust binary for portable release assets and a small
  supply-chain surface.
- Make the scenario schema JSON v1; YAML is not accepted in 0.1.0 because a
  dependency-free, standards-correct YAML parser would add disproportionate
  complexity.
- Execute only argv arrays via `std::process::Command`; no shell parsing.
- Copy declared fixture files into separate temporary workspaces; inventory
  only those workspaces after execution.
- Treat inability to compare as `unknown`/exit 2, never a clean result.

## Dependency graph

```text
JSON + validated schema
  -> normalizers + observation model
  -> bounded isolated runner + file inventory
  -> semantic diff/classification + allowances
  -> text/JSON/Markdown/SARIF reporters
  -> CLI exit semantics
  -> examples, packages, CI, release gate
```

## Phases

1. Contract and parser foundation.
2. Safe execution and observation capture.
3. Semantic comparison and reporters.
4. End-to-end demo and failure-path hardening.
5. Documentation, packaging, CI, review, and release.

## Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Compared command is malicious | High | Document that temp workspaces are not OS sandboxes; no shell, bounded runtime/output, minimal inherited env, explicit local programs only. |
| Cross-platform process termination differs | High | Dedicated Windows/Unix implementations behind one runner contract and CI on all three OS families. |
| Normalization hides a breaking change | High | Opt-in normalizers, evidence retains raw hashes and applied normalizer names, no broad fuzzy matching. |
| JSON parser divergence | Medium | RFC-style parser tests for Unicode escapes, numbers, invalid input, depth and size limits. |
| Reports disagree | Medium | Generate every format from one typed report and assert shared finding IDs/counts. |

