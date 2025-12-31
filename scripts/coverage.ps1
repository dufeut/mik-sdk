# Coverage script for mik-sdk
# Requires: cargo install cargo-llvm-cov

param(
    [switch]$Html,
    [switch]$Report,
    [switch]$All
)

$ErrorActionPreference = "Stop"

Write-Host "=== mik-sdk Coverage ===" -ForegroundColor Cyan

# Check if cargo-llvm-cov is installed
if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
    Write-Host "Error: cargo-llvm-cov is not installed." -ForegroundColor Red
    Write-Host "Install with: cargo install cargo-llvm-cov" -ForegroundColor Yellow
    exit 1
}

# Run tests with coverage and generate LCOV
Write-Host "`nGenerating coverage data..." -ForegroundColor Green
cargo llvm-cov --all-features --workspace --lcov --output-path target/lcov.info
if ($LASTEXITCODE -ne 0) {
    Write-Host "Coverage run failed!" -ForegroundColor Red
    exit 1
}

# Generate HTML report
if ($Html -or $All) {
    Write-Host "`nGenerating HTML report..." -ForegroundColor Green
    cargo llvm-cov --all-features --workspace --html --output-dir target/coverage
    Write-Host "HTML report: target/coverage/index.html" -ForegroundColor Cyan
}

# Show summary report
if ($Report -or $All) {
    Write-Host "`n=== Coverage Summary ===" -ForegroundColor Cyan
    cargo llvm-cov report --all-features --workspace
}

Write-Host "`n=== Coverage Complete ===" -ForegroundColor Green
Write-Host "LCOV file: target/lcov.info" -ForegroundColor Cyan

if ($Html -or $All) {
    Write-Host "`nOpening HTML report..." -ForegroundColor Green
    Start-Process "target/coverage/index.html"
}
