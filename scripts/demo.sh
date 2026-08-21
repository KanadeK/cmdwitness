#!/usr/bin/env sh
set -eu

cargo build --locked --bin cmdwitness --examples

set +e
target/debug/cmdwitness compare \
  --spec examples/scenarios.json \
  --baseline target/debug/examples/baseline_cli \
  --candidate target/debug/examples/candidate_cli \
  --format markdown \
  --output target/demo-report.md
demo_exit=$?
set -e

if [ "$demo_exit" -ne 1 ]; then
  echo "expected compatibility break exit 1, got $demo_exit" >&2
  exit 2
fi

cat target/demo-report.md
