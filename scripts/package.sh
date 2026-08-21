#!/usr/bin/env sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: scripts/package.sh VERSION TARGET_NAME" >&2
  exit 2
fi

version=$1
target_name=$2
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
package_name="cmdwitness-v${version}-${target_name}"
dist="$repo_root/dist"
mkdir -p "$repo_root/target" "$dist"
staging=$(mktemp -d "$repo_root/target/cmdwitness-package.XXXXXX")
trap 'rm -rf -- "$staging"' EXIT HUP INT TERM

cargo build --release --locked
mkdir -p "$staging/$package_name/examples" "$staging/$package_name/schema"
cp "$repo_root/target/release/cmdwitness" "$staging/$package_name/"
cp "$repo_root/README.md" "$repo_root/LICENSE" "$staging/$package_name/"
cp "$repo_root/examples/scenarios.json" "$staging/$package_name/examples/"
cp "$repo_root/schema/cmdwitness-v1.schema.json" "$staging/$package_name/schema/"

archive="$dist/$package_name.tar.gz"
rm -f -- "$archive"
tar -C "$staging" -czf "$archive" "$package_name"
printf '%s\n' "$archive"
