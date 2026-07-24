#Requires -Version 5.1
<#
.SYNOPSIS
  Build release binary and produce portable + installer zip packages.

.OUTPUTS
  dist\GrokDesktop-<ver>-windows-x64-portable.zip
  dist\GrokDesktop-<ver>-windows-x64-setup.zip   (contains Install.ps1)
  dist\stage\...                                 (unpacked layout)
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$OpenDist,
    # Override Cargo.toml version for this package (e.g. CI: -Version 0.1.1)
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
try { chcp 65001 | Out-Null } catch {}
try {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $OutputEncoding = [System.Text.Encoding]::UTF8
} catch {}

$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "Cargo.toml"))) {
    $Root = $PSScriptRoot
    if (-not (Test-Path (Join-Path $Root "Cargo.toml"))) {
        throw "Cannot find Cargo.toml (run from repo packaging/ or repo root)"
    }
}
Set-Location -LiteralPath $Root

if (-not $Version) {
    $CargoToml = Get-Content -LiteralPath (Join-Path $Root "Cargo.toml") -Raw
    if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        $Version = "0.1.0"
    }
}
$Version = $Version.TrimStart('v')

$Arch = "x64"
$Product = "GrokDesktop"
$ExeName = "GrokDesktop.exe"
$ReleaseExe = Join-Path $Root "target\release\$ExeName"
$DistRoot = Join-Path $Root "dist"
$StageName = "$Product-$Version-windows-$Arch"
$StageDir = Join-Path $DistRoot "stage\$StageName"
$PortableZip = Join-Path $DistRoot "$StageName-portable.zip"
$SetupZip = Join-Path $DistRoot "$StageName-setup.zip"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Grok Desktop  ·  release packager" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Version : $Version"
Write-Host "  Root    : $Root"
Write-Host "  Out     : $DistRoot"
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if (-not $SkipBuild) {
    Write-Host ">> cargo build --release" -ForegroundColor Yellow
    $env:CARGO_TERM_COLOR = "always"
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed ($LASTEXITCODE)" }
} else {
    Write-Host ">> SkipBuild: using existing $ReleaseExe" -ForegroundColor DarkGray
}

if (-not (Test-Path -LiteralPath $ReleaseExe)) {
    throw "Release binary not found: $ReleaseExe"
}

$exeInfo = Get-Item -LiteralPath $ReleaseExe
Write-Host (">> Binary: {0:N1} MB  {1}" -f ($exeInfo.Length / 1MB), $ReleaseExe) -ForegroundColor Green

# Clean stage
if (Test-Path -LiteralPath $StageDir) {
    Remove-Item -LiteralPath $StageDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
New-Item -ItemType Directory -Force -Path $DistRoot | Out-Null

Copy-Item -LiteralPath $ReleaseExe -Destination (Join-Path $StageDir $ExeName) -Force
Copy-Item -LiteralPath (Join-Path $Root "packaging\Install.ps1") -Destination $StageDir -Force
Copy-Item -LiteralPath (Join-Path $Root "packaging\Uninstall.ps1") -Destination $StageDir -Force
Copy-Item -LiteralPath (Join-Path $Root "packaging\README.txt") -Destination $StageDir -Force
Copy-Item -LiteralPath (Join-Path $Root "packaging\LICENSE.txt") -Destination $StageDir -Force

Set-Content -LiteralPath (Join-Path $StageDir "VERSION.txt") -Value $Version -Encoding ASCII -NoNewline

# Launcher for portable users who double-click zip contents
$launchBat = @"
@echo off
cd /d "%~dp0"
start "" "%~dp0GrokDesktop.exe"
"@
Set-Content -LiteralPath (Join-Path $StageDir "Launch.bat") -Value $launchBat -Encoding ASCII

# Double-click friendly install entry (ASCII name avoids encoding issues)
$installBat = @"
@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo Installing Grok Desktop for current user...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install.ps1"
if errorlevel 1 pause
"@
Set-Content -LiteralPath (Join-Path $StageDir "Install.bat") -Value $installBat -Encoding ASCII

function New-ZipFromFolder([string]$SourceFolder, [string]$ZipPath) {
    if (Test-Path -LiteralPath $ZipPath) { Remove-Item -LiteralPath $ZipPath -Force }
    # Compress content with top-level folder name preserved
    $parent = Split-Path -Parent $SourceFolder
    $name = Split-Path -Leaf $SourceFolder
    Compress-Archive -Path (Join-Path $parent $name) -DestinationPath $ZipPath -CompressionLevel Optimal -Force
}

Write-Host ">> Creating portable zip..." -ForegroundColor Yellow
New-ZipFromFolder -SourceFolder $StageDir -ZipPath $PortableZip

# Setup zip is same layout (Install.ps1 included) — named for distribution clarity
Write-Host ">> Creating setup zip..." -ForegroundColor Yellow
Copy-Item -LiteralPath $PortableZip -Destination $SetupZip -Force

# SHA256 sums
function Write-Hash([string]$Path) {
    $h = Get-FileHash -LiteralPath $Path -Algorithm SHA256
    $line = "{0}  {1}" -f $h.Hash.ToLowerInvariant(), (Split-Path -Leaf $Path)
    Add-Content -LiteralPath (Join-Path $DistRoot "SHA256SUMS.txt") -Value $line -Encoding ASCII
    Write-Host "   $($h.Hash.Substring(0,16))…  $(Split-Path -Leaf $Path)" -ForegroundColor DarkGray
}

$sums = Join-Path $DistRoot "SHA256SUMS.txt"
if (Test-Path $sums) { Remove-Item $sums -Force }
Write-Hash $PortableZip
Write-Hash $SetupZip

Write-Host ""
Write-Host "DONE" -ForegroundColor Green
Write-Host "  portable : $PortableZip"
Write-Host "  setup    : $SetupZip  (unzip then run Install.bat)"
Write-Host "  staged   : $StageDir"
Write-Host ""
Write-Host "On another PC:" -ForegroundColor Cyan
Write-Host "  1. Unzip"
Write-Host "  2. Double-click Install.bat"
Write-Host "  3. Install grok CLI + grok login on that machine"
Write-Host ""

if ($OpenDist) {
    Start-Process explorer.exe -ArgumentList $DistRoot
}
