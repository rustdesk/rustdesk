# RustDesk Migration Script - Switch to Cislink Server
# This script migrates an existing public RustDesk installation to Cislink custom server
#
# Requirements: Administrator privileges
# Usage: Right-click -> Run with PowerShell (as Administrator)

#Requires -RunAsAdministrator

param(
    [switch]$RenameExe = $false,  # Use file rename method (recommended but requires shortcut updates)
    [switch]$ConfigOnly = $false,  # Only modify config files (simpler, lower priority)
    [switch]$Auto = $false         # Automatically choose best method
)

# ==================== Configuration ====================
$SERVER_CONFIG = @{
    Host = "hbbs.cislink.nl"
    Relay = "hbbr.cislink.nl"
    Key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="
    Api = ""
}

$NEW_EXE_NAME = "rustdesk-host=$($SERVER_CONFIG.Host),key=$($SERVER_CONFIG.Key),relay=$($SERVER_CONFIG.Relay),.exe"

# ==================== Helper Functions ====================

function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

function Write-Header {
    param([string]$Title)
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host $Title -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
}

function Stop-RustDeskProcesses {
    Write-ColorOutput "Stopping RustDesk processes..." "Yellow"

    $processes = Get-Process | Where-Object { $_.Name -like "rustdesk*" }

    if ($processes) {
        $processes | ForEach-Object {
            Write-ColorOutput "  Stopping: $($_.Name) (PID: $($_.Id))" "Gray"
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
        Start-Sleep -Seconds 2
        Write-ColorOutput "[OK] All RustDesk processes stopped" "Green"
    } else {
        Write-ColorOutput "[OK] No RustDesk processes running" "Green"
    }
}

function Find-RustDeskInstallation {
    Write-ColorOutput "Searching for RustDesk installation..." "Yellow"

    # Common installation paths
    $possiblePaths = @(
        "$env:ProgramFiles\RustDesk",
        "${env:ProgramFiles(x86)}\RustDesk",
        "$env:LocalAppData\RustDesk",
        "$env:APPDATA\RustDesk"
    )

    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            $exeFiles = Get-ChildItem -Path $path -Filter "rustdesk*.exe" -ErrorAction SilentlyContinue
            if ($exeFiles) {
                Write-ColorOutput "[OK] Found RustDesk at: $path" "Green"
                return @{
                    Path = $path
                    Exe = $exeFiles[0]
                }
            }
        }
    }

    Write-ColorOutput "[ERROR] RustDesk installation not found!" "Red"
    return $null
}

function Write-ConfigFiles {
    Write-ColorOutput "Writing configuration files..." "Yellow"

    $configContent = @"
[options]
custom-rendezvous-server = "$($SERVER_CONFIG.Host)"
relay-server = "$($SERVER_CONFIG.Relay)"
key = "$($SERVER_CONFIG.Key)"
disable-update-check = true
disable-installation = true
"@

    $configPaths = @(
        "$env:APPDATA\RustDesk\config",
        "$env:PROGRAMDATA\RustDesk\config"
    )

    foreach ($configPath in $configPaths) {
        if (-not (Test-Path $configPath)) {
            New-Item -ItemType Directory -Path $configPath -Force | Out-Null
        }

        $configFile1 = Join-Path $configPath "RustDesk.toml"
        $configFile2 = Join-Path $configPath "RustDesk2.toml"

        $configContent | Out-File -FilePath $configFile1 -Encoding utf8 -NoNewline
        $configContent | Out-File -FilePath $configFile2 -Encoding utf8 -NoNewline

        Write-ColorOutput "  [OK] $configFile1" "Green"
        Write-ColorOutput "  [OK] $configFile2" "Green"
    }
}

function Write-RegistryConfig {
    Write-ColorOutput "Writing registry configuration..." "Yellow"

    # Find the correct registry key
    $regPaths = @(
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk",
        "HKLM:\Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk"
    )

    $regPathFound = $false
    foreach ($regPath in $regPaths) {
        if (Test-Path $regPath) {
            try {
                Set-ItemProperty -Path $regPath -Name "Host" -Value $SERVER_CONFIG.Host -ErrorAction Stop
                Set-ItemProperty -Path $regPath -Name "Key" -Value $SERVER_CONFIG.Key -ErrorAction Stop

                if ($SERVER_CONFIG.Relay) {
                    # Note: Registry method might not support Relay, but we try anyway
                    Set-ItemProperty -Path $regPath -Name "Relay" -Value $SERVER_CONFIG.Relay -ErrorAction SilentlyContinue
                }

                Write-ColorOutput "  [OK] Registry updated: $regPath" "Green"
                $regPathFound = $true
                break
            } catch {
                Write-ColorOutput "  [WARNING] Could not write to: $regPath" "Yellow"
            }
        }
    }

    if (-not $regPathFound) {
        Write-ColorOutput "  [INFO] Registry path not found (RustDesk may not be installed via installer)" "Gray"
    }
}

