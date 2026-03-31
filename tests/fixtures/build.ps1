# PowerShell build script to compile all test fixture contracts to WASM and refresh manifest.json.

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ContractsDir = Join-Path $ScriptDir "contracts"
$WasmDir = Join-Path $ScriptDir "wasm"
$ManifestPath = Join-Path $ScriptDir "manifest.json"
$ReleaseTargetDir = Join-Path $ContractsDir "target\wasm32-unknown-unknown\release"
$DebugTargetDir = Join-Path $ContractsDir "target\wasm32-unknown-unknown\release-debug"

function Get-FixtureExports {
    param([string]$Name)

    switch ($Name) {
        "always_panic" { return @("panic") }
        "budget_heavy" { return @("heavy") }
        "counter" { return @("get", "increment") }
        "cross_contract" { return @("call") }
        "echo" { return @("echo") }
        "same_return" { return @("same") }
        default { throw "Unknown fixture export set for '$Name'" }
    }
}

function Get-Sha256 {
    param([string]$Path)
    (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

$InstalledTargets = rustup target list --installed
if ($InstalledTargets -notcontains "wasm32-unknown-unknown") {
    Write-Host "Error: wasm32-unknown-unknown target not installed." -ForegroundColor Red
    Write-Host "Install it with: rustup target add wasm32-unknown-unknown" -ForegroundColor Yellow
    exit 1
}

New-Item -ItemType Directory -Force -Path $WasmDir | Out-Null

Write-Host "Building test fixture contracts..." -ForegroundColor Cyan

$ManifestFixtures = @()

Get-ChildItem -Path $ContractsDir -Directory | Sort-Object Name | ForEach-Object {
    $ContractDir = $_.FullName
    $ContractName = $_.Name
    $CargoToml = Join-Path $ContractDir "Cargo.toml"

    if (-not (Test-Path $CargoToml)) {
        return
    }

    Write-Host "  Building $ContractName..." -ForegroundColor Yellow

    Push-Location $ContractDir
    try {
        cargo build --release --target wasm32-unknown-unknown
        cargo build --profile release-debug --target wasm32-unknown-unknown
    } finally {
        Pop-Location
    }

    $PackageName = Select-String -Path $CargoToml -Pattern '^name = "(.*)"$' |
        ForEach-Object { $_.Matches[0].Groups[1].Value } |
        Select-Object -First 1

    if (-not $PackageName) {
        throw "Failed to determine package name for $ContractName"
    }

    $WasmFileName = $PackageName -replace "-", "_"
    $ReleaseSource = Join-Path $ReleaseTargetDir "${WasmFileName}.wasm"
    $DebugSource = Join-Path $DebugTargetDir "${WasmFileName}.wasm"
    $ReleaseDest = Join-Path $WasmDir "${ContractName}.wasm"
    $DebugDest = Join-Path $WasmDir "${ContractName}_debug.wasm"

    if (-not (Test-Path $ReleaseSource)) {
        throw "Failed to find release WASM output for $ContractName"
    }
    if (-not (Test-Path $DebugSource)) {
        throw "Failed to find debug WASM output for $ContractName"
    }

    Copy-Item $ReleaseSource $ReleaseDest -Force
    Copy-Item $DebugSource $DebugDest -Force

    $ManifestFixtures += [ordered]@{
        name = $ContractName
        exports = (Get-FixtureExports -Name $ContractName)
        source = [ordered]@{
            contract_dir = "tests/fixtures/contracts/$ContractName"
            lib_rs = "tests/fixtures/contracts/$ContractName/src/lib.rs"
        }
        artifacts = [ordered]@{
            release = [ordered]@{
                path = "tests/fixtures/wasm/$ContractName.wasm"
                sha256 = Get-Sha256 -Path $ReleaseDest
            }
            debug = [ordered]@{
                path = "tests/fixtures/wasm/${ContractName}_debug.wasm"
                sha256 = Get-Sha256 -Path $DebugDest
            }
        }
    }
}

$Manifest = [ordered]@{
    version = 1
    fixtures = $ManifestFixtures
}

$Manifest | ConvertTo-Json -Depth 6 | Set-Content -Path $ManifestPath

Write-Host ""
Write-Host "All contracts built successfully." -ForegroundColor Green
Write-Host "WASM files are in: $WasmDir" -ForegroundColor Cyan
Write-Host "Manifest refreshed at: $ManifestPath" -ForegroundColor Cyan
