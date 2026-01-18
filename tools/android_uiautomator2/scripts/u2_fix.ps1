param(
  [Parameter(Mandatory = $false)]
  [string]$Serial = "emulator-5554",

  [Parameter(Mandatory = $false)]
  [string]$SdkRoot = "$Env:LOCALAPPDATA\Android\Sdk",

  [Parameter(Mandatory = $false)]
  [int]$AdbPort = 5037,

  [Parameter(Mandatory = $false)]
  [string]$U2ProjectPath = "",

  [switch]$ReinstallU2 = $true
)

$ErrorActionPreference = "Stop"

function Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Ok($msg) { Write-Host "[ OK ] $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "[FAIL] $msg" -ForegroundColor Red }

$adb = Join-Path $SdkRoot "platform-tools\adb.exe"
if (!(Test-Path $adb)) {
  Fail "找不到 adb.exe：$adb"
  Fail "请确认已安装 Android SDK Platform-Tools，并传入正确的 -SdkRoot"
  exit 2
}

Info "使用 adb：$adb"
Info "目标设备：$Serial"

Info "1) 重启 adb server（为 WSL 放开监听：-a）…"
& $adb kill-server | Out-Null
& $adb -a -P $AdbPort start-server | Out-Null
& $adb devices -l

Info "2) 清理端口转发/残留进程…"
try { & $adb forward --remove-all | Out-Null } catch { }
try { & $adb -s $Serial shell "pkill -f com.wetest.uia2.Main 2>/dev/null || true" | Out-Null } catch { }
try { & $adb -s $Serial shell "pkill -f atx-agent 2>/dev/null || true" | Out-Null } catch { }

Info "3) 清理设备侧残留文件…"
& $adb -s $Serial shell "rm -rf /data/local/tmp/u2 /data/local/tmp/u2.jar /data/local/tmp/atx-agent /data/local/tmp/atx-agent.* 2>/dev/null || true" | Out-Null

if ($ReinstallU2) {
  Info "4) 卸载旧的 uiautomator2 相关包（忽略失败）…"
  try { & $adb -s $Serial uninstall com.github.uiautomator | Out-Null } catch { }
  try { & $adb -s $Serial uninstall com.github.uiautomator.test | Out-Null } catch { }
  try { & $adb -s $Serial uninstall com.github.uiautomator.test.runner | Out-Null } catch { }
}

if ($U2ProjectPath -and (Test-Path $U2ProjectPath)) {
  Info "5) 在指定工程目录执行 u2 init：$U2ProjectPath"
  Push-Location $U2ProjectPath
  try {
    & uv run python -m uiautomator2 init -s $Serial
    Ok "u2 init 完成"
  }
  finally {
    Pop-Location
  }
} else {
  Warn "未指定 -U2ProjectPath（或路径不存在），跳过自动 init。你可以手动执行："
  Write-Host "  uv run python -m uiautomator2 init -s $Serial"
}

Info "6) 快速验证（需要 uiautomator2 已安装在当前 uv venv/环境中）"
Write-Host "  uv run python -c `"import uiautomator2 as u2; d=u2.connect('$Serial'); print(d.info); print(d.app_current()); d.press('home'); print('ok')`""

Ok "完成。若你在 WSL 里跑 u2，请确保 Windows 防火墙放行 $AdbPort/TCP，且 WSL 设置 ADB_SERVER_SOCKET=tcp:<WindowsHostIP>:$AdbPort"

