param(
  [Parameter(Mandatory = $false)]
  [string]$Serial = "emulator-5554",

  [Parameter(Mandatory = $false)]
  [string]$SdkRoot = "$Env:LOCALAPPDATA\Android\Sdk",

  [Parameter(Mandatory = $false)]
  [string]$ProjectPath = (Get-Location).Path,

  [Parameter(Mandatory = $false)]
  [int]$DeviceTimeoutSec = 120,

  [switch]$ResetAdb = $false
)

$ErrorActionPreference = "Stop"

function Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Ok($msg) { Write-Host "[ OK ] $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "[FAIL] $msg" -ForegroundColor Red }

$adb = Join-Path $SdkRoot "platform-tools\adb.exe"
if (!(Test-Path $adb)) {
  Fail "找不到 adb.exe：$adb"
  Fail "请确认已安装 Android SDK Platform-Tools，或传入正确的 -SdkRoot"
  exit 2
}

Info "使用 adb：$adb"
Info "目标设备：$Serial"

if ($ResetAdb) {
  Warn "你启用了 -ResetAdb：这会重启 adb server，期间模拟器可能短暂 offline。"
  & $adb kill-server | Out-Null
  Get-Process adb -ErrorAction SilentlyContinue | Stop-Process -Force
}

& $adb start-server | Out-Null

Info "等待设备上线（最多 ${DeviceTimeoutSec}s）…"
$deadline = (Get-Date).AddSeconds($DeviceTimeoutSec)
while ($true) {
  $state = (& $adb -s $Serial get-state 2>$null | ForEach-Object { $_.Trim() }) -join ""
  if ($state -eq "device") { break }
  if ((Get-Date) -gt $deadline) {
    Fail "设备未就绪：state=$state"
    & $adb devices -l
    exit 3
  }
  Start-Sleep -Milliseconds 500
}
Ok "设备已就绪：$Serial"

Info "执行 uiautomator2 init（用 uv 运行；请在含 uiautomator2 的工程目录执行/或传 -ProjectPath）…"
if (!(Test-Path $ProjectPath)) {
  Fail "ProjectPath 不存在：$ProjectPath"
  exit 4
}

Push-Location $ProjectPath
try {
  & uv run python -m uiautomator2 init -s $Serial
  Ok "u2 init 完成"

  Info "冒烟验证（info + app_current + home）…"
  $py = "import uiautomator2 as u2; d=u2.connect('$Serial'); print(d.info); print(d.app_current()); d.press('home'); print('ok')"
  & uv run python -c $py
  Ok "u2 可用"
}
finally {
  Pop-Location
}

