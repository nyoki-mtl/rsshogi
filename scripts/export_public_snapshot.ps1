param(
    [Parameter(Position = 0, Mandatory = $true)]
    [string]$Version,

    [Parameter(Position = 1)]
    [string]$SourceRef = "develop/main",

    [Parameter(Position = 2)]
    [string]$PublicBase = "public/main",

    [switch]$DryRun,

    [string]$NotesFile = ""
)

$ErrorActionPreference = "Stop"

function Invoke-Git {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$GitArgs
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git @GitArgs
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    if ($exitCode -ne 0) {
        throw "git $($GitArgs -join ' ') failed with exit code $exitCode"
    }
}

function Test-GitSuccess {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$GitArgs
    )

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & git @GitArgs *> $null
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    return $exitCode -eq 0
}

function Require-CleanTree {
    if (-not (Test-GitSuccess diff --quiet --ignore-submodules --)) {
        throw "working tree has unstaged changes"
    }
    if (-not (Test-GitSuccess diff --cached --quiet --ignore-submodules --)) {
        throw "index has staged changes"
    }

    $untracked = & git ls-files --others --exclude-standard
    if ($LASTEXITCODE -ne 0) {
        throw "git ls-files failed"
    }
    if ($untracked) {
        throw "untracked files exist"
    }
}

function Build-CommitMessage {
    param(
        [string]$ReleaseVersion,
        [string]$ReleaseNotesFile
    )

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("Release $ReleaseVersion")
    if ($ReleaseNotesFile) {
        $lines.Add("")
        foreach ($line in Get-Content -LiteralPath $ReleaseNotesFile) {
            $lines.Add($line)
        }
    }
    return ($lines -join "`n") + "`n"
}

function Assert-NoDevOnlyPaths {
    $found = $false
    $paths = & git ls-files
    if ($LASTEXITCODE -ne 0) {
        throw "git ls-files failed"
    }

    foreach ($path in $paths) {
        if (
            $path -eq "AGENTS.md" -or
            $path -eq "CLAUDE.md" -or
            $path -eq "GEMINI.md" -or
            $path -like ".agents/*" -or
            $path -like ".cursor/*" -or
            $path -like ".codex/*" -or
            $path -like ".claude/*" -or
            $path -like ".vscode/*" -or
            $path -like "agent-docs/*" -or
            $path -like "release-notes/*" -or
            $path -like ".sandbox/*" -or
            $path -like ".serena/*"
        ) {
            Write-Error "dev-only path would be exported: $path"
            $found = $true
        }
    }

    if ($found) {
        throw "dev-only paths found in public snapshot"
    }
}

function Remove-PathIfExists {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

if ($NotesFile -and -not (Test-Path -LiteralPath $NotesFile)) {
    throw "notes file not found: $NotesFile"
}

$exportBranch = "export-public"
$publicGitAuthorName = if ($env:PUBLIC_GIT_AUTHOR_NAME) { $env:PUBLIC_GIT_AUTHOR_NAME } else { "nyoki-mtl" }
$publicGitAuthorEmail = if ($env:PUBLIC_GIT_AUTHOR_EMAIL) { $env:PUBLIC_GIT_AUTHOR_EMAIL } else { "charmer.popopo@gmail.com" }

$excludePaths = @(
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".agents/**",
    ".cursor/**",
    ".codex/**",
    ".claude/**",
    ".vscode/**",
    "agent-docs/**",
    "release-notes/**",
    ".sandbox/**",
    ".serena/**"
)

# REMOVE_PATHS should only list paths that are committed in SourceRef. Untracked
# local-only dirs are excluded from checkout but are not removed from disk.
$removePaths = @(
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".agents",
    ".cursor",
    ".codex",
    ".claude",
    ".vscode",
    "agent-docs",
    "release-notes",
    ".sandbox",
    ".serena"
)

Require-CleanTree

Write-Host "[1/5] Fetch remotes"
Invoke-Git fetch develop
Invoke-Git fetch public

if (-not (Test-GitSuccess rev-parse --verify --quiet $SourceRef)) {
    throw "source ref not found: $SourceRef"
}

$hasPublicBase = Test-GitSuccess rev-parse --verify --quiet $PublicBase

Write-Host "[2/5] Prepare export branch"
if ($hasPublicBase) {
    Write-Host "  base: $PublicBase"
    Invoke-Git checkout -B $exportBranch $PublicBase
} else {
    Write-Host "  base: (none, recreating orphan branch)"

    if (Test-GitSuccess rev-parse --verify --quiet "refs/heads/$exportBranch") {
        $currentBranch = & git branch --show-current
        if ($currentBranch -eq $exportBranch) {
            Invoke-Git checkout --detach HEAD
        }
        # Quote -D so PowerShell does not bind it as the common -Debug parameter.
        Invoke-Git branch "-D" $exportBranch
    }

    Invoke-Git checkout --orphan $exportBranch
    & git rm -r -f --ignore-unmatch . *> $null
}

Write-Host "[3/5] Overlay $SourceRef with excluded dev-only paths"
$checkoutArgs = @($SourceRef, "--", ".")
foreach ($path in $excludePaths) {
    $checkoutArgs += ":(exclude)$path"
}
Invoke-Git checkout @checkoutArgs

Write-Host "[4/5] Remove excluded paths from public snapshot"
& git rm -r --ignore-unmatch -- @removePaths *> $null
foreach ($path in $removePaths) {
    Remove-PathIfExists $path
}

Write-Host "[5/5] Reflect deletions from $SourceRef"
if ($hasPublicBase) {
    $deletedPaths = & git diff --name-only --diff-filter=D "$PublicBase..$SourceRef"
    if ($LASTEXITCODE -ne 0) {
        throw "git diff failed"
    }
    foreach ($path in $deletedPaths) {
        if ($path) {
            & git rm --ignore-unmatch -- $path *> $null
        }
    }
}

Invoke-Git add -A
Assert-NoDevOnlyPaths

if (Test-GitSuccess diff --cached --quiet) {
    Write-Host "No changes to export."
    exit 0
}

if ($DryRun) {
    Write-Host "Dry run OK. Snapshot is staged and no commit was created."
    Invoke-Git diff --cached --stat
    exit 0
}

$commitMessageFile = [System.IO.Path]::GetTempFileName()
try {
    $message = Build-CommitMessage $Version $NotesFile
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($commitMessageFile, $message, $utf8NoBom)

    $env:GIT_AUTHOR_NAME = $publicGitAuthorName
    $env:GIT_AUTHOR_EMAIL = $publicGitAuthorEmail
    $env:GIT_COMMITTER_NAME = $publicGitAuthorName
    $env:GIT_COMMITTER_EMAIL = $publicGitAuthorEmail
    Invoke-Git commit -F $commitMessageFile
} finally {
    Remove-Item -LiteralPath $commitMessageFile -Force -ErrorAction SilentlyContinue
}

Write-Host "Snapshot commit created on branch '$exportBranch'."
Write-Host "Next:"
Write-Host "  git push public HEAD:main"
