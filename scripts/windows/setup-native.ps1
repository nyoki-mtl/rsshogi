$ErrorActionPreference = "Stop"

Write-Host "[setup] Ensuring Rust toolchain from rust-toolchain.toml"
rustup show active-toolchain

function Repair-RustComponent($Component, $Binary) {
    rustup component add $Component
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $resolved = & rustup which $Binary 2>$null
    $whichExitCode = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorActionPreference
    if ($whichExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($resolved) -or -not (Test-Path -LiteralPath $resolved)) {
        Write-Host "[setup] Repairing missing $Binary from $Component component"
        rustup component remove $Component
        rustup component add $Component
    }
}

Write-Host "[setup] Ensuring Rust components"
Repair-RustComponent "cargo" "cargo"
Repair-RustComponent "rustfmt" "rustfmt"
Repair-RustComponent "clippy" "cargo-clippy"

Write-Host "[setup] Installing optional Cargo tools"
if (-not (Get-Command cargo-nextest -ErrorAction SilentlyContinue)) {
    cargo install cargo-nextest --locked
}
if (-not (Get-Command just -ErrorAction SilentlyContinue)) {
    cargo install just --locked
}
if (-not (Get-Command mdbook -ErrorAction SilentlyContinue)) {
    cargo install mdbook --locked
}

Write-Host "[setup] Syncing Python dev dependencies"
uv sync --dev

Write-Host "[setup] Reducing Windows file mode noise in this repository"
git config core.filemode false

Write-Host "[setup] Done"
