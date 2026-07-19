$ErrorActionPreference = "Stop"

function Invoke-Git {
    param(
        [string]$Repo,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$GitArgs
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git -C $Repo @GitArgs
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    if ($exitCode -ne 0) {
        throw "git -C $Repo $($GitArgs -join ' ') failed with exit code $exitCode"
    }
}

function Git-Output {
    param(
        [string]$Repo,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$GitArgs
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & git -C $Repo @GitArgs
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    if ($exitCode -ne 0) {
        throw "git -C $Repo $($GitArgs -join ' ') failed with exit code $exitCode"
    }
    return $output
}

function Set-GitIdentity {
    param([string]$Repo)

    Invoke-Git $Repo config user.name "rsshogi export check"
    Invoke-Git $Repo config user.email "rsshogi-export-check@example.com"
}

function Write-TestFile {
    param(
        [string]$Path,
        [string]$Content
    )

    $dir = Split-Path -Parent $Path
    if ($dir) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    Set-Content -LiteralPath $Path -Value $Content -Encoding UTF8
}

function Assert-NotTracked {
    param(
        [string]$Repo,
        [string]$Pathspec
    )

    $tracked = Git-Output $Repo ls-files -- $Pathspec
    if ($tracked) {
        throw "dev-only path is tracked after export: $Pathspec"
    }
}

$rootDir = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "git rev-parse failed"
}

$tmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("rsshogi-public-export-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmpRoot | Out-Null

try {
    $developGit = Join-Path $tmpRoot "develop.git"
    $publicGit = Join-Path $tmpRoot "public.git"
    $source = Join-Path $tmpRoot "source"
    $publicWork = Join-Path $tmpRoot "public-work"
    $exportWork = Join-Path $tmpRoot "export-work"

    & git init -q --bare --initial-branch=main $developGit
    if ($LASTEXITCODE -ne 0) { throw "git init develop.git failed" }
    & git init -q --bare --initial-branch=main $publicGit
    if ($LASTEXITCODE -ne 0) { throw "git init public.git failed" }

    & git init -q -b main $source
    if ($LASTEXITCODE -ne 0) { throw "git init source failed" }
    Set-GitIdentity $source

    Write-TestFile (Join-Path $source "README.md") "public readme"
    Write-TestFile (Join-Path $source "crates/rsshogi/Cargo.toml") "[package]"
    Write-TestFile (Join-Path $source "AGENTS.md") "private agent instructions"
    Write-TestFile (Join-Path $source "CLAUDE.md") "private claude instructions"
    Write-TestFile (Join-Path $source "GEMINI.md") "private gemini instructions"
    Write-TestFile (Join-Path $source ".agents/private.md") "private agents metadata"
    Write-TestFile (Join-Path $source ".cursor/rules.md") "private cursor rules"
    Write-TestFile (Join-Path $source ".codex/private.md") "private codex metadata"
    Write-TestFile (Join-Path $source ".claude/settings.json") "{}"
    Write-TestFile (Join-Path $source "agent-docs/tasks/private.md") "private task"
    Write-TestFile (Join-Path $source "release-notes/v0.9.0.txt") "previous project release"
    Write-TestFile (Join-Path $source ".sandbox/private.txt") "private sandbox"
    Write-TestFile (Join-Path $source ".serena/private.txt") "private serena"

    Invoke-Git $source add -A
    Invoke-Git $source commit -q -m "source snapshot"
    Invoke-Git $source remote add develop $developGit
    Invoke-Git $source push -q develop main

    & git init -q -b main $publicWork
    if ($LASTEXITCODE -ne 0) { throw "git init public-work failed" }
    Set-GitIdentity $publicWork
    Write-TestFile (Join-Path $publicWork "README.md") "old public readme"
    Write-TestFile (Join-Path $publicWork "obsolete.txt") "removed upstream"
    Write-TestFile (Join-Path $publicWork "AGENTS.md") "must be removed from public base"
    Invoke-Git $publicWork add -A
    Invoke-Git $publicWork commit -q -m "old public snapshot"
    Invoke-Git $publicWork remote add public $publicGit
    Invoke-Git $publicWork push -q public main

    & git clone -q -b main $publicGit $exportWork
    if ($LASTEXITCODE -ne 0) { throw "git clone export-work failed" }
    Set-GitIdentity $exportWork
    Invoke-Git $exportWork remote rename origin public
    Invoke-Git $exportWork remote add develop $developGit

    New-Item -ItemType Directory -Force -Path (Join-Path $exportWork "scripts") | Out-Null
    Copy-Item -LiteralPath (Join-Path $rootDir "scripts/export_public_snapshot.ps1") -Destination (Join-Path $exportWork "scripts/export_public_snapshot.ps1")
    Invoke-Git $exportWork add scripts/export_public_snapshot.ps1
    Invoke-Git $exportWork commit -q -m "add export script for check"

    Push-Location $exportWork
    try {
        $previousErrorActionPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $powerShellExe = (Get-Process -Id $PID).Path
            $powerShellArgs = @("-NoProfile")
            if ([System.IO.Path]::GetFileName($powerShellExe) -ieq "powershell.exe") {
                $powerShellArgs += @("-ExecutionPolicy", "Bypass")
            }
            $powerShellArgs += @("-File", "scripts/export_public_snapshot.ps1", "-DryRun", "v0.0.0", "develop/main", "public/main")
            & $powerShellExe @powerShellArgs *> $null
            $exitCode = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previousErrorActionPreference
        }
        if ($exitCode -ne 0) {
            throw "export_public_snapshot.ps1 dry-run failed"
        }
    } finally {
        Pop-Location
    }

    Assert-NotTracked $exportWork "AGENTS.md"
    Assert-NotTracked $exportWork "CLAUDE.md"
    Assert-NotTracked $exportWork "GEMINI.md"
    Assert-NotTracked $exportWork ".agents"
    Assert-NotTracked $exportWork ".cursor"
    Assert-NotTracked $exportWork ".codex"
    Assert-NotTracked $exportWork ".claude"
    Assert-NotTracked $exportWork "agent-docs"
    Assert-NotTracked $exportWork "release-notes"
    Assert-NotTracked $exportWork ".sandbox"
    Assert-NotTracked $exportWork ".serena"

    if (Git-Output $exportWork ls-files -- obsolete.txt) {
        throw "deleted upstream path is still tracked after export: obsolete.txt"
    }

    if (-not (Git-Output $exportWork ls-files -- README.md)) {
        throw "public file was not exported: README.md"
    }

    Write-Host "public export check OK"
} finally {
    Remove-Item -LiteralPath $tmpRoot -Recurse -Force -ErrorAction SilentlyContinue
}
