param(
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

$orig = Get-Location
$repo = Resolve-Path "$PSScriptRoot\..\.."

try {
    Set-Location $repo

    Write-Host "=== VCorp distro: all targets ===" -ForegroundColor Cyan

    $childArgs = @{}
    if ($Clean) {
        $childArgs["Clean"] = $true

        Write-Host "`n[0/3] Clean caches" -ForegroundColor Yellow
        $desktop = Join-Path $repo "ui\desktop"
        foreach ($path in @(
                (Join-Path $desktop "out"),
                (Join-Path $desktop ".vite"),
                (Join-Path $PSScriptRoot "dist")
            )) {
            if (Test-Path $path) {
                Write-Host "Removing $path"
                Remove-Item -LiteralPath $path -Recurse -Force
            }
        }
        foreach ($name in @("goose.exe", "init-config.yaml")) {
            $staged = Join-Path $desktop "src\bin\$name"
            if (Test-Path $staged) {
                Write-Host "Removing $staged"
                Remove-Item -LiteralPath $staged -Force
            }
        }
    }

    Write-Host "`n[1/3] Windows CLI" -ForegroundColor Yellow
    & "$PSScriptRoot\build-windows.ps1" @childArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    Write-Host "`n[2/3] Windows GUI" -ForegroundColor Yellow
    & "$PSScriptRoot\build-windows-gui.ps1" @childArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $wsl = Get-Command wsl -ErrorAction SilentlyContinue
    if ($wsl) {
        Write-Host "`n[3/3] Linux CLI (WSL)" -ForegroundColor Yellow
        $linuxScript = if ($Clean) { "build-linux.sh" } else { "rebuild-linux.sh" }
        wsl -d Ubuntu -- bash "/mnt/c/working/goose/distro/vcorp/$linuxScript"
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        wsl -d Ubuntu -- bash /mnt/c/working/goose/distro/vcorp/package-linux.sh
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } else {
        Write-Host "`n[3/3] Linux CLI skipped (no WSL)" -ForegroundColor DarkYellow
    }

    Write-Host "`n=== All requested targets complete ===" -ForegroundColor Cyan
    Get-ChildItem "$PSScriptRoot\dist" -Recurse -File | Select-Object FullName, @{N='MB';E={[math]::Round($_.Length/1MB,1)}}
} finally {
    Set-Location $orig
}
