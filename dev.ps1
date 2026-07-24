#Requires -Version 5.1
<#
.SYNOPSIS
  Grok Desktop 开发启动器：改代码自动重编并重启（热重载式调试）。

.DESCRIPTION
  默认 watch 模式：轮询监视 src/、tests/、examples/、Cargo.toml，
  保存后防抖 → cargo build → 杀掉旧进程 → 再启动 GrokDesktop.exe。

  用法:
    .\dev.ps1                 # 监视 + 自动重启（推荐）
    .\dev.ps1 -Once           # 只编译并启动一次
    .\dev.ps1 -Test           # 改代码后跑单元测试（不启 GUI）
    .\dev.ps1 -Release        # 用 release 配置
    .\dev.ps1 -NoRun          # 只编译，不启动窗口
    .\dev.ps1 -Interval 0.8   # 防抖秒数（默认 0.6）

  快捷键（watch 时在此控制台）:
    R  立即重新编译并重启
    T  跑单元测试
    L  打印 crash.log 尾部
    Q  退出（会关掉 app 进程）
    H  帮助
#>

[CmdletBinding()]
param(
    [switch]$Once,
    [switch]$Test,
    [switch]$Release,
    [switch]$NoRun,
    [double]$Interval = 0.6,
    [string]$CargoArgs = ""
)

$ErrorActionPreference = "Continue"
try { chcp 65001 | Out-Null } catch {}
try {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    $OutputEncoding = [System.Text.Encoding]::UTF8
} catch {}

$Root = $PSScriptRoot
if ([string]::IsNullOrEmpty($Root)) { $Root = (Get-Location).Path }
Set-Location -LiteralPath $Root

$ProfileName = if ($Release) { "release" } else { "debug" }
$ExePath = Join-Path $Root "target\$ProfileName\GrokDesktop.exe"

# Shared state (single runspace — polling only, no FSW cross-runspace issues)
$State = @{
    AppProcess = $null
    RunId      = 0
    LastFp     = ""
    ExitedHint = $false
    EventJobs  = @()
}

