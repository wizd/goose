param(
    [switch]$Clean
)

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

$orig = Get-Location
$repo = Resolve-Path "$PSScriptRoot\..\.."

try {
    Set-Location $repo
    if ($Clean) {
        Write-Host "CLEAN cargo (windows-gnu release)"
        cargo +1.96.1-x86_64-pc-windows-gnu clean
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    Write-Host "START_BUILD $(Get-Location)"
    # windows-gnu has no rusty_v8 / llama.cpp prebuilts (those need MSVC).
    # This still bundles the VCorp declarative provider.
    cargo +1.96.1-x86_64-pc-windows-gnu build --release -p goose-cli --bin goose --no-default-features --features "aws-providers,telemetry,nostr,otel,rustls-tls,live-voice,system-keyring,update"
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $exe = Join-Path $repo "target\release\goose.exe"
    if (-not (Test-Path $exe)) {
        Write-Error "Binary not found: $exe"
        exit 1
    }

    $out = Join-Path $PSScriptRoot "dist\windows-x86_64"
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    $dst = Join-Path $out "goose.exe"
    Copy-Item -Force $exe $dst
    if (Get-Command strip.exe -ErrorAction SilentlyContinue) {
        & strip.exe $dst
    }
    Copy-Item -Force (Join-Path $PSScriptRoot "init-config.yaml") (Join-Path $out "init-config.yaml")
    Copy-Item -Force (Join-Path $PSScriptRoot "README.md") (Join-Path $out "README.md")
    $zip = Join-Path $out "goose-vcorp-windows-x86_64.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $dst, (Join-Path $out "init-config.yaml"), (Join-Path $out "README.md") -DestinationPath $zip -Force

    Get-Item $dst | Format-List FullName, Length, LastWriteTime
    Write-Host "BUILD_OK"
} finally {
    Set-Location $orig
}
