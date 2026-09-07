# Dot-source this file to use project-local tools: . ./scripts/dev-env.ps1
$whistleBoxRoot = Split-Path -Parent $PSScriptRoot
$env:CARGO_HOME = Join-Path $whistleBoxRoot '.tooling\cargo'
$env:RUSTUP_HOME = Join-Path $whistleBoxRoot '.tooling\rustup'
$env:RUSTUP_DIST_SERVER = 'https://rsproxy.cn'
$env:RUSTUP_UPDATE_ROOT = 'https://rsproxy.cn/rustup'
$env:CARGO_HTTP_TIMEOUT = '120'
$env:CARGO_HTTP_MULTIPLEXING = 'false'
$env:PATH = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:PATH
$env:npm_config_cache = Join-Path $whistleBoxRoot '.tooling\npm-cache'
$env:npm_config_registry = 'https://registry.npmmirror.com'
