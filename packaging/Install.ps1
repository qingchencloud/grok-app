#Requires -Version 5.1
<#
.SYNOPSIS
  Install Grok Desktop for the current user (no admin required).

.DESCRIPTION
  Copies files next to this script into:
    %LOCALAPPDATA%\Programs\Grok Desktop\

  Creates Start Menu + Desktop shortcuts, and an Uninstall entry under HKCU.
#>
[CmdletBinding()]
param(
    [switch]$NoDesktopShortcut,
    [switch]$Silent
)

$ErrorActionPreference = "Stop"
try { chcp 65001 | Out-Null } catch {}

function Write-Step([string]$msg) {
    if (-not $Silent) { Write-Host ">> $msg" -ForegroundColor Cyan }
}

$SourceDir = $PSScriptRoot
if ([string]::IsNullOrEmpty($SourceDir)) { $SourceDir = (Get-Location).Path }

$ExeName = "GrokDesktop.exe"
$SourceExe = Join-Path $SourceDir $ExeName
if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "找不到 $ExeName（应与 Install.ps1 在同一目录）: $SourceExe"
}

$ProductName = "Grok Desktop"
$InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Grok Desktop"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Grok Desktop"
$DesktopLnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "$ProductName.lnk"
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GrokDesktop"

Write-Step "安装目录: $InstallRoot"
New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
New-Item -ItemType Directory -Force -Path $StartMenuDir | Out-Null

# Stop running instance
Get-Process -Name "GrokDesktop" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Step "关闭运行中的 GrokDesktop (pid=$($_.Id))"
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Milliseconds 300

$copyNames = @(
    "GrokDesktop.exe",
    "README.txt",
    "LICENSE.txt",
    "Uninstall.ps1",
    "Install.ps1"
)
foreach ($name in $copyNames) {
    $src = Join-Path $SourceDir $name
    if (Test-Path -LiteralPath $src) {
        Copy-Item -LiteralPath $src -Destination (Join-Path $InstallRoot $name) -Force
    }
}

$InstalledExe = Join-Path $InstallRoot $ExeName

function New-Shortcut([string]$Path, [string]$Target, [string]$WorkDir, [string]$Desc) {
    $w = New-Object -ComObject WScript.Shell
    $s = $w.CreateShortcut($Path)
    $s.TargetPath = $Target
    $s.WorkingDirectory = $WorkDir
    $s.Description = $Desc
    $s.IconLocation = "$Target,0"
    $s.Save()
}

Write-Step "创建开始菜单快捷方式"
New-Shortcut `
    -Path (Join-Path $StartMenuDir "$ProductName.lnk") `
    -Target $InstalledExe `
    -WorkDir $InstallRoot `
    -Desc "Grok 桌面客户端"

New-Shortcut `
    -Path (Join-Path $StartMenuDir "卸载 $ProductName.lnk") `
    -Target "powershell.exe" `
    -WorkDir $InstallRoot `
    -Desc "卸载 Grok Desktop" | Out-Null
# Fix uninstall shortcut args
$w = New-Object -ComObject WScript.Shell
$u = $w.CreateShortcut((Join-Path $StartMenuDir "卸载 $ProductName.lnk"))
$u.TargetPath = "powershell.exe"
$u.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $InstallRoot 'Uninstall.ps1')`""
$u.WorkingDirectory = $InstallRoot
$u.Save()

if (-not $NoDesktopShortcut) {
    Write-Step "创建桌面快捷方式"
    New-Shortcut -Path $DesktopLnk -Target $InstalledExe -WorkDir $InstallRoot -Desc "Grok 桌面客户端"
}

# Version from file if present
$version = "0.1.0"
$verFile = Join-Path $SourceDir "VERSION.txt"
if (Test-Path $verFile) { $version = (Get-Content -LiteralPath $verFile -Raw).Trim() }

$exeItem = Get-Item -LiteralPath $InstalledExe
Write-Step "写入卸载信息 (当前用户)"
New-Item -Path $UninstallKey -Force | Out-Null
Set-ItemProperty -Path $UninstallKey -Name "DisplayName" -Value $ProductName
Set-ItemProperty -Path $UninstallKey -Name "DisplayVersion" -Value $version
Set-ItemProperty -Path $UninstallKey -Name "Publisher" -Value "GrokApp"
Set-ItemProperty -Path $UninstallKey -Name "InstallLocation" -Value $InstallRoot
Set-ItemProperty -Path $UninstallKey -Name "DisplayIcon" -Value $InstalledExe
Set-ItemProperty -Path $UninstallKey -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $InstallRoot 'Uninstall.ps1')`""
Set-ItemProperty -Path $UninstallKey -Name "QuietUninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $InstallRoot 'Uninstall.ps1')`" -Silent"
Set-ItemProperty -Path $UninstallKey -Name "NoModify" -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallKey -Name "NoRepair" -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallKey -Name "EstimatedSize" -Value ([int]($exeItem.Length / 1KB)) -Type DWord

if (-not $Silent) {
    Write-Host ""
    Write-Host "安装完成: $InstalledExe" -ForegroundColor Green
    Write-Host "开始菜单: $ProductName"
    Write-Host ""
    Write-Host "首次使用前请在目标电脑安装 Grok CLI 并登录:" -ForegroundColor Yellow
    Write-Host "  irm https://x.ai/cli/install.ps1 | iex"
    Write-Host "  grok login"
    Write-Host ""
    $ans = Read-Host "现在启动 Grok Desktop? [Y/n]"
    if ([string]::IsNullOrWhiteSpace($ans) -or $ans -match '^[Yy]') {
        Start-Process -FilePath $InstalledExe -WorkingDirectory $InstallRoot
    }
}
