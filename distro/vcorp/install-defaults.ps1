# Copy system-config.yaml to the machine-wide Goose config.
# Goose reads %PROGRAMDATA%\goose\config.yaml before the user config.
# Run once per machine. Run again only when these defaults change.

$ErrorActionPreference = "Stop"

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    $proc = Start-Process -FilePath "powershell.exe" -Verb RunAs -Wait -PassThru -ArgumentList @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`""
    )
    if ($null -eq $proc) {
        exit 1
    }
    exit $proc.ExitCode
}

$source = Join-Path $PSScriptRoot "system-config.yaml"
if (-not (Test-Path -LiteralPath $source)) {
    throw "system-config.yaml was not found next to this script: $source"
}

$programData = $env:PROGRAMDATA
if ([string]::IsNullOrWhiteSpace($programData)) {
    $programData = "C:\ProgramData"
}

$destDir = Join-Path $programData "goose"
$dest = Join-Path $destDir "config.yaml"
New-Item -ItemType Directory -Force -Path $destDir | Out-Null

if (Test-Path -LiteralPath $dest) {
    $backup = Join-Path $destDir "config.yaml.bak"
    Copy-Item -LiteralPath $dest -Destination $backup -Force
    Write-Host "Backed up existing config to $backup"
}

Copy-Item -LiteralPath $source -Destination $dest -Force
Write-Host "Wrote $dest"
