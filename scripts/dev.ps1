$ErrorActionPreference = "Stop"
Write-Host "PonyClean dev mode" -ForegroundColor Cyan
cargo run
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
