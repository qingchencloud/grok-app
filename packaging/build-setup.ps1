#Requires -Version 5.1
<#
.SYNOPSIS
  Build GrokDesktop.exe (optional) and produce a single Setup.exe via Inno Setup.

.OUTPUTS
  dist\GrokDesktop-Setup-<ver>-windows-x64.exe   ← double-click to install
  dist\GrokDesktop-<ver>-windows-x64.exe         ← portable single file (optional)
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$Version = "",
    [switch]$PortableExeOnly
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "Cargo.toml"))) {
    throw "Cargo.toml not found"
}
Set-Location -LiteralPath $Root

if (-not $Version) {
    $toml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
    if ($toml -match 'version\s*=\s*"([^"]+)"') { $Version = $Matches[1] } else { $Version = "0.1.0" }
}
$Version = $Version.TrimStart("v")

$ReleaseExe = Join-Path $Root "target\release\GrokDesktop.exe"
$Dist = Join-Path $Root "dist"
$Stage = Join-Path $Root "packaging\stage"
New-Item -ItemType Directory -Force -Path $Dist, $Stage | Out-Null

if (-not $SkipBuild) {
    Write-Host ">> cargo build --release" -ForegroundColor Yellow
    cargo build --release --bin GrokDesktop
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
if (-not (Test-Path $ReleaseExe)) { throw "Missing $ReleaseExe" }

# Stage for Inno
Copy-Item $ReleaseExe (Join-Path $Stage "GrokDesktop.exe") -Force
foreach ($f in @("LICENSE.txt", "README.txt", "Uninstall.ps1")) {
    $src = Join-Path $Root "packaging\$f"
    if (Test-Path $src) { Copy-Item $src $Stage -Force }
}
if (Test-Path (Join-Path $Root "LICENSE")) {
    Copy-Item (Join-Path $Root "LICENSE") (Join-Path $Stage "LICENSE.txt") -Force
}
Set-Content (Join-Path $Stage "VERSION.txt") -Value $Version -NoNewline -Encoding ascii

# Portable single-file rename (same binary)
$PortableName = "GrokDesktop-$Version-windows-x64.exe"
Copy-Item $ReleaseExe (Join-Path $Dist $PortableName) -Force
Write-Host ">> Portable: dist\$PortableName" -ForegroundColor Green

if ($PortableExeOnly) { return }

# Find Inno Setup compiler
$iscc = @(
    "${env:LocalAppData}\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc) {
    Write-Host ">> Inno Setup not found — trying winget/choco..." -ForegroundColor Yellow
    if (Get-Command choco -ErrorAction SilentlyContinue) {
        choco install innosetup -y --no-progress
    } elseif (Get-Command winget -ErrorAction SilentlyContinue) {
        winget install --id JRSoftware.InnoSetup -e --accept-source-agreements --accept-package-agreements
    } else {
        throw "Install Inno Setup 6 from https://jrsoftware.org/isinfo.php then re-run"
    }
    $iscc = @(
        "${env:LocalAppData}\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw "ISCC.exe still not found after install attempt" }

Write-Host ">> ISCC: $iscc" -ForegroundColor Cyan
& $iscc "/DMyAppVersion=$Version" (Join-Path $Root "packaging\setup.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }

$setup = Join-Path $Dist "GrokDesktop-Setup-$Version-windows-x64.exe"
if (-not (Test-Path $setup)) {
    # Inno may write to packaging\..\dist
    $found = Get-ChildItem $Dist -Filter "GrokDesktop-Setup-*.exe" | Select-Object -First 1
    if ($found) { $setup = $found.FullName }
}
Write-Host ""
Write-Host "DONE" -ForegroundColor Green
Write-Host "  Installer : $setup   ← 双击安装"
Write-Host "  Portable  : dist\$PortableName   ← 单文件直接运行"
Write-Host ""
