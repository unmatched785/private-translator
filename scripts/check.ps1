$ErrorActionPreference = "Stop"
if (-not $env:CARGO_HOME) {
    $env:CARGO_HOME = Join-Path (Get-Location) ".cache\cargo"
}
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
node --check .\web\app.js
node --check .\web\i18n.js
node --check .\scripts\mock-server.mjs