function Rename-RustDeskExe {
    param(
        [string]$InstallPath,
        [object]$CurrentExe
    )

    Write-ColorOutput "Renaming executable to embed server configuration..." "Yellow"

    $currentPath = $CurrentExe.FullName
    $newPath = Join-Path $InstallPath $NEW_EXE_NAME

    if ($currentPath -eq $newPath) {
        Write-ColorOutput "[INFO] Executable already has correct name" "Green"
        return $true
    }

    try {
        # Backup original
        $backupPath = $currentPath + ".backup"
        if (-not (Test-Path $backupPath)) {
            Copy-Item -Path $currentPath -Destination $backupPath -Force
            Write-ColorOutput "  [OK] Backup created: $backupPath" "Green"
        }

        # Rename
        Move-Item -Path $currentPath -Destination $newPath -Force
        Write-ColorOutput "  [OK] Renamed to: $NEW_EXE_NAME" "Green"

        return $true
    } catch {
        Write-ColorOutput "  [ERROR] Failed to rename: $_" "Red"
        return $false
    }
}

function Update-Shortcuts {
    param(
        [string]$InstallPath,
        [string]$NewExePath
    )

    Write-ColorOutput "Updating shortcuts..." "Yellow"

    $shortcutLocations = @(
        "$env:Public\Desktop",
        "$env:USERPROFILE\Desktop",
        "$env:APPDATA\Microsoft\Windows\Start Menu\Programs",
        "$env:ProgramData\Microsoft\Windows\Start Menu\Programs"
    )

    $shell = New-Object -ComObject WScript.Shell
    $updated = 0

    foreach ($location in $shortcutLocations) {
        if (Test-Path $location) {
            $shortcuts = Get-ChildItem -Path $location -Filter "*RustDesk*.lnk" -Recurse -ErrorAction SilentlyContinue

            foreach ($shortcut in $shortcuts) {
                try {
                    $lnk = $shell.CreateShortcut($shortcut.FullName)
                    if ($lnk.TargetPath -like "*rustdesk*.exe") {
                        $lnk.TargetPath = $NewExePath
                        $lnk.Save()
                        Write-ColorOutput "  [OK] Updated: $($shortcut.FullName)" "Green"
                        $updated++
                    }
                } catch {
                    Write-ColorOutput "  [WARNING] Could not update: $($shortcut.FullName)" "Yellow"
                }
            }
        }
    }

    Write-ColorOutput "  [INFO] Updated $updated shortcut(s)" "Gray"
}

function Test-Configuration {
    param([string]$InstallPath)

    Write-Header "Verification"

    # Check executable name
    $exeFiles = Get-ChildItem -Path $InstallPath -Filter "rustdesk*.exe" -ErrorAction SilentlyContinue
    if ($exeFiles) {
        Write-ColorOutput "Executable:" "Yellow"
        foreach ($exe in $exeFiles) {
            $isConfigured = $exe.Name -match "host="
            $status = if ($isConfigured) { "[CONFIGURED]" } else { "[STANDARD]" }
            $color = if ($isConfigured) { "Green" } else { "Yellow" }
            Write-ColorOutput "  $status $($exe.Name)" $color
        }
    }

    # Check config files
    Write-ColorOutput "`nConfiguration files:" "Yellow"
    $configPaths = @(
        "$env:APPDATA\RustDesk\config\RustDesk.toml",
        "$env:APPDATA\RustDesk\config\RustDesk2.toml"
    )

    foreach ($configPath in $configPaths) {
        if (Test-Path $configPath) {
            $content = Get-Content $configPath -Raw
            if ($content -match $SERVER_CONFIG.Host) {
                Write-ColorOutput "  [OK] $configPath" "Green"
            } else {
                Write-ColorOutput "  [!] $configPath (not configured)" "Yellow"
            }
        } else {
            Write-ColorOutput "  [X] $configPath (not found)" "Red"
        }
    }
}

