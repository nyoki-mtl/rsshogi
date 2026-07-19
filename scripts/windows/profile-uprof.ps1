param(
    [string]$UProfPath = "C:\Program Files\AMD\AMDuProf\bin\AMDuProfCLI.exe",
    [string]$Config = "hotspots",
    [int]$Depth = 4,
    [string]$Positions = "data\bench\diverse_positions.csv",
    [int]$Duration = 10,
    [string]$OutputDir = "target\uprof",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $UProfPath)) {
    throw "AMDuProfCLI.exe not found: $UProfPath"
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Push-Location $repoRoot
try {
    New-Item -ItemType Directory -Force $OutputDir | Out-Null

    if (-not $SkipBuild) {
        $oldRustflags = $env:RUSTFLAGS
        $env:RUSTFLAGS = "-C target-cpu=native -C target-feature=+avx2,+bmi1,+bmi2 -C debuginfo=2 -C force-frame-pointers=yes"
        try {
            cargo build --release -p rsshogi --bin perft_bench
        } finally {
            $env:RUSTFLAGS = $oldRustflags
        }
    }

    $before = Get-ChildItem $OutputDir -Directory -Filter "AMDuProf-perft_bench-*" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    & $UProfPath profile `
        --config $Config `
        --duration $Duration `
        --terminate `
        --output-dir $OutputDir `
        --working-dir $repoRoot `
        target\release\perft_bench.exe `
        --depth $Depth `
        --positions $Positions

    $session = Get-ChildItem $OutputDir -Directory -Filter "AMDuProf-perft_bench-*" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $session -or ($null -ne $before -and $session.FullName -eq $before.FullName)) {
        throw "Could not locate the new AMD uProf session directory."
    }

    $reportPath = Join-Path $session.FullName "report-detail.csv"
    & $UProfPath report `
        -i $session.FullName `
        --detail `
        --group-by module `
        --bin-path target\release `
        --symbol-path target\release `
        --src-path crates\rsshogi\src `
        --report-output $reportPath

    Write-Host "AMD uProf session: $($session.FullName)"
    Write-Host "Detailed report: $reportPath"
} finally {
    Pop-Location
}
