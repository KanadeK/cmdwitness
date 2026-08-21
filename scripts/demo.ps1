$ErrorActionPreference = "Stop"

cargo build --locked --bin cmdwitness --examples
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$cmdwitness = Join-Path "target/debug" "cmdwitness.exe"
$baseline = Join-Path "target/debug/examples" "baseline_cli.exe"
$candidate = Join-Path "target/debug/examples" "candidate_cli.exe"
$report = "target/demo-report.md"

& $cmdwitness compare --spec examples/scenarios.json --baseline $baseline --candidate $candidate --format markdown --output $report
$demoExit = $LASTEXITCODE
if ($demoExit -ne 1) { throw "expected compatibility break exit 1, got $demoExit" }

Get-Content -LiteralPath $report -Raw
