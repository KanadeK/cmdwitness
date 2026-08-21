$ErrorActionPreference = "Stop"
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Push-Location $repoRoot
try {
    $dirty = git status --porcelain
    if ($dirty) { throw "release gate requires a clean worktree" }

    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "format check failed" }
    cargo clippy --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "Clippy failed" }
    cargo test --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }

    cargo audit --version | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "cargo-audit is required: cargo install cargo-audit --locked" }
    cargo audit
    if ($LASTEXITCODE -ne 0) { throw "dependency audit failed" }

    & (Join-Path $PSScriptRoot "demo.ps1") | Out-Null

    cargo package --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo package failed" }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object { $_.name -eq "cmdwitness" }).version
    & (Join-Path $PSScriptRoot "package.ps1") -Version $version -TargetName "windows-x86_64" | Out-Null

    $archive = Join-Path $repoRoot "dist/cmdwitness-v$version-windows-x86_64.zip"
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([System.IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath (Join-Path $repoRoot "dist/SHA256SUMS.txt") -Encoding ascii

    $smoke = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target/package-smoke"))
    if (-not $smoke.StartsWith($repoRoot + [System.IO.Path]::DirectorySeparatorChar)) {
        throw "package smoke path escaped the repository"
    }
    if (Test-Path -LiteralPath $smoke) { Remove-Item -LiteralPath $smoke -Recurse -Force }
    Expand-Archive -LiteralPath $archive -DestinationPath $smoke
    $packagedBinary = Join-Path $smoke "cmdwitness-v$version-windows-x86_64/cmdwitness.exe"
    $reportedVersion = & $packagedBinary version
    if ($reportedVersion -ne "cmdwitness $version") { throw "packaged version mismatch" }
    $schema = & $packagedBinary schema | ConvertFrom-Json
    if ($schema.properties.schemaVersion.'const' -ne 1) { throw "packaged schema smoke failed" }

    $secretMatches = git grep -n -I -E 'gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}' -- . ':!Cargo.lock'
    if ($LASTEXITCODE -eq 0) { throw "high-signal secret pattern found:`n$secretMatches" }
    if ($LASTEXITCODE -ne 1) { throw "secret scan failed to run" }

    "Release gate passed for cmdwitness v$version"
}
finally {
    Pop-Location
}
