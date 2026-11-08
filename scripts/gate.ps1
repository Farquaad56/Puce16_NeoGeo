$ErrorActionPreference = "Stop"
function Step($cmd) { Invoke-Expression $cmd; if ($LASTEXITCODE -ne 0) { Write-Host "GATE FAIL : $cmd"; exit 1 } }
Step "cargo fmt --all --check"
Step "cargo clippy --workspace --all-targets -- -D warnings"
Step "cargo test --workspace"
if (Test-Path tools/check_metrics.ps1) { Step "powershell -NoProfile -File tools/check_metrics.ps1" }
Write-Host "GATE OK"
