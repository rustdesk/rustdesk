@echo off
:: 适用于 Windows Server 2012 的 RustDesk 密钥修复脚本
:: 请以管理员身份运行

echo ============================================================
echo RustDesk 密钥更新工具 - Windows Server 2012 兼容版
echo ============================================================
echo.

:: 检查管理员权限
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [错误] 需要管理员权限！
    echo 请右键点击此文件，选择"以管理员身份运行"
    echo.
    pause
    exit /b 1
)

echo [OK] 管理员权限验证通过
echo.

set "NEW_KEY=AAAAC3NzaC1lZDI1NTE5AAAAIBAjWWVdpMda/rF5zAObc92HsyO2xWyNNaUtQByf0RYI"

echo 新密钥: %NEW_KEY%
echo.
echo ============================================================
echo 正在更新配置文件...
echo ============================================================
echo.

:: 停止 RustDesk 服务（如果作为服务运行）
echo 正在停止 RustDesk 进程...
taskkill /F /IM rustdesk.exe >nul 2>&1
timeout /t 2 >nul

:: 更新 ProgramData 配置
set "FILE1=C:\ProgramData\RustDesk\config\RustDesk.toml"
set "FILE2=C:\ProgramData\RustDesk\config\RustDesk2.toml"

if exist "%FILE1%" (
    echo [1/4] 正在更新: %FILE1%
    powershell -Command "(Get-Content '%FILE1%' -Raw) -replace 'key\s*=\s*\"[^\"]*\"', 'key = \"%NEW_KEY%\"' | Set-Content '%FILE1%' -NoNewline"
    if !errorLevel! equ 0 (
        echo [OK] 更新成功
    ) else (
        echo [ERROR] 更新失败
    )
) else (
    echo [SKIP] 文件不存在: %FILE1%
)
echo.

if exist "%FILE2%" (
    echo [2/4] 正在更新: %FILE2%
    powershell -Command "(Get-Content '%FILE2%' -Raw) -replace 'key\s*=\s*\"[^\"]*\"', 'key = \"%NEW_KEY%\"' | Set-Content '%FILE2%' -NoNewline"
    if !errorLevel! equ 0 (
        echo [OK] 更新成功
    ) else (
        echo [ERROR] 更新失败
    )
) else (
    echo [SKIP] 文件不存在: %FILE2%
)
echo.

:: 更新用户配置
set "USER_CONFIG=%APPDATA%\RustDesk\config\RustDesk2.toml"
if exist "%USER_CONFIG%" (
    echo [3/4] 正在更新: %USER_CONFIG%
    powershell -Command "(Get-Content '%USER_CONFIG%' -Raw) -replace 'key\s*=\s*[''\""][^''\"]*[''\""]', 'key = ''%NEW_KEY%''' | Set-Content '%USER_CONFIG%' -NoNewline"
    if !errorLevel! equ 0 (
        echo [OK] 更新成功
    ) else (
        echo [ERROR] 更新失败
    )
) else (
    echo [SKIP] 文件不存在: %USER_CONFIG%
)
echo.

:: 重置用户配置中的密钥确认状态
set "USER_CONFIG_MAIN=%APPDATA%\RustDesk\config\RustDesk.toml"
if exist "%USER_CONFIG_MAIN%" (
    echo [4/4] 正在重置密钥确认状态...
    powershell -Command "(Get-Content '%USER_CONFIG_MAIN%' -Raw) -replace 'key_confirmed\s*=\s*true', 'key_confirmed = false' | Set-Content '%USER_CONFIG_MAIN%' -NoNewline"
    if !errorLevel! equ 0 (
        echo [OK] 重置成功
    ) else (
        echo [ERROR] 重置失败
    )
) else (
    echo [SKIP] 文件不存在: %USER_CONFIG_MAIN%
)

echo.
echo ============================================================
echo 更新完成！
echo ============================================================
echo.
echo 请执行以下步骤:
echo 1. 重新启动 RustDesk 应用程序
echo 2. 检查网络设置中的服务器配置
echo 3. 尝试连接到另一台电脑
echo.
echo ============================================================
echo.
pause
