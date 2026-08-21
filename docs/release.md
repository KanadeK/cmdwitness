# Release and recovery runbook

## Pre-release

1. Update `CHANGELOG.md` and confirm `Cargo.toml` has the intended version.
2. Start from a clean `main` checkout.
3. Run `scripts/release_check.ps1` on Windows or
   `scripts/release_check.sh` on Unix.
4. Confirm the demo exits through its expected breaking path and the package
   smoke test runs `version` and `schema` from the extracted archive.
5. Verify author/committer identity and that commit messages contain no
   `Co-authored-by` trailer.

## Publish

```sh
git tag -a v0.1.0 -m "Release 0.1.0"
git push origin main
git push origin v0.1.0
```

The tag workflow tests and packages native Windows, Linux, and macOS binaries,
creates one `SHA256SUMS.txt`, and publishes the GitHub Release.

## Verify

- CI and Security audit are green for the exact tagged commit.
- The public tag resolves to the intended commit.
- Release archives and `SHA256SUMS.txt` are present.
- Download one archive for the current platform, verify its checksum once,
  extract it, and run `cmdwitness version` plus the included scenario schema.
- GitHub contributors list only the intended author for this release.

## Recovery

- **Tag workflow fails before Release creation:** fix the cause on `main`, bump
  to the next patch version, and create a new tag. Do not move a public tag.
- **A published archive is wrong:** mark the release as affected, fix forward,
  and publish a patch release. Do not silently replace immutable evidence.
- **CLI regression is discovered:** add a scenario that reproduces it, restore
  the contract or document the intentional breaking change, then publish the
  correct semantic version.
- **A secret appears in history or artifacts:** revoke it first, remove the
  affected public artifact, follow GitHub's sensitive-data removal guidance,
  and disclose impact. Deleting one line is not remediation.
