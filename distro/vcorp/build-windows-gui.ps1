$ErrorActionPreference = "Stop"

$repo = Resolve-Path "$PSScriptRoot\..\.."
$desktop = Join-Path $repo "ui\desktop"
$binDir = Join-Path $desktop "src\bin"
$gooseSrc = Join-Path $repo "distro\vcorp\dist\windows-x86_64\goose.exe"
if (-not (Test-Path $gooseSrc)) {
    $gooseSrc = Join-Path $repo "target\release\goose.exe"
}
if (-not (Test-Path $gooseSrc)) {
    throw "goose.exe not found. Build the CLI first."
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item -Force $gooseSrc (Join-Path $binDir "goose.exe")
Copy-Item -Force (Join-Path $PSScriptRoot "init-config.yaml") (Join-Path $binDir "init-config.yaml")
Write-Host "Staged $(Get-Item (Join-Path $binDir 'goose.exe') | Select-Object -ExpandProperty Length) byte goose.exe"

$env:ELECTRON_PLATFORM = "win32"
$env:ELECTRON_ARCH = "x64"
Set-Location $desktop

Write-Host "Installing UI dependencies..."
$env:CI = "true"
pnpm install --frozen-lockfile --trust-lockfile
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Preparing Windows platform binaries..."
node scripts/prepare-platform-binaries.js
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Packaging Electron app..."
pnpm run make --platform=win32 --arch=x64
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$outApp = Join-Path $desktop "out\Goose-win32-x64"
if (-not (Test-Path (Join-Path $outApp "Goose.exe"))) {
    throw "Goose.exe not found in $outApp"
}

$dist = Join-Path $repo "distro\vcorp\dist\windows-x86_64-gui"
New-Item -ItemType Directory -Force -Path $dist | Out-Null
$zip = Join-Path $dist "Goose-vcorp-win32-x64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $outApp "*") -DestinationPath $zip -Force

Write-Host "GUI_BUILD_OK"
Get-Item (Join-Path $outApp "Goose.exe") | Format-List FullName, Length, LastWriteTime
Get-Item $zip | Format-List FullName, Length
