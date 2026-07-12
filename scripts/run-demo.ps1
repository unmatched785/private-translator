$ErrorActionPreference = "Stop"
$env:CARGO_HOME = Join-Path (Get-Location) ".cache\cargo"
$env:TRANSLATOR_BUNDLE_DIR = (Get-Location).Path
$env:TRANSLATOR_DATA_DIR = Join-Path (Get-Location) ".data\demo"
cargo run -- --profile demo --no-open