function Write-Banner {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  Grok Desktop  ·  dev.ps1" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  目录: $Root"
    Write-Host "  配置: $ProfileName"
    Write-Host "  产物: $ExePath"
    Write-Host "  日志: $env:APPDATA\GrokApp\crash.log"
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Info([string]$msg) {
    Write-Host ("[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $msg) -ForegroundColor DarkGray
}
function Write-Ok([string]$msg) {
    Write-Host ("[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $msg) -ForegroundColor Green
}
function Write-Warn([string]$msg) {
    Write-Host ("[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $msg) -ForegroundColor Yellow
}
function Write-ErrMsg([string]$msg) {
    Write-Host ("[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $msg) -ForegroundColor Red
}

function Show-Help {
    Write-Host @"

  快捷键 (本控制台):
    R  立即重新编译并重启
    T  跑 cargo test --test core_logic
    L  打印 crash.log 最后 25 行
    Q  退出 dev（并结束 app）
    H  显示本帮助

  监视: src\  tests\  examples\  Cargo.toml
  保存 .rs / .toml 后约 ${Interval}s 自动 rebuild + restart

"@ -ForegroundColor Gray
}

function Clear-AppEvents {
    foreach ($j in @($State.EventJobs)) {
        try {
            Unregister-Event -SourceIdentifier $j.Name -ErrorAction SilentlyContinue
            Remove-Job -Id $j.Id -Force -ErrorAction SilentlyContinue
        } catch {}
    }
    $State.EventJobs = @()
}

function Stop-App {
    Clear-AppEvents
    $p = $State.AppProcess
    if ($null -ne $p) {
        try {
            if (-not $p.HasExited) {
                Write-Info "停止 GrokDesktop (pid=$($p.Id)) …"
                Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
                # Give Windows a moment to unlock the exe for rebuild
                $deadline = (Get-Date).AddSeconds(3)
                while (-not $p.HasExited -and (Get-Date) -lt $deadline) {
                    Start-Sleep -Milliseconds 50
                }
            }
        } catch {}
        try { $p.Dispose() } catch {}
        $State.AppProcess = $null
    }

    # Leftover processes started from this tree
    Get-Process -Name "GrokDesktop","grok_app" -ErrorAction SilentlyContinue | ForEach-Object {
        $proc = $_
        try {
            $path = $null
            try { $path = $proc.Path } catch {}
            if ($path -and $path.StartsWith($Root, [StringComparison]::OrdinalIgnoreCase)) {
                Write-Info "清理残留 $($proc.ProcessName) pid=$($proc.Id)"
                Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            }
        } catch {}
    }
    Start-Sleep -Milliseconds 200
}

function Invoke-CargoBuild {
    $argList = New-Object System.Collections.Generic.List[string]
    [void]$argList.Add("build")
    if ($Release) { [void]$argList.Add("--release") }
    if (-not [string]::IsNullOrWhiteSpace($CargoArgs)) {
        foreach ($a in ($CargoArgs -split "\s+")) {
            if ($a) { [void]$argList.Add($a) }
        }
    }

    Write-Info ("cargo " + ($argList -join " "))
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & cargo @($argList.ToArray())
    $code = $LASTEXITCODE
    $sw.Stop()
    if ($code -ne 0) {
        Write-ErrMsg ("编译失败 (exit={0}, {1}s) — 修好保存后会再试" -f $code, $sw.Elapsed.TotalSeconds.ToString("0.0"))
        return $false
    }
    Write-Ok ("编译成功 ({0}s)" -f $sw.Elapsed.TotalSeconds.ToString("0.0"))
    return $true
}

function Invoke-CargoTest {
    Write-Info "cargo test --test core_logic"
    $argList = @("test", "--test", "core_logic")
    if ($Release) { $argList += "--release" }
    & cargo @argList
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "测试通过"
    } else {
        Write-ErrMsg "测试失败 (exit=$LASTEXITCODE)"
    }
}

function Start-App {
    if ($NoRun) {
        Write-Info "NoRun: 不启动 GUI"
        return
    }
    if (-not (Test-Path -LiteralPath $ExePath)) {
        Write-ErrMsg "找不到可执行文件: $ExePath"
        return
    }

    Write-Info "启动 $ExePath"
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.WorkingDirectory = $Root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $false

    # PS 5.1 uses EnvironmentVariables; PS 7+ also accepts Environment
    try {
        $psi.EnvironmentVariables["RUST_BACKTRACE"] = "1"
        if (-not $psi.EnvironmentVariables["RUST_LOG"]) {
            $psi.EnvironmentVariables["RUST_LOG"] = "info"
        }
    } catch {
        try {
            $psi.Environment["RUST_BACKTRACE"] = "1"
            if (-not $psi.Environment.ContainsKey("RUST_LOG")) {
                $psi.Environment["RUST_LOG"] = "info"
            }
        } catch {}
    }

    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    $p.EnableRaisingEvents = $true

    # Stream stdout/stderr into this console
    $outAction = {
        $line = $EventArgs.Data
        if (-not [string]::IsNullOrEmpty($line)) {
            Write-Host $line
        }
    }
    $errAction = {
        $line = $EventArgs.Data
        if (-not [string]::IsNullOrEmpty($line)) {
            Write-Host $line -ForegroundColor DarkYellow
        }
    }
    $j1 = Register-ObjectEvent -InputObject $p -EventName OutputDataReceived -Action $outAction
    $j2 = Register-ObjectEvent -InputObject $p -EventName ErrorDataReceived -Action $errAction
    $State.EventJobs = @($j1, $j2)

    [void]$p.Start()
    $p.BeginOutputReadLine()
    $p.BeginErrorReadLine()
    $State.AppProcess = $p
    $State.RunId++
    $State.ExitedHint = $false
    Write-Ok ("已启动  pid={0}  run#{1}" -f $p.Id, $State.RunId)
}

function Show-CrashLog {
    $path = Join-Path $env:APPDATA "GrokApp\crash.log"
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Warn "尚无 crash.log ($path)"
        return
    }
    Write-Host "--- crash.log (tail 25) ---" -ForegroundColor Magenta
    Get-Content -LiteralPath $path -Tail 25 -ErrorAction SilentlyContinue
    Write-Host "---------------------------" -ForegroundColor Magenta
}

function Invoke-RebuildAndRestart {
    param([string]$Reason = "manual")
    Write-Host ""
    Write-Info "触发重建: $Reason"
    Stop-App
    if ($Test) {
        Invoke-CargoTest
        return
    }
    if (Invoke-CargoBuild) {
        Start-App
    }
}

function Get-SourceFingerprint {
    $parts = New-Object System.Collections.Generic.List[string]
    foreach ($dir in @("src", "tests", "examples")) {
        $p = Join-Path $Root $dir
        if (-not (Test-Path -LiteralPath $p)) { continue }
        Get-ChildItem -LiteralPath $p -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Extension -match '\.(rs|toml)$' } |
            ForEach-Object {
                [void]$parts.Add(("{0}|{1}|{2}" -f $_.FullName, $_.Length, $_.LastWriteTimeUtc.Ticks))
            }
    }
    foreach ($f in @("Cargo.toml", "Cargo.lock")) {
        $p = Join-Path $Root $f
        if (Test-Path -LiteralPath $p) {
            $item = Get-Item -LiteralPath $p
            [void]$parts.Add(("{0}|{1}|{2}" -f $item.FullName, $item.Length, $item.LastWriteTimeUtc.Ticks))
        }
    }
    $parts.Sort()
    return ($parts -join "`n")
}

# ---------- main ----------
Write-Banner

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-ErrMsg "未找到 cargo，请先安装 Rust: https://rustup.rs"
    exit 1
}

if ($Test -and $Once) {
    Invoke-CargoTest
    exit $LASTEXITCODE
}

Invoke-RebuildAndRestart -Reason "initial"
$State.LastFp = Get-SourceFingerprint

if ($Once) {
    if ($NoRun -or $Test) { exit 0 }
    Write-Info "Once 模式：等待 app 退出 … (Ctrl+C 可结束)"
    try {
        while ($null -ne $State.AppProcess -and -not $State.AppProcess.HasExited) {
            Start-Sleep -Milliseconds 300
        }
    } finally {
        Stop-App
    }
    Show-CrashLog
    exit 0
}

# ---- Watch mode ----
Show-Help
Write-Ok "Watch 已开启 — 改代码保存即可自动重启。Ctrl+C 或按 Q 退出。"
Write-Host ""

$pendingSince = $null
$pendingReason = $null
$shouldExit = $false

try {
    while (-not $shouldExit) {
        # Keyboard (when console has focus)
        if ([Console]::KeyAvailable) {
            $key = [Console]::ReadKey($true)
            $ch = $key.KeyChar
            $k = $key.Key
            if ($k -eq [ConsoleKey]::R -or $ch -eq "r" -or $ch -eq "R") {
                $pendingSince = $null
                Invoke-RebuildAndRestart -Reason "key:R"
                $State.LastFp = Get-SourceFingerprint
            }
            elseif ($k -eq [ConsoleKey]::T -or $ch -eq "t" -or $ch -eq "T") {
                Invoke-CargoTest
            }
            elseif ($k -eq [ConsoleKey]::L -or $ch -eq "l" -or $ch -eq "L") {
                Show-CrashLog
            }
            elseif ($k -eq [ConsoleKey]::H -or $ch -eq "h" -or $ch -eq "H") {
                Show-Help
            }
            elseif ($k -eq [ConsoleKey]::Q -or $ch -eq "q" -or $ch -eq "Q" -or $k -eq [ConsoleKey]::Escape) {
                Write-Info "退出…"
                $shouldExit = $true
                break
            }
        }

        # Poll source fingerprint
        $fp = Get-SourceFingerprint
        if ($fp -ne $State.LastFp) {
            if ($null -eq $pendingSince) {
                $pendingSince = Get-Date
                $pendingReason = "source changed"
                Write-Info "检测到文件变更，${Interval}s 后重建…"
            }
        }

        if ($null -ne $pendingSince) {
            $elapsed = ((Get-Date) - $pendingSince).TotalSeconds
            if ($elapsed -ge $Interval) {
                # Re-read after debounce so mid-save bursts settle
                $State.LastFp = Get-SourceFingerprint
                $reason = $pendingReason
                $pendingSince = $null
                $pendingReason = $null
                Invoke-RebuildAndRestart -Reason $reason
                $State.LastFp = Get-SourceFingerprint
            }
        }

        # App closed by user
        if ($null -ne $State.AppProcess -and $State.AppProcess.HasExited -and -not $State.ExitedHint) {
            $code = $State.AppProcess.ExitCode
            Write-Warn "app 已退出 (code=$code)。改代码或按 R 重新启动。"
            Show-CrashLog
            Clear-AppEvents
            $State.AppProcess = $null
            $State.ExitedHint = $true
        }

        Start-Sleep -Milliseconds 200
    }
}
finally {
    Write-Info "清理…"
    Stop-App
    Write-Ok "dev.ps1 已结束"
}
