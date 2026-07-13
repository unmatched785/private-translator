$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $env:CARGO_HOME) {
    $env:CARGO_HOME = Join-Path $root ".cache\cargo"
}

Push-Location $root
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed with exit code $LASTEXITCODE" }
    cargo test --locked --all-targets
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed with exit code $LASTEXITCODE" }
    cargo clippy --locked --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed with exit code $LASTEXITCODE" }
    node --check .\web\app.js
    if ($LASTEXITCODE -ne 0) { throw "web/app.js syntax check failed with exit code $LASTEXITCODE" }
    node --check .\web\i18n.js
    if ($LASTEXITCODE -ne 0) { throw "web/i18n.js syntax check failed with exit code $LASTEXITCODE" }
    node --check .\scripts\mock-server.mjs
    if ($LASTEXITCODE -ne 0) { throw "scripts/mock-server.mjs syntax check failed with exit code $LASTEXITCODE" }
    & .\scripts\test-release-tag-available.ps1
    & .\scripts\test-package-policy.ps1
}
finally {
    Pop-Location
}
