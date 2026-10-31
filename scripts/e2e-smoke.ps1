$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $Root "src-tauri")
cargo run --quiet --example e2e_smoke