# ==================== Main Script ====================

Write-Header "RustDesk to Cislink Server Migration"

Write-ColorOutput "Target Server Configuration:" "Yellow"
Write-ColorOutput "  ID Server:    $($SERVER_CONFIG.Host)" "White"
Write-ColorOutput "  Relay Server: $($SERVER_CONFIG.Relay)" "White"
Write-ColorOutput "  Public Key:   $($SERVER_CONFIG.Key)" "White"
Write-Host ""

# Check if running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-ColorOutput "[ERROR] This script requires Administrator privileges!" "Red"
    Write-ColorOutput "Please right-click and select 'Run as Administrator'" "Yellow"
    pause
    exit 1
}

# Find RustDesk installation
$installation = Find-RustDeskInstallation
if (-not $installation) {
    Write-ColorOutput "`n[ERROR] Cannot proceed without RustDesk installation" "Red"
    pause
    exit 1
}

$installPath = $installation.Path
$currentExe = $installation.Exe

Write-ColorOutput "Current executable: $($currentExe.Name)" "Gray"

# Determine migration method
if (-not $RenameExe -and -not $ConfigOnly -and -not $Auto) {
    Write-Host "`nChoose migration method:" -ForegroundColor Yellow
    Write-Host "1. Rename EXE method (Recommended - Highest priority, requires shortcut updates)"
    Write-Host "2. Config files only (Simpler - No shortcut updates needed, lower priority)"
    Write-Host "3. Auto (Tries rename, falls back to config-only)"
    Write-Host ""

    $choice = Read-Host "Enter choice (1-3)"

    switch ($choice) {
        "1" { $RenameExe = $true }
        "2" { $ConfigOnly = $true }
        "3" { $Auto = $true }
        default {
            Write-ColorOutput "Invalid choice, using Auto mode" "Yellow"
            $Auto = $true
        }
    }
}

# Stop RustDesk
Stop-RustDeskProcesses

# Execute migration based on selected method
$success = $false

if ($RenameExe -or $Auto) {
    Write-Header "Migration Method: Rename EXE + Config Files"

    # Try to rename EXE
    $renameSuccess = Rename-RustDeskExe -InstallPath $installPath -CurrentExe $currentExe

    if ($renameSuccess) {
        $newExePath = Join-Path $installPath $NEW_EXE_NAME
        Update-Shortcuts -InstallPath $installPath -NewExePath $newExePath
        $success = $true
    } elseif ($Auto) {
        Write-ColorOutput "`n[INFO] Rename failed, falling back to config-only method..." "Yellow"
        $ConfigOnly = $true
    } else {
        Write-ColorOutput "`n[ERROR] Rename method failed. Try config-only method instead." "Red"
    }
}

if ($ConfigOnly -or (-not $success -and $Auto)) {
    Write-Header "Migration Method: Config Files + Registry"
    Write-ColorOutput "[INFO] Using config file method (exe rename not performed)" "Yellow"
    $success = $true
}

# Always write config files and registry (as backup/fallback)
Write-ConfigFiles
Write-RegistryConfig

# Verification
Test-Configuration -InstallPath $installPath

# Summary
Write-Header "Migration Complete"

if ($success) {
    Write-ColorOutput "[SUCCESS] RustDesk has been configured for Cislink server!" "Green"
    Write-Host ""
    Write-ColorOutput "Next steps:" "Yellow"
    Write-ColorOutput "1. Start RustDesk" "White"
    Write-ColorOutput "2. Go to Settings -> Network" "White"
    Write-ColorOutput "3. Verify server shows: $($SERVER_CONFIG.Host)" "White"
    Write-Host ""
    Write-ColorOutput "Press any key to start RustDesk now, or close this window to start manually..." "Yellow"
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")

    # Start RustDesk
    $exePath = Get-ChildItem -Path $installPath -Filter "rustdesk*.exe" | Select-Object -First 1
    if ($exePath) {
        Start-Process -FilePath $exePath.FullName
        Write-ColorOutput "[OK] RustDesk started" "Green"
    }
} else {
    Write-ColorOutput "[ERROR] Migration failed. Please contact support." "Red"
    pause
    exit 1
}
