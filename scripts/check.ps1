$ErrorActionPreference = "Stop"
$env:CARGO_HOME = Join-Path (Get-Location) ".cache\cargo"
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
node --check .\web\app.js

