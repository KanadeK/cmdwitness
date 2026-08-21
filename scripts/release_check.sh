#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

if [ -n "$(git status --porcelain)" ]; then
  echo "release gate requires a clean worktree" >&2
  exit 2
fi

cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo audit
sh scripts/demo.sh >/dev/null
cargo package --locked

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
if [ -z "$version" ]; then
  echo "could not read package version" >&2
  exit 2
fi

case $(uname -s) in
  Linux) target_name="linux-$(uname -m)" ;;
  Darwin) target_name="macos-$(uname -m)" ;;
  *) echo "unsupported packaging platform" >&2; exit 2 ;;
esac
sh scripts/package.sh "$version" "$target_name" >/dev/null
archive="dist/cmdwitness-v${version}-${target_name}.tar.gz"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$archive" | sed 's#  .*/#  #' > dist/SHA256SUMS.txt
else
  shasum -a 256 "$archive" | sed 's#  .*/#  #' > dist/SHA256SUMS.txt
fi

smoke=$(mktemp -d "$repo_root/target/cmdwitness-smoke.XXXXXX")
trap 'rm -rf -- "$smoke"' EXIT HUP INT TERM
tar -C "$smoke" -xzf "$archive"
binary="$smoke/cmdwitness-v${version}-${target_name}/cmdwitness"
test "$("$binary" version)" = "cmdwitness $version"
"$binary" schema >/dev/null

if git grep -n -I -E 'gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}' -- . ':!Cargo.lock'; then
  echo "high-signal secret pattern found" >&2
  exit 2
fi

echo "Release gate passed for cmdwitness v$version"
