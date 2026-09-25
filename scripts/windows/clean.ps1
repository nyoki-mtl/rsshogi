$ErrorActionPreference = "Stop"

cargo clean

$directories = @(
    ".pytest_cache",
    ".ruff_cache"
)

foreach ($directory in $directories) {
    if (Test-Path -LiteralPath $directory) {
        Remove-Item -LiteralPath $directory -Recurse -Force
    }
}

Get-ChildItem -Path . -Recurse -Directory -Force -Filter "__pycache__" |
    Remove-Item -Recurse -Force

Get-ChildItem -Path . -Recurse -File -Force -Filter "*.pyc" |
    Remove-Item -Force
