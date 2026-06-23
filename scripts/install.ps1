<#
.SYNOPSIS
    Install nglimit for Windows
.DESCRIPTION
    Downloads the latest nglimit binary or builds from source.
    Adds to PATH and creates a default .env config.
.PARAMETER BuildFromSource
    Build from source using Cargo instead of downloading a release binary.
.EXAMPLE
    .\install.ps1
    .\install.ps1 -BuildFromSource
#>
param(
    [switch]$BuildFromSource
)

$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\nglimit"
$BinDir = "$InstallDir\bin"

Write-Host "nglimit installer for Windows" -ForegroundColor Cyan
Write-Host ""

# Ensure install directory exists
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
}

if ($BuildFromSource) {
    Write-Host "Building from source..." -ForegroundColor Yellow
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "Error: cargo not found. Install Rust from https://rustup.rs" -ForegroundColor Red
        exit 1
    }
    $repoDir = Join-Path $env:TEMP "neurogate-limit-watch"
    if (Test-Path $repoDir) { Remove-Item -Recurse -Force $repoDir }
    git clone https://github.com/xodapi/neurogate-limit-watch.git $repoDir
    Push-Location $repoDir
    cargo build --release
    Copy-Item "target\release\nglimit.exe" $BinDir
    Pop-Location
    Remove-Item -Recurse -Force $repoDir
} else {
    Write-Host "Downloading latest release..." -ForegroundColor Yellow
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/xodapi/neurogate-limit-watch/releases/latest"
    $asset = $release.assets | Where-Object { $_.name -like "*windows*x86_64*" -or $_.name -like "*windows*msvc*" } | Select-Object -First 1
    if (-not $asset) {
        Write-Host "No Windows binary found in latest release. Trying --BuildFromSource..." -ForegroundColor Yellow
        & $PSCommandPath -BuildFromSource
        return
    }
    $zipPath = Join-Path $env:TEMP "nglimit.zip"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath
    Expand-Archive -Path $zipPath -DestinationPath $BinDir -Force
    Remove-Item $zipPath
}

# Add to user PATH if not already there
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$BinDir", "User")
    Write-Host "Added $BinDir to user PATH" -ForegroundColor Green
    $env:Path = "$env:Path;$BinDir"
}

# Create default .env if not present
$EnvFile = "$env:USERPROFILE\.nglimit\.env"
if (-not (Test-Path $EnvFile)) {
    $envDir = Split-Path $EnvFile
    if (-not (Test-Path $envDir)) { New-Item -ItemType Directory -Path $envDir -Force | Out-Null }
    @"
# nglimit configuration
# Get your API key from NeuroGate dashboard
NEUROGATE_API_KEY=
NEUROGATE_API_BASE=https://api.neurogate.space
"@ | Set-Content $EnvFile
    Write-Host "Created default config at $EnvFile" -ForegroundColor Green
    Write-Host "  Edit it and add your NEUROGATE_API_KEY" -ForegroundColor Yellow
}

# Verify
$nglimit = Get-Command nglimit -ErrorAction SilentlyContinue
if ($nglimit) {
    Write-Host ""
    Write-Host "Installation complete!" -ForegroundColor Green
    & nglimit --version
    Write-Host ""
    Write-Host "Quick start:" -ForegroundColor Cyan
    Write-Host "  nglimit --demo              # test with demo data"
    Write-Host "  nglimit --monitor           # live dashboard"
    Write-Host "  nglimit --monitor --preset compact  # narrow terminal"
} else {
    Write-Host ""
    Write-Host "Installed to $BinDir" -ForegroundColor Green
    Write-Host "Restart your terminal or run:" -ForegroundColor Yellow
    Write-Host "  `$env:Path += `";$BinDir`""
    Write-Host "  nglimit --version"
}
