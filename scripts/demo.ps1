$ErrorActionPreference = "Stop"

cargo build --locked --bin cmdwitness --examples
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$suffix = if ($env:OS -eq "Windows_NT") { ".exe" } else { "" }
$cmdwitness = Join-Path "target/debug" ("cmdwitness" + $suffix)
$baseline = Join-Path "target/debug/examples" ("baseline_cli" + $suffix)
$candidate = Join-Path "target/debug/examples" ("candidate_cli" + $suffix)
$report = "target/demo-report.md"

& $cmdwitness compare --spec examples/scenarios.json --baseline $baseline --candidate $candidate --format markdown --output $report
$demoExit = $LASTEXITCODE
if ($demoExit -ne 1) { throw "expected compatibility break exit 1, got $demoExit" }

Get-Content -LiteralPath $report -Raw
