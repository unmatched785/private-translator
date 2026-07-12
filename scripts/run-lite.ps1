$ErrorActionPreference = "Stop"
$env:CARGO_HOME = Join-Path (Get-Location) ".cache\cargo"
$env:TRANSLATOR_BUNDLE_DIR = (Get-Location).Path
cargo run --release -- --profile lite --open
