@echo off
REM English-friendly launcher (same as 启动Grok.bat)
chcp 65001 >nul
title Grok Desktop
cd /d "%~dp0"

if not exist "target\debug\GrokDesktop.exe" (
  echo Building...
  cargo build
  if errorlevel 1 (
    echo Build failed
    pause
    exit /b 1
  )
)

target\debug\GrokDesktop.exe
set EXITCODE=%ERRORLEVEL%
echo Exit code: %EXITCODE%
if exist "%APPDATA%\GrokApp\crash.log" (
  powershell -NoProfile -Command "Get-Content -Tail 15 $env:APPDATA\GrokApp\crash.log"
)
pause
exit /b %EXITCODE%
