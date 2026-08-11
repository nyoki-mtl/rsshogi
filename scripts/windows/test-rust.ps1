$ErrorActionPreference = "Stop"

cargo test --doc -p rsshogi --all-features

if (Get-Command cargo-nextest -ErrorAction SilentlyContinue) {
    Write-Host "[make] cargo-nextest detected: running nextest"
    cargo nextest run -p rsshogi --tests --all-features
} else {
    Write-Host "[make] cargo-nextest not found: falling back to cargo test"
    cargo test -p rsshogi --tests --all-features
}
