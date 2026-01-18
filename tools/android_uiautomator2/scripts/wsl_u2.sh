#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-}"
shift || true

if [[ -z "${cmd}" || "${cmd}" == "-h" || "${cmd}" == "--help" ]]; then
  cat <<'USAGE'
用法：
  tools/android_uiautomator2/scripts/wsl_u2.sh <command> [args...]

说明：
  - Emulator 与 UIAutomator2 均运行在 Windows 侧
  - 本脚本仅在 WSL 侧触发：调用 Windows PowerShell 执行 Windows 侧的 uv/uiautomator2

示例：
  tools/android_uiautomator2/scripts/wsl_u2.sh sync
  tools/android_uiautomator2/scripts/wsl_u2.sh u2-connect --serial emulator-5554
  tools/android_uiautomator2/scripts/wsl_u2.sh u2-smoketest --serial emulator-5554 --screenshot .\\u2.png
USAGE
  exit 2
fi

WIN_PS="${WIN_PS:-/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe}"
if [[ ! -x "${WIN_PS}" ]]; then
  echo "找不到 Windows PowerShell：${WIN_PS}" >&2
  exit 3
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
U2_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd -P)"
U2_DIR_WIN="$(wslpath -w "${U2_DIR}")"

# 关键：在 Windows 侧执行 uv，并显式设置 ADBUTILS_ADB_PATH/ADB 指向 Windows 的 adb.exe
exec "${WIN_PS}" -NoProfile -Command "& {
  \$Sdk=\"\$env:LOCALAPPDATA\\Android\\Sdk\"
  \$adb=\"\$Sdk\\platform-tools\\adb.exe\"
  \$env:ADBUTILS_ADB_PATH=\$adb
  \$env:ADB=\$adb
  # 避免与 WSL/Linux 侧的 .venv/ 混用导致 Windows 删除/重建失败
  \$env:UV_PROJECT_ENVIRONMENT='.venv-win'
  cd \"${U2_DIR_WIN}\"

  \$cmd=\$args[0]
  \$rest=@()
  if (\$args.Length -gt 1) { \$rest=\$args[1..(\$args.Length-1)] }

  if (\$cmd -eq 'sync') {
    uv sync
  } else {
    # tool.uv.package=false 时不会安装 project.scripts entry points；
    # 这里将常用命令映射为直接执行脚本，保证在 Windows 侧可用。
    if (\$cmd -eq 'u2-connect') {
      uv run python src\\u2runner\\connect.py @rest
    } elseif (\$cmd -eq 'u2-smoketest') {
      uv run python src\\u2runner\\smoketest.py @rest
    } elseif (\$cmd -eq 'u2-story10-disable-update') {
      uv run python src\\u2runner\\story10_disable_update.py @rest
    } else {
      uv run \$cmd @rest
    }
  }
}" "${cmd}" "$@"
