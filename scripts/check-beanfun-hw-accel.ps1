#Requires -Version 5.1
<#
.SYNOPSIS
  Check Beanfun hardware-accel from the OS: config file, global WPF registry, WebView2 child command lines, GPU engine counters.

.DESCRIPTION
  Microsoft documents (Graphics Rendering Registry Settings) that per-process RenderOptions.ProcessRenderMode
  is NOT readable from outside that process. This script uses what CAN be observed from the system:
  - Config.xml: saved checkbox (intent).
  - HKCU\Software\Microsoft\Avalon.Graphics\DisableHWAcceleration: global WPF override for ALL apps (Beanfun's in-app option does NOT set this unless you set it manually).
  - Child processes of Beanfun: msedgewebview2.exe CommandLine should contain --disable-gpu when our option is on and a WebView was opened.
  - GPU Engine performance counters: instances named like pid_<PID> (see DirectX blog / Task Manager GPU); non-zero suggests that PID is using GPU for 3D/Copy/Video etc.

.PARAMETER Quick
  Only config + registry + Beanfun process list (no WMI children, no GPU counters).

.PARAMETER NoGpu
  Skip GPU Engine counter sampling.

.EXAMPLE
  .\check-beanfun-hw-accel.ps1
  .\check-beanfun-hw-accel.ps1 -Quick
#>
param(
    [switch]$Quick,
    [switch]$NoGpu
)

$ErrorActionPreference = 'Continue'

Write-Host ''
Write-Host '=== Limitation (WPF) ===' -ForegroundColor DarkYellow
Write-Host 'Beanfun uses RenderOptions.ProcessRenderMode in-process. Windows does not expose that per-app to other tools.'
Write-Host 'Verification here: registry (global WPF only), WebView2 cmdline, GPU counters (indirect).'
Write-Host ''

# --- 1. Config ---
$configPath = Join-Path $env:APPDATA 'Beanfun\Config.xml'
Write-Host '=== 1. Config.xml (saved checkbox / intent) ===' -ForegroundColor Cyan
$disableRequested = $false
if (-not (Test-Path -LiteralPath $configPath)) {
    Write-Host "Not found: $configPath" -ForegroundColor Yellow
} else {
    try {
        [xml]$doc = Get-Content -LiteralPath $configPath -Raw
        $adds = @($doc.configuration.appSettings.add)
        $entry = $adds | Where-Object { $_.key -eq 'disableHardwareAcceleration' } | Select-Object -First 1
        if (-not $entry) {
            Write-Host 'disableHardwareAcceleration: missing -> false'
        } else {
            $raw = $entry.value
            Write-Host "disableHardwareAcceleration: `"$raw`""
            if ($null -ne $raw -and ($raw.Trim() -ieq 'true')) { $disableRequested = $true }
        }
    } catch {
        Write-Host "XML error: $_" -ForegroundColor Red
    }
}
Write-Host 'If true: after full restart, app intends software WPF + WebView2 with --disable-gpu when embedded browser is created.'

# --- 2. Global WPF registry (all WPF apps) ---
Write-Host ''
Write-Host '=== 2. System: global WPF registry (all WPF apps) ===' -ForegroundColor Cyan
Write-Host 'Key: HKCU\Software\Microsoft\Avalon.Graphics\DisableHWAcceleration (DWORD 1 = force software for every WPF app)'
try {
    $av = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Avalon.Graphics' -Name 'DisableHWAcceleration' -ErrorAction Stop
    $v = $av.DisableHWAcceleration
    Write-Host "DisableHWAcceleration = $v"
    if ($v -eq 1) {
        Write-Host '  -> Global WPF hardware acceleration is OFF for all WPF programs.' -ForegroundColor Green
    } else {
        Write-Host '  -> Global key present but not 1; or not forcing software globally.'
    }
} catch {
    Write-Host 'Key not set or key path missing (normal if you never set global WPF disable).'
}

# --- 3. Beanfun processes ---
Write-Host ''
Write-Host '=== 3. Running Beanfun.exe ===' -ForegroundColor Cyan
$procs = @(Get-Process -Name 'Beanfun' -ErrorAction SilentlyContinue)
if ($procs.Count -eq 0) {
    Write-Host 'No Beanfun.exe. Start Beanfun to check WebView2 children and GPU counters.'
    $bfPids = @()
} else {
    $bfPids = @($procs | ForEach-Object { $_.Id })
    foreach ($p in $procs) {
        Write-Host ('PID {0}, start {1}' -f $p.Id, $p.StartTime)
    }
}

# --- 4. WebView2 children (system-observable Chromium flags) ---
if (-not $Quick -and $bfPids.Count -gt 0) {
    Write-Host ''
    Write-Host '=== 4. System: child processes of Beanfun (WebView2 / Edge WebView) ===' -ForegroundColor Cyan
    try {
        $all = Get-CimInstance -ClassName Win32_Process -ErrorAction Stop |
            Where-Object { $bfPids -contains [uint32]$_.ParentProcessId }
        $webview = $all | Where-Object {
            $_.Name -match 'msedgewebview2|WebView2' -or
            ($_.ExecutablePath -and $_.ExecutablePath -match 'WebView2|msedge')
        }
        if (-not $webview) {
            Write-Host 'No WebView2 child processes found (normal if you have not opened an embedded browser window).'
        } else {
            foreach ($w in $webview) {
                $cl = $w.CommandLine
                $hasGpu = $cl -and ($cl -match '(^|\s)--disable-gpu(\s|$)')
                Write-Host "  PID $($w.ProcessId) Name=$($w.Name)"
                if ($hasGpu) {
                    Write-Host '    CommandLine contains --disable-gpu -> WebView2 GPU compositing likely disabled for this runtime.' -ForegroundColor Green
                } else {
                    Write-Host '    CommandLine has no --disable-gpu (default WebView2 may use GPU).' -ForegroundColor Yellow
                }
            }
        }
    } catch {
        Write-Host "WMI error: $_" -ForegroundColor Red
    }
}

# --- 5. GPU Engine counters (scheduler / Task Manager style) ---
if (-not $Quick -and -not $NoGpu -and $bfPids.Count -gt 0) {
    Write-Host ''
    Write-Host '=== 5. System: GPU Engine utilization (pid in instance name) ===' -ForegroundColor Cyan
    Write-Host 'Ref: https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/'
    try {
        $samples = (Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop).CounterSamples
        foreach ($bfProcId in $bfPids) {
            $pidStr = "pid_$bfProcId"
            $related = $samples | Where-Object { $_.InstanceName -like "*$pidStr*" }
            if (-not $related) {
                Write-Host "PID $bfProcId : no GPU Engine samples (often means little/no GPU use for this PID right now)."
            } else {
                $max = ($related | Measure-Object -Property CookedValue -Maximum).Maximum
                Write-Host "PID $bfProcId : GPU Engine samples (max utilization $([math]::Round($max,1)) %):"
                $related | Where-Object { $_.CookedValue -gt 0.5 } | ForEach-Object {
                    Write-Host ('  {0} = {1:N1} %' -f $_.InstanceName, $_.CookedValue)
                }
                if ($max -lt 1) {
                    Write-Host '  -> Mostly idle on GPU for this PID at sample time (consistent with reduced GPU use).' -ForegroundColor DarkGray
                }
            }
        }
    } catch {
        Write-Host "GPU counters unavailable: $_"
        Write-Host 'Try Task Manager -> Processes -> GPU column while Beanfun is open.'
    }
}

Write-Host ''
Write-Host '=== Summary ===' -ForegroundColor Cyan
Write-Host "Config disableHardwareAcceleration (intent): $disableRequested"
if ($bfPids.Count -eq 0) {
    Write-Host 'Run Beanfun and (if testing WebView2) open a page that uses embedded browser, then re-run for sections 4-5.'
}
Write-Host ''
exit 0
