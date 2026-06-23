<#
.SYNOPSIS
    Uninstall nglimit from Windows
.DESCRIPTION
    Removes the nglimit binary and optionally the config directory.
.PARAMETER RemoveConfig
    Also remove the .nglimit config directory.
.EXAMPLE
    .\uninstall.ps1
    .\uninstall.ps1 -RemoveConfig
#>
param(
    [switch]$RemoveConfig
)

$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\nglimit"
$BinDir = "$InstallDir\bin"
$ConfigDir = "$env:USERPROFILE\.nglimit"

Write-Host "nglimit uninstaller" -ForegroundColor Cyan
Write-Host ""

# Remove binary
if (Test-Path $BinDir) {
    Remove-Item -Recurse -Force $BinDir
    Write-Host "Removed $BinDir" -ForegroundColor Green
}

if (Test-Path $InstallDir) {
    Remove-Item -Recurse -Force $InstallDir
    Write-Host "Removed $InstallDir" -ForegroundColor Green
}

# Remove from PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -like "*$BinDir*") {
    $newPath = ($userPath -split ";" | Where-Object { $_ -ne $BinDir }) -join ";"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Removed from user PATH" -ForegroundColor Green
}

# Remove config
if ($RemoveConfig -and (Test-Path $ConfigDir)) {
    Remove-Item -Recurse -Force $ConfigDir
    Write-Host "Removed $ConfigDir" -ForegroundColor Green
} elseif (Test-Path $ConfigDir) {
    Write-Host "Config kept at $ConfigDir (use -RemoveConfig to delete)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "nglimit uninstalled." -ForegroundColor Green
