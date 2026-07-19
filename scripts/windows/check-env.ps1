$ErrorActionPreference = "Stop"

function Test-Command($Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        [pscustomobject]@{ Name = $Name; Status = "missing"; Path = "" }
    } else {
        [pscustomobject]@{ Name = $Name; Status = "ok"; Path = $command.Source }
    }
}

$tools = @(
    "git",
    "gh",
    "rustup",
    "rustc",
    "cargo",
    "uv",
    "python",
    "make",
    "just",
    "rg",
    "cl",
    "link",
    "node",
    "npm"
)

$tools | ForEach-Object { Test-Command $_ } | Format-Table -AutoSize

Write-Host ""
Write-Host "Rust toolchain:"
rustup show active-toolchain

Write-Host ""
Write-Host "Versions:"
rustc --version
cargo --version
uv --version
python --version
git --version

Write-Host ""
Write-Host "Optional tools:"
foreach ($tool in @("cargo-nextest", "mdbook")) {
    $command = Get-Command $tool -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        Write-Host "  ${tool}: missing"
    } else {
        Write-Host "  ${tool}: $($command.Source)"
    }
}
