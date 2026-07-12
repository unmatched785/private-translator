$ErrorActionPreference = "Stop"
$env:CARGO_HOME = Join-Path (Get-Location) ".cache\cargo"
cargo run --release -- --profile lite --open

