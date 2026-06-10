@echo off
REM ===================================================
REM 一键安装Cislink版RustDesk
REM 此脚本会自动请求管理员权限并运行安装包
REM ===================================================

echo.
echo ========================================
echo   Cislink RustDesk 一键安装
echo ========================================
echo.

REM 检查管理员权限
net session >nul 2>&1
if %errorLevel% == 0 (
    echo [OK] 已获得管理员权限
    goto :install
) else (
    echo [提示] 正在请求管理员权限...
    echo.
    goto :elevate
)

:elevate
REM 自动请求管理员权限
powershell -Command "Start-Process '%~f0' -Verb RunAs"
exit /b

:install
echo.
echo [1/3] 检查安装包...

REM 查找安装包（支持同目录或Output子目录）
set INSTALLER=
if exist "%~dp0RustDesk_Cislink_Installer_v2.1.exe" (
    set INSTALLER=%~dp0RustDesk_Cislink_Installer_v2.1.exe
) else if exist "%~dp0Output\RustDesk_Cislink_Installer_v2.1.exe" (
    set INSTALLER=%~dp0Output\RustDesk_Cislink_Installer_v2.1.exe
) else if exist "%~dp0*.exe" (
    REM 尝试查找任何RustDesk安装包
    for %%f in ("%~dp0RustDesk*.exe") do set INSTALLER=%%f
)

if "%INSTALLER%"=="" (
    echo [错误] 未找到安装包
    echo.
    echo 请确保以下文件之一存在：
    echo   - RustDesk_Cislink_Installer_v2.1.exe
    echo   - Output\RustDesk_Cislink_Installer_v2.1.exe
    echo.
    pause
    exit /b 1
)

echo [OK] 找到安装包: %INSTALLER%
echo.

echo [2/3] 停止运行中的RustDesk...
taskkill /F /IM rustdesk*.exe /T >nul 2>&1
if %errorLevel% == 0 (
    echo [OK] 已停止RustDesk进程
) else (
    echo [提示] RustDesk未运行
)
timeout /t 2 /nobreak >nul
echo.

echo [3/3] 正在启动安装程序...
echo.
echo ========================================
echo   请按照安装向导完成安装
echo ========================================
echo.

REM 运行安装包（静默安装选项：添加 /VERYSILENT /NORESTART）
start "" "%INSTALLER%" /SILENT

echo.
echo [完成] 安装程序已启动
echo.
echo 安装完成后，RustDesk将自动配置为连接到Cislink服务器
echo 服务器地址: hbbs.cislink.nl
echo.
echo 按任意键关闭此窗口...
pause >nul
exit /b 0
