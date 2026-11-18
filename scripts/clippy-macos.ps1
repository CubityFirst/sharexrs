# Lints the workspace for macOS (aarch64-apple-darwin) from Windows:
#   cargo clippy --workspace --all-targets --target aarch64-apple-darwin -- -D warnings
# with a stub C compiler and archiver (scripts/cc-stub.py) for the build
# scripts that compile C (libwebp-sys). Clippy doesn't link, so they only need
# to produce files. Extra arguments go to cargo clippy before `--`.
# Needs `rustup target add aarch64-apple-darwin` and Python on PATH.
$ErrorActionPreference = 'Stop'
$stub = Join-Path $PSScriptRoot 'cc-stub.py'
$python = (Get-Command python).Source
$env:CC_aarch64_apple_darwin = "$python $stub"
$env:AR_aarch64_apple_darwin = "$python $stub"
& cargo clippy --workspace --all-targets --target aarch64-apple-darwin @args -- -D warnings
exit $LASTEXITCODE
