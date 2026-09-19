$ErrorActionPreference = "Stop"

$env:Path = @(
    "$env:USERPROFILE\.cargo\bin",
    "$env:USERPROFILE\scoop\apps\mingw\current\bin",
    "$env:USERPROFILE\scoop\shims",
    "$env:USERPROFILE\scoop\apps\cmake\current\bin",
    $env:Path
) -join ";"

$env:CC = "gcc"
$env:CXX = "g++"
$env:RUSTUP_TOOLCHAIN = "1.96.1-x86_64-pc-windows-gnu"

Write-Host "rustc: $((rustc --version))"
Write-Host "gcc: $((gcc --version | Select-Object -First 1))"
Write-Host "cmake: $((cmake --version | Select-Object -First 1))"

Set-Location (Resolve-Path "$PSScriptRoot\..\..")
Write-Host "START_BUILD $(Get-Location)"
# windows-gnu has no rusty_v8 / llama.cpp prebuilts (those need MSVC).
# This still bundles the VCorp declarative provider.
cargo +1.96.1-x86_64-pc-windows-gnu build --release -p goose-cli --bin goose --no-default-features --features "aws-providers,telemetry,nostr,otel,rustls-tls,live-voice,system-keyring,update"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = "target\release\goose.exe"
if (-not (Test-Path $exe)) {
    Write-Error "Binary not found: $exe"
    exit 1
}
Get-Item $exe | Format-List FullName, Length, LastWriteTime
Write-Host "BUILD_OK"
