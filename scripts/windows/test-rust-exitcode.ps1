$ErrorActionPreference = "Stop"

$testScript = Join-Path $PSScriptRoot "test-rust.ps1"
$powershell = (Get-Command powershell.exe -ErrorAction Stop).Source
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("rsshogi-test-rust-" + [guid]::NewGuid())
$originalPath = $env:PATH
$fakeCargo = @'
@echo off
>> "%RSSHOGI_TEST_RUST_LOG%" echo %*
if "%1"=="check" goto check
if "%1"=="test" goto test
if "%1"=="nextest" goto nextest
exit /b 0

:check
if "%RSSHOGI_TEST_RUST_FAIL_PHASE%"=="check" exit /b 17
exit /b 0

:test
if "%2"=="--doc" goto doctest
if "%RSSHOGI_TEST_RUST_FAIL_PHASE%"=="test" exit /b 19
exit /b 0

:doctest
if "%RSSHOGI_TEST_RUST_FAIL_PHASE%"=="doctest" exit /b 18
exit /b 0

:nextest
if "%RSSHOGI_TEST_RUST_FAIL_PHASE%"=="nextest" exit /b 20
exit /b 0
'@
$expectedCommands = @{
    check = @("check -p rsshogi --all-targets")
    doctest = @("check -p rsshogi --all-targets", "test --doc -p rsshogi --all-features")
    nextest = @("check -p rsshogi --all-targets", "test --doc -p rsshogi --all-features", "nextest run -p rsshogi --tests --all-features")
    test = @("check -p rsshogi --all-targets", "test --doc -p rsshogi --all-features", "test -p rsshogi --tests --all-features")
}
$expectedExitCodes = @{ check = 17; doctest = 18; nextest = 20; test = 19 }

New-Item -ItemType Directory -Path $tempRoot | Out-Null
try {
    foreach ($phase in @("check", "doctest", "nextest", "test")) {
        $caseRoot = Join-Path $tempRoot $phase
        New-Item -ItemType Directory -Path $caseRoot | Out-Null
        $logPath = Join-Path $caseRoot "cargo.log"
        [System.IO.File]::WriteAllText((Join-Path $caseRoot "cargo.cmd"), $fakeCargo, [System.Text.Encoding]::ASCII)
        if ($phase -ne "test") {
            [System.IO.File]::WriteAllText((Join-Path $caseRoot "cargo-nextest.cmd"), "@exit /b 0`r`n", [System.Text.Encoding]::ASCII)
        }

        if ($phase -eq "test") {
            $env:PATH = "$caseRoot;$env:SystemRoot\System32"
        } else {
            $env:PATH = "$caseRoot;$originalPath"
        }
        $env:RSSHOGI_TEST_RUST_FAIL_PHASE = $phase
        $env:RSSHOGI_TEST_RUST_LOG = $logPath
        & $powershell -NoProfile -File $testScript
        $exitCode = $LASTEXITCODE
        $env:PATH = $originalPath
        Remove-Item Env:RSSHOGI_TEST_RUST_FAIL_PHASE -ErrorAction SilentlyContinue
        Remove-Item Env:RSSHOGI_TEST_RUST_LOG -ErrorAction SilentlyContinue

        if ($exitCode -ne $expectedExitCodes[$phase]) {
            throw "${phase}: expected exit code $($expectedExitCodes[$phase]), got $exitCode"
        }
        $actualCommands = @(Get-Content -LiteralPath $logPath)
        $expected = $expectedCommands[$phase]
        if ($actualCommands.Count -ne $expected.Count) {
            throw "${phase}: expected $($expected.Count) cargo commands, got $($actualCommands.Count)"
        }
        for ($index = 0; $index -lt $expected.Count; $index++) {
            if ($actualCommands[$index] -cne $expected[$index]) {
                throw "${phase}: expected '$($expected[$index])', got '$($actualCommands[$index])'"
            }
        }
        Write-Host "[test] ${phase}: propagated exit code $exitCode before later phases"
    }
} finally {
    $env:PATH = $originalPath
    Remove-Item Env:RSSHOGI_TEST_RUST_FAIL_PHASE -ErrorAction SilentlyContinue
    Remove-Item Env:RSSHOGI_TEST_RUST_LOG -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $tempRoot) {
        $resolvedTempRoot = (Resolve-Path -LiteralPath $tempRoot).Path
        $expectedTempRoot = [System.IO.Path]::GetFullPath($tempRoot)
        if ($resolvedTempRoot -ne $expectedTempRoot -or
            [System.IO.Path]::GetDirectoryName($resolvedTempRoot) -ne [System.IO.Path]::GetTempPath().TrimEnd('\')) {
            throw "refusing to remove unexpected test directory: $resolvedTempRoot"
        }
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
