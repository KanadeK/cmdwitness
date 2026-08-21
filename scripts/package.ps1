param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    [string]$TargetName = "windows-x86_64"
)

$ErrorActionPreference = "Stop"
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$dist = Join-Path $repoRoot "dist"
$packageName = "cmdwitness-v$Version-$TargetName"
$staging = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target/package-staging/$packageName"))
if (-not $staging.StartsWith($repoRoot + [System.IO.Path]::DirectorySeparatorChar)) {
    throw "package staging path escaped the repository"
}

Push-Location $repoRoot
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    if (Test-Path -LiteralPath $staging) {
        Remove-Item -LiteralPath $staging -Recurse -Force
    }
    New-Item -ItemType Directory -Path (Join-Path $staging "examples") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $staging "schema") -Force | Out-Null
    New-Item -ItemType Directory -Path $dist -Force | Out-Null

    Copy-Item -LiteralPath (Join-Path $repoRoot "target/release/cmdwitness.exe") -Destination $staging
    Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination $staging
    Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE") -Destination $staging
    Copy-Item -LiteralPath (Join-Path $repoRoot "examples/scenarios.json") -Destination (Join-Path $staging "examples")
    Copy-Item -LiteralPath (Join-Path $repoRoot "schema/cmdwitness-v1.schema.json") -Destination (Join-Path $staging "schema")

    $archive = Join-Path $dist "$packageName.zip"
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
    Compress-Archive -LiteralPath $staging -DestinationPath $archive -CompressionLevel Optimal
    $archive
}
finally {
    if (Test-Path -LiteralPath $staging) {
        Remove-Item -LiteralPath $staging -Recurse -Force
    }
    Pop-Location
}
