#!/usr/bin/env bash
# Beanfun 硬體加速：從「系統可觀測」項目檢查（非僅設定檔）。
# WPF 的每程序 SoftwareOnly 無法被其他行程讀取；此腳本在 Windows 上委派 PowerShell 做完整檢查。
#
# 仍會做的「純 shell」項目（不依賴 .ps1 內容）：全域 WPF 登錄（影響所有 WPF 程式）。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PS1="$SCRIPT_DIR/check-beanfun-hw-accel.ps1"

echo "=== Global WPF registry (HKCU, all WPF apps) ==="
if command -v reg.exe >/dev/null 2>&1; then
  reg.exe query "HKCU\Software\Microsoft\Avalon.Graphics" /v DisableHWAcceleration 2>/dev/null || echo "(Key or value not set — normal if you never set global WPF disable)"
else
  echo "reg.exe not found"
fi

echo ""
if command -v powershell.exe >/dev/null 2>&1 && [[ -f "$PS1" ]]; then
  echo "=== Delegating to PowerShell (WebView2 cmdline, GPU Engine counters, Config.xml) ==="
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PS1" "$@"
  exit $?
fi

echo "PowerShell or $PS1 missing — cannot check WebView2 children or GPU counters."
echo "Install/use Windows and run: powershell.exe -File \"$PS1\""
exit 1
