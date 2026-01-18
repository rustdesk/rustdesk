param(
  [Parameter(Mandatory = $false)]
  [string]$AvdName = "Small_Phone",

  [Parameter(Mandatory = $false)]
  [string]$SdkRoot = "$Env:LOCALAPPDATA\Android\Sdk",

  [Parameter(Mandatory = $false)]
  [string[]]$ExtraArgs = @("-no-snapshot-save")
)

$ErrorActionPreference = "Stop"

function Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Ok($msg) { Write-Host "[ OK ] $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "[FAIL] $msg" -ForegroundColor Red }

$emu = Join-Path $SdkRoot "emulator\emulator.exe"
if (!(Test-Path $emu)) {
  Fail "找不到 emulator.exe：$emu"
  Fail "请确认已安装 Android SDK（含 emulator），或传入正确的 -SdkRoot"
  exit 2
}

Info "使用 emulator：$emu"
Info "启动 AVD：$AvdName"

$avds = & $emu -list-avds
if (($avds | Where-Object { $_.Trim() -eq $AvdName }).Count -eq 0) {
  Fail "未找到该 AVD：$AvdName"
  Write-Host "已存在的 AVD：" -ForegroundColor Yellow
  $avds | ForEach-Object { Write-Host "  $_" }
  exit 3
}

Ok "开始启动模拟器（本窗口会保持运行；关掉窗口/进程即退出模拟器）"
& $emu -avd $AvdName @ExtraArgs

