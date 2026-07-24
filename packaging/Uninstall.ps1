#Requires -Version 5.1
<#
.SYNOPSIS
  Uninstall Grok Desktop (current-user install).
#>
[CmdletBinding()]
param([switch]$Silent)

$ErrorActionPreference = "Continue"
try { chcp 65001 | Out-Null } catch {}

$ProductName = "Grok Desktop"
$InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Grok Desktop"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Grok Desktop"
$DesktopLnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "$ProductName.lnk"
$UninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GrokDesktop"

if (-not $Silent) {
    Write-Host "即将卸载 $ProductName" -ForegroundColor Yellow
    Write-Host "目录: $InstallRoot"
    $ans = Read-Host "确认卸载? [y/N]"
    if ($ans -notmatch '^[Yy]') {
        Write-Host "已取消"
        exit 0
    }
}

Get-Process -Name "GrokDesktop" -ErrorAction SilentlyContinue | ForEach-Object {
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Milliseconds 400

if (Test-Path -LiteralPath $DesktopLnk) {
    Remove-Item -LiteralPath $DesktopLnk -Force -ErrorAction SilentlyContinue
}
if (Test-Path -LiteralPath $StartMenuDir) {
    Remove-Item -LiteralPath $StartMenuDir -Recurse -Force -ErrorAction SilentlyContinue
}
if (Test-Path -LiteralPath $UninstallKey) {
    Remove-Item -Path $UninstallKey -Recurse -Force -ErrorAction SilentlyContinue
}

# Remove install dir (this script may live inside it)
$self = $MyInvocation.MyCommand.Path
if ($self -and ($self.StartsWith($InstallRoot, [System.StringComparison]::OrdinalIgnoreCase))) {
    # Schedule delete after process exits
    $cmd = @"
Start-Sleep -Seconds 1
Remove-Item -LiteralPath '$InstallRoot' -Recurse -Force -ErrorAction SilentlyContinue
"@
    Start-Process powershell.exe -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $cmd) -WindowStyle Hidden
} elseif (Test-Path -LiteralPath $InstallRoot) {
    Remove-Item -LiteralPath $InstallRoot -Recurse -Force -ErrorAction SilentlyContinue
}

if (-not $Silent) {
    Write-Host "已卸载 $ProductName" -ForegroundColor Green
    Read-Host "按回车退出"
}
