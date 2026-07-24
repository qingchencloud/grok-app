@echo off
chcp 65001 >nul
title Grok Desktop
cd /d "%~dp0"

echo ========================================
echo   Grok Desktop 启动器
echo ========================================
echo.
echo 工作目录: %CD%
echo 程序路径: %CD%\target\debug\GrokDesktop.exe
echo 崩溃日志: %APPDATA%\GrokApp\crash.log
echo.

if not exist "target\debug\GrokDesktop.exe" (
  echo [!] 还没有编译，正在 cargo build ...
  cargo build
  if errorlevel 1 (
    echo [x] 编译失败
    pause
    exit /b 1
  )
)

echo [>] 正在启动...
echo.

target\debug\GrokDesktop.exe
set EXITCODE=%ERRORLEVEL%

echo.
echo ----------------------------------------
echo 程序已退出，代码: %EXITCODE%
if exist "%APPDATA%\GrokApp\crash.log" (
  echo.
  echo --- crash.log 最后 20 行 ---
  powershell -NoProfile -Command "Get-Content -Tail 20 $env:APPDATA\GrokApp\crash.log"
)
echo.
echo 窗口关掉后按任意键关闭此黑框...
pause >nul
