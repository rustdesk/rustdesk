<#
.SYNOPSIS
    Installs the build dependencies and builds the RustDesk Flutter client for Windows x64.

.DESCRIPTION
    Mirrors the Windows x64 job in .github/workflows/flutter-build.yml:
      * Git, Python 3, Visual Studio 2022 Build Tools (C++), via winget
      * LLVM/Clang (for bindgen)
      * Rust toolchain via rustup
      * vcpkg at the pinned commit, plus the C/C++ deps from vcpkg.json
      * Flutter SDK with RustDesk's custom Windows engine and Flutter patch
    Then runs build.py and assembles the runnable folder in -OutputDir.

    System-wide or per-user tools (Git, Python 3.12+, VS Build Tools, LLVM, rustup)
    are detected the usual way: PATH, registry, vswhere, standard install folders.
    Project-specific tools (vcpkg, Flutter with RustDesk's patches, downloads) live
    only in -ToolsDir, unless -VcpkgRoot / -FlutterRoot point at existing copies.
    Every step is idempotent, so re-running the script after a failure or for
    a rebuild only redoes what is missing.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\build_windows.ps1

.EXAMPLE
    .\build_windows.ps1 -ToolsDir D:\rd-tools -Installer
#>
[CmdletBinding()]
param(
    # Where LLVM, vcpkg and Flutter are installed. Keep the path short: vcpkg's ffmpeg build hits MAX_PATH otherwise.
    [string]$ToolsDir = "C:\rustdesk-deps",
    # Reuse an existing vcpkg checkout instead of <ToolsDir>\vcpkg. It must already be at the pinned commit.
    [string]$VcpkgRoot,
    # Reuse an existing Flutter SDK instead of <ToolsDir>\flutter. It must be the pinned version;
    # RustDesk's engine and patch are applied to it, which affects every project built with it.
    [string]$FlutterRoot,
    # Where the runnable RustDesk folder is assembled. Defaults to <repo>\target\rustdesk-windows.
    [string]$OutputDir,
    # Regenerate the flutter_rust_bridge files even if they already exist.
    [switch]$GenerateBridge,
    # Also pack a single-file rustdesk-<version>-install.exe (libs/portable).
    [switch]$Installer,
    # Skip downloading the virtual display and printer drivers into the output folder.
    [switch]$SkipExtras
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# Keep in sync with .github/workflows/flutter-build.yml and bridge.yml.
$RustVersion = "1.75"
$LlvmVersion = "15.0.6"
$FlutterVersion = "3.24.5"
$VcpkgCommit = "9e593bb18ea69cc5095e012465dcd675a822ed0d"
$FrbVersion = "1.80.1"
$CargoExpandVersion = "1.0.95"
$RustTarget = "x86_64-pc-windows-msvc"
$Triplet = "x64-windows-static"

# $PSScriptRoot is empty inside param() defaults on Windows PowerShell 5.1.
$RepoRoot = $PSScriptRoot
if (-not $OutputDir) { $OutputDir = Join-Path $RepoRoot "target\rustdesk-windows" }
$DownloadDir = Join-Path $ToolsDir "downloads"
$LlvmDir = Join-Path $ToolsDir "LLVM"
$UseOwnVcpkg = -not $VcpkgRoot
if ($UseOwnVcpkg) { $VcpkgRoot = Join-Path $ToolsDir "vcpkg" }
$UseOwnFlutter = -not $FlutterRoot
if ($UseOwnFlutter) { $FlutterRoot = Join-Path $ToolsDir "flutter" }

function Write-Step([string]$Message) {
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# Native tools write progress to stderr; with "Stop" some hosts turn that into a terminating error.
function Invoke-Checked {
    param([string]$Exe, [string[]]$Arguments = @())
    Write-Host "> $Exe $($Arguments -join ' ')" -ForegroundColor DarkGray
    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Exe @Arguments
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $old
    }
    if ($code -ne 0) { throw "'$Exe $($Arguments -join ' ')' failed with exit code $code" }
}

function Test-Native {
    param([string]$Exe, [string[]]$Arguments = @())
    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Exe @Arguments 2>&1 | Out-Null
        return ($LASTEXITCODE -eq 0)
    } finally {
        $ErrorActionPreference = $old
    }
}

function Get-Download([string]$Url, [string]$OutFile) {
    if (Test-Path $OutFile) { return }
    Write-Host "Downloading $Url"
    $tmp = "$OutFile.part"
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $tmp
    Move-Item -Force $tmp $OutFile
}

function Update-SessionPath {
    $env:Path = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
        [Environment]::GetEnvironmentVariable("Path", "User")
}

function Add-SessionPath([string]$Dir) {
    if (($env:Path -split ";") -notcontains $Dir) { $env:Path = "$Dir;$env:Path" }
}

function Test-Admin {
    $principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Returns the path of the first Python >= 3.12 found on PATH or via the py launcher.
# The Microsoft Store "python.exe" stub in WindowsApps is not a real interpreter.
function Get-Python {
    $candidates = @(Get-Command python, python3 -All -ErrorAction SilentlyContinue |
        Where-Object { $_.Source -notlike "*\WindowsApps\*" } |
        ForEach-Object { , @($_.Source) })
    if (Get-Command py -ErrorAction SilentlyContinue) { $candidates += , @("py", "-3") }
    foreach ($cmd in $candidates) {
        $exe = $cmd[0]
        $pre = @($cmd | Select-Object -Skip 1)
        $old = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $out = & $exe @pre -c "import sys; print('%d.%d' % sys.version_info[:2]); print(sys.executable)" 2>$null
        } catch {
            continue
        } finally {
            $ErrorActionPreference = $old
        }
        if ($LASTEXITCODE -ne 0 -or @($out).Count -lt 2) { continue }
        if ([version]$out[0] -ge [version]"3.12") {
            Write-Host "Found Python $($out[0]): $($out[1])"
            return $out[1]
        }
        Write-Host "Ignoring Python $($out[0]) at $($out[1]) (need 3.12+)."
    }
    return $null
}

# Returns the root of an existing LLVM install (the folder holding bin\libclang.dll), or $null.
function Find-Llvm {
    $roots = @()
    if ($env:LIBCLANG_PATH) { $roots += (Split-Path $env:LIBCLANG_PATH) }
    $clang = Get-Command clang -ErrorAction SilentlyContinue
    if ($clang) { $roots += (Split-Path (Split-Path $clang.Source)) }
    foreach ($key in "HKLM:\SOFTWARE\LLVM\LLVM", "HKLM:\SOFTWARE\WOW6432Node\LLVM\LLVM") {
        $regRoot = (Get-ItemProperty $key -ErrorAction SilentlyContinue).'(default)'
        if ($regRoot) { $roots += $regRoot }
    }
    $roots += $LlvmDir, (Join-Path $env:ProgramFiles "LLVM")
    foreach ($root in $roots) {
        if ($root -and (Test-Path (Join-Path $root "bin\libclang.dll"))) { return $root }
    }
    return $null
}

function Get-VsInstall {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $null }
    & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}

# Adds the C++ workload to an existing VS 2019+ instance, or installs VS 2022 Build Tools.
# The VS installer is run directly (not via winget) so its own exit code is reported.
function Install-VsCppTools {
    $vsInstallerDir = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer"
    $vswhere = Join-Path $vsInstallerDir "vswhere.exe"
    $instance = $null
    if (Test-Path $vswhere) {
        # Parenthesized: Windows PowerShell's ConvertFrom-Json emits a JSON array as a single object.
        $instance = ((& $vswhere -all -products * -version 16 -format json) -join "`n" | ConvertFrom-Json) |
            Sort-Object { [version]$_.installationVersion } -Descending | Select-Object -First 1
    }
    if ($instance) {
        $workload = if ($instance.productId -like "*BuildTools") { "Microsoft.VisualStudio.Workload.VCTools" } else { "Microsoft.VisualStudio.Workload.NativeDesktop" }
        Write-Host "Adding the C++ workload to $($instance.displayName) at $($instance.installationPath)"
        $exe = Join-Path $vsInstallerDir "setup.exe"
        $vsArgs = "modify --installPath `"$($instance.installationPath)`" --add $workload --includeRecommended --passive --norestart"
    } else {
        Write-Host "Installing Visual Studio 2022 Build Tools with the C++ workload"
        $exe = Join-Path $DownloadDir "vs_buildtools.exe"
        Get-Download "https://aka.ms/vs/17/release/vs_buildtools.exe" $exe
        $vsArgs = "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    }
    $startArgs = @{ FilePath = $exe; ArgumentList = $vsArgs; Wait = $true; PassThru = $true }
    if (-not (Test-Admin)) { $startArgs.Verb = "RunAs" }
    $proc = Start-Process @startArgs
    switch ($proc.ExitCode) {
        0 { }
        3010 { Write-Warning "Visual Studio needs a reboot to finish; the build may still work, reboot if it fails." }
        1602 { throw "The Visual Studio installer was canceled (1602): UAC prompt declined or timed out, the installer window closed, or the Visual Studio Installer was already open. Close it and re-run, ideally from an elevated PowerShell." }
        1618 { throw "Another installation is in progress (1618). Wait for it to finish (or reboot) and re-run." }
        5007 { throw "This computer does not meet Visual Studio's requirements (5007)." }
        default { throw "The Visual Studio installer failed with exit code $($proc.ExitCode). Logs: $env:TEMP\dd_*.log" }
    }
}

function Install-WingetPackage([string]$Id, [string[]]$Extra = @()) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "winget is not available. Install '$Id' manually (or install App Installer from the Microsoft Store) and re-run."
    }
    $wingetArgs = @("install", "--id", $Id, "-e", "--silent", "--disable-interactivity",
        "--accept-source-agreements", "--accept-package-agreements") + $Extra
    Write-Host "> winget $($wingetArgs -join ' ')" -ForegroundColor DarkGray
    & winget @wingetArgs
    # winget returns non-zero for benign cases (e.g. already installed); callers re-check instead.
    Update-SessionPath
}

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "This script builds the x64 client only (host is $env:PROCESSOR_ARCHITECTURE)."
}

# Installers ask for elevation through a UAC dialog, which times out when nobody is at the machine.
# If any of them is going to run, elevate once now, while the user is still here.
if (-not (Test-Admin)) {
    $devModeOn = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" `
        -Name AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue).AllowDevelopmentWithoutDevLicense -eq 1
    $needsAdmin = -not (Get-Command git -ErrorAction SilentlyContinue) -or
        -not (Get-Python 6>$null) -or
        -not (Get-VsInstall) -or
        -not (Find-Llvm) -or
        -not $devModeOn
    if ($needsAdmin) {
        Write-Host "Some prerequisites need an administrator to install; asking for elevation once now." -ForegroundColor Yellow
        $relaunch = @("-NoExit", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"")
        foreach ($p in $PSBoundParameters.GetEnumerator()) {
            if ($p.Value -is [System.Management.Automation.SwitchParameter]) {
                if ($p.Value.IsPresent) { $relaunch += "-$($p.Key)" }
            } else {
                $relaunch += "-$($p.Key)", "`"$($p.Value)`""
            }
        }
        Start-Process powershell.exe -Verb RunAs -ArgumentList $relaunch
        Write-Host "The build continues in the new administrator window."
        return
    }
}

New-Item -ItemType Directory -Force $ToolsDir, $DownloadDir | Out-Null

# ---------------------------------------------------------------------------
Write-Step "System prerequisites (Git, Python, Visual Studio Build Tools)"

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Install-WingetPackage "Git.Git"
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "Git installation failed." }
}

$Python = Get-Python
if (-not $Python) {
    Install-WingetPackage "Python.Python.3.12"
    $Python = Get-Python
    if (-not $Python) { throw "Python 3.12+ installation failed (open a new terminal if it was just installed)." }
}

if (-not (Get-VsInstall)) {
    Install-VsCppTools
    if (-not (Get-VsInstall)) { throw "Visual Studio C++ build tools are still missing. Install the 'Desktop development with C++' workload. Logs: $env:TEMP\dd_*.log" }
}

# Flutter needs symlinks for plugins, which on Windows requires Developer Mode.
$devKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock"
$devMode = (Get-ItemProperty $devKey -Name AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue).AllowDevelopmentWithoutDevLicense
if ($devMode -ne 1) {
    if (Test-Admin) {
        Write-Host "Enabling Windows Developer Mode (needed by Flutter plugins)."
        if (-not (Test-Path $devKey)) { New-Item $devKey | Out-Null }
        Set-ItemProperty $devKey -Name AllowDevelopmentWithoutDevLicense -Value 1 -Type DWord
    } else {
        throw "Windows Developer Mode is off. Enable it (run 'start ms-settings:developers') or re-run this script as Administrator."
    }
}

# ---------------------------------------------------------------------------
Write-Step "LLVM"

$existingLlvm = Find-Llvm
if ($existingLlvm) {
    $LlvmDir = $existingLlvm
    $clangExe = Join-Path $LlvmDir "bin\clang.exe"
    $clangVersion = if (Test-Path $clangExe) { (& $clangExe --version | Select-Object -First 1) } else { "version unknown" }
    Write-Host "Found LLVM at ${LlvmDir}: $clangVersion"
} else {
    Write-Host "No LLVM found; installing $LlvmVersion into $LlvmDir."
    $llvmInstaller = Join-Path $DownloadDir "LLVM-$LlvmVersion-win64.exe"
    Get-Download "https://github.com/llvm/llvm-project/releases/download/llvmorg-$LlvmVersion/LLVM-$LlvmVersion-win64.exe" $llvmInstaller
    # NSIS: /D must be last and unquoted.
    $proc = Start-Process -FilePath $llvmInstaller -ArgumentList "/S /D=$LlvmDir" -Wait -PassThru
    if ($proc.ExitCode -ne 0 -or -not (Test-Path (Join-Path $LlvmDir "bin\libclang.dll"))) {
        throw "LLVM installation failed (exit code $($proc.ExitCode))."
    }
}
$env:LIBCLANG_PATH = Join-Path $LlvmDir "bin"
Add-SessionPath (Join-Path $LlvmDir "bin")

# ---------------------------------------------------------------------------
Write-Step "Rust $RustVersion"

$cargoBin = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME "bin" } else { Join-Path $env:USERPROFILE ".cargo\bin" }
Add-SessionPath $cargoBin
$rustup = Get-Command rustup -ErrorAction SilentlyContinue
if ($rustup) {
    Write-Host "Found rustup: $($rustup.Source)"
} else {
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "Found cargo without rustup; installing rustup to manage the pinned $RustVersion toolchain."
    }
    $rustupInit = Join-Path $DownloadDir "rustup-init.exe"
    Get-Download "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" $rustupInit
    Invoke-Checked $rustupInit @("-y", "--default-toolchain", "none", "--profile", "minimal")
}
$hasToolchain = @(& rustup toolchain list) -match "^$([regex]::Escape($RustVersion))(\.0)?-"
if ($hasToolchain) {
    Write-Host "Found Rust toolchain $RustVersion."
    if (-not (@(& rustup "+$RustVersion" target list --installed) -contains $RustTarget)) {
        Invoke-Checked rustup @("+$RustVersion", "target", "add", $RustTarget)
    }
    if (-not (@(& rustup "+$RustVersion" component list --installed) -match "^rustfmt")) {
        Invoke-Checked rustup @("+$RustVersion", "component", "add", "rustfmt")
    }
} else {
    Invoke-Checked rustup @("toolchain", "install", $RustVersion, "--profile", "minimal",
        "--component", "rustfmt", "--target", $RustTarget)
}
# Scoped to this session so the user's default toolchain is left alone.
$env:RUSTUP_TOOLCHAIN = $RustVersion

# ---------------------------------------------------------------------------
Write-Step "vcpkg ($VcpkgCommit)"

# A checkout passed with -VcpkgRoot is only checked, never moved to another commit.
if (-not $UseOwnVcpkg) {
    if (-not (Test-Path (Join-Path $VcpkgRoot ".git"))) { throw "-VcpkgRoot $VcpkgRoot is not a vcpkg git checkout." }
    $userHead = (& git -C $VcpkgRoot rev-parse HEAD).Trim()
    if ($userHead -ne $VcpkgCommit) {
        throw "-VcpkgRoot $VcpkgRoot is at $userHead; RustDesk needs $VcpkgCommit. Check it out there or omit -VcpkgRoot."
    }
    Write-Host "Using vcpkg at $VcpkgRoot"
}

if (-not (Test-Path (Join-Path $VcpkgRoot ".git"))) {
    Invoke-Checked git @("clone", "https://github.com/microsoft/vcpkg", $VcpkgRoot)
}
$head = (& git -C $VcpkgRoot rev-parse HEAD).Trim()
if ($head -ne $VcpkgCommit) {
    if (-not (Test-Native git @("-C", $VcpkgRoot, "cat-file", "-e", "$VcpkgCommit^{commit}"))) {
        Invoke-Checked git @("-C", $VcpkgRoot, "fetch", "origin")
    }
    Invoke-Checked git @("-C", $VcpkgRoot, "checkout", "--quiet", $VcpkgCommit)
    Remove-Item -Force (Join-Path $VcpkgRoot "vcpkg.exe") -ErrorAction SilentlyContinue
}
$vcpkgExe = Join-Path $VcpkgRoot "vcpkg.exe"
if (-not (Test-Path $vcpkgExe)) {
    Invoke-Checked (Join-Path $VcpkgRoot "bootstrap-vcpkg.bat") @("-disableMetrics")
}
$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKG_DEFAULT_HOST_TRIPLET = $Triplet

Write-Step "vcpkg dependencies from vcpkg.json (first run builds ffmpeg and takes a while)"
Push-Location $RepoRoot
try {
    Invoke-Checked $vcpkgExe @("install", "--triplet", $Triplet, "--x-install-root=$VcpkgRoot\installed")
} catch {
    Write-Host "vcpkg build logs: $VcpkgRoot\buildtrees\<port>\*.log" -ForegroundColor Yellow
    throw
} finally {
    Pop-Location
}

# ---------------------------------------------------------------------------
Write-Step "Flutter $FlutterVersion"

$flutterBat = Join-Path $FlutterRoot "bin\flutter.bat"
if (-not $UseOwnFlutter) {
    if (-not (Test-Path $flutterBat) -or -not (Test-Path (Join-Path $FlutterRoot ".git"))) {
        throw "-FlutterRoot $FlutterRoot is not a Flutter SDK git checkout."
    }
    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { $machine = (& $flutterBat --version --machine 2>$null) -join "`n" } finally { $ErrorActionPreference = $old }
    $userFlutterVersion = $null
    if ($machine -match '"frameworkVersion"\s*:\s*"([^"]+)"') { $userFlutterVersion = $Matches[1] }
    if ($userFlutterVersion -ne $FlutterVersion) {
        throw "-FlutterRoot $FlutterRoot is Flutter $userFlutterVersion; RustDesk needs $FlutterVersion. Omit -FlutterRoot to use $ToolsDir\flutter."
    }
    Write-Host "Using Flutter $userFlutterVersion at $FlutterRoot; RustDesk's engine and patch will be applied to it."
}
if (-not (Test-Path $flutterBat)) {
    $flutterZip = Join-Path $DownloadDir "flutter_windows_$FlutterVersion-stable.zip"
    Get-Download "https://storage.googleapis.com/flutter_infra_release/releases/stable/windows/flutter_windows_$FlutterVersion-stable.zip" $flutterZip
    # The archive contains a top-level "flutter" folder (a git checkout, which the patch below relies on).
    Invoke-Checked "$env:SystemRoot\System32\tar.exe" @("-xf", $flutterZip, "-C", $ToolsDir)
}
Add-SessionPath (Join-Path $FlutterRoot "bin")
Invoke-Checked $flutterBat @("config", "--no-analytics")
Invoke-Checked $flutterBat @("precache", "--windows")

# https://github.com/flutter/flutter/issues/155685
$engineDir = Join-Path $FlutterRoot "bin\cache\artifacts\engine\windows-x64-release"
$engineStamp = Join-Path $engineDir ".rustdesk-engine"
if (-not (Test-Path $engineStamp)) {
    Write-Host "Replacing the Flutter windows-x64-release engine with RustDesk's build."
    $engineZip = Join-Path $DownloadDir "rustdesk-engine-windows-x64-release.zip"
    $engineTmp = Join-Path $DownloadDir "rustdesk-engine"
    Get-Download "https://github.com/rustdesk/engine/releases/download/main/windows-x64-release.zip" $engineZip
    Remove-Item -Recurse -Force $engineTmp -ErrorAction SilentlyContinue
    Expand-Archive -Path $engineZip -DestinationPath $engineTmp
    Copy-Item -Recurse -Force (Join-Path $engineTmp "*") $engineDir
    Remove-Item -Recurse -Force $engineTmp
    New-Item -ItemType File $engineStamp | Out-Null
}

$flutterPatch = Join-Path $RepoRoot ".github\patches\flutter_3.24.4_dropdown_menu_enableFilter.diff"
if (Test-Native git @("-C", $FlutterRoot, "apply", "--check", $flutterPatch)) {
    Invoke-Checked git @("-C", $FlutterRoot, "apply", $flutterPatch)
} elseif (-not (Test-Native git @("-C", $FlutterRoot, "apply", "--reverse", "--check", $flutterPatch))) {
    throw "Cannot apply $flutterPatch to $FlutterRoot (is it a Flutter $FlutterVersion git checkout?)."
}

# Flutter 3.24 maps any VS newer than 2022 to the "Visual Studio 16 2019" CMake generator, which fails
# when VS 2026 is the newest install. Teach it the VS 2026 generator.
$vsDart = Join-Path $FlutterRoot "packages\flutter_tools\lib\src\windows\visual_studio.dart"
$vsDartText = [IO.File]::ReadAllText($vsDart)
if ($vsDartText -notmatch "Visual Studio 18 2026") {
    $nl = if ($vsDartText.Contains("`r`n")) { "`r`n" } else { "`n" }
    $patched = [regex]::Replace($vsDartText, "([ \t]*)17 => 'Visual Studio 17 2022',",
        { param($m) "$($m.Groups[1].Value)18 => 'Visual Studio 18 2026',$nl$($m.Value)" })
    if ($patched -eq $vsDartText) { throw "Cannot add the VS 2026 generator to $vsDart." }
    [IO.File]::WriteAllText($vsDart, $patched)
    # flutter.bat only rebuilds its tool snapshot when this stamp is missing or the git HEAD changes.
    Remove-Item -Force (Join-Path $FlutterRoot "bin\cache\flutter_tools.stamp") -ErrorAction SilentlyContinue
    Write-Host "Added the VS 2026 CMake generator to Flutter."
}

# ---------------------------------------------------------------------------
Write-Step "Source tree"

Invoke-Checked git @("-C", $RepoRoot, "submodule", "update", "--init", "--recursive")

$bridgeFiles = @(
    "src\bridge_generated.rs",
    "src\bridge_generated.io.rs",
    "flutter\lib\generated_bridge.dart",
    "flutter\lib\generated_bridge.freezed.dart"
)
$bridgeMissing = @($bridgeFiles | Where-Object { -not (Test-Path (Join-Path $RepoRoot $_)) })
if ($GenerateBridge -or $bridgeMissing.Count -gt 0) {
    Write-Step "flutter_rust_bridge $FrbVersion codegen"
    Invoke-Checked cargo @("install", "cargo-expand", "--version", $CargoExpandVersion, "--locked")
    Invoke-Checked cargo @("install", "flutter_rust_bridge_codegen", "--version", $FrbVersion, "--features", "uuid", "--locked")
    Push-Location (Join-Path $RepoRoot "flutter")
    try { Invoke-Checked $flutterBat @("pub", "get") } finally { Pop-Location }
    Push-Location $RepoRoot
    try {
        Invoke-Checked flutter_rust_bridge_codegen @(
            "--rust-input", "./src/flutter_ffi.rs",
            "--dart-output", "./flutter/lib/generated_bridge.dart",
            "--c-output", "./flutter/macos/Runner/bridge_generated.h",
            "--llvm-path", $LlvmDir)
        Copy-Item -Force "flutter\macos\Runner\bridge_generated.h" "flutter\ios\Runner\bridge_generated.h"
    } finally {
        Pop-Location
    }
}

# ---------------------------------------------------------------------------
Write-Step "Build RustDesk"

Push-Location $RepoRoot
try {
    Invoke-Checked $Python @("build.py", "--portable", "--flutter", "--skip-portable-pack", "--hwcodec", "--vram")
} finally {
    Pop-Location
}

$releaseDir = Join-Path $RepoRoot "flutter\build\windows\x64\runner\Release"
if (-not (Test-Path (Join-Path $releaseDir "rustdesk.exe"))) { throw "Build finished but $releaseDir\rustdesk.exe is missing." }
Remove-Item -Recurse -Force $OutputDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Split-Path $OutputDir) | Out-Null
Copy-Item -Recurse $releaseDir $OutputDir

# ---------------------------------------------------------------------------
if (-not $SkipExtras) {
    Write-Step "Virtual display and printer drivers"

    $extrasDir = Join-Path $DownloadDir ("extras-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory $extrasDir | Out-Null
    try {
        $usbZip = Join-Path $extrasDir "usbmmidd_v2.zip"
        Get-Download "https://github.com/rustdesk-org/rdev/releases/download/usbmmidd_v2/usbmmidd_v2.zip" $usbZip
        Expand-Archive $usbZip -DestinationPath $extrasDir
        $usbDir = Join-Path $extrasDir "usbmmidd_v2"
        Remove-Item -Recurse -Force (Join-Path $usbDir "Win32")
        Remove-Item -Force (Join-Path $usbDir "deviceinstaller64.exe"), (Join-Path $usbDir "deviceinstaller.exe"), (Join-Path $usbDir "usbmmidd.bat")
        Copy-Item -Recurse -Force $usbDir $OutputDir

        try {
            $driverName = "rustdesk_printer_driver_v4-1.4"
            $driverZip = Join-Path $extrasDir "$driverName.zip"
            $adapterZip = Join-Path $extrasDir "printer_driver_adapter.zip"
            $sums = Join-Path $extrasDir "sha256sums"
            Get-Download "https://github.com/rustdesk/hbb_common/releases/download/driver/$driverName.zip" $driverZip
            Get-Download "https://github.com/rustdesk/hbb_common/releases/download/driver/printer_driver_adapter.zip" $adapterZip
            Get-Download "https://github.com/rustdesk/hbb_common/releases/download/driver/sha256sums" $sums

            $expectedDriver = (Select-String -Path $sums -Pattern "^([a-fA-F0-9]{64}) \*$([regex]::Escape($driverName))\.zip$").Matches.Groups[1].Value
            $expectedAdapter = (Select-String -Path $sums -Pattern '^([a-fA-F0-9]{64}) \*printer_driver_adapter\.zip$').Matches.Groups[1].Value
            if ($expectedDriver -eq (Get-FileHash $driverZip -Algorithm SHA256).Hash -and
                $expectedAdapter -eq (Get-FileHash $adapterZip -Algorithm SHA256).Hash) {
                Expand-Archive $driverZip -DestinationPath $extrasDir
                $driversOut = Join-Path $OutputDir "drivers"
                New-Item -ItemType Directory -Force $driversOut | Out-Null
                Copy-Item -Recurse -Force (Join-Path $extrasDir $driverName) (Join-Path $driversOut "RustDeskPrinterDriver")
                Expand-Archive $adapterZip -DestinationPath $extrasDir
                Copy-Item -Force (Join-Path $extrasDir "printer_driver_adapter.dll") $OutputDir
            } else {
                Write-Warning "Printer driver checksums do not match; skipping the printer driver."
            }
        } catch {
            Write-Warning "Skipping the printer driver: $_"
        }
    } finally {
        Remove-Item -Recurse -Force $extrasDir -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------
if ($Installer) {
    Write-Step "Single-file installer"

    $version = (Select-String -Path (Join-Path $RepoRoot "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1).Matches[0].Groups[1].Value
    Push-Location (Join-Path $RepoRoot "libs\portable")
    try {
        Invoke-Checked $Python @("-m", "pip", "install", "-r", "requirements.txt")
        Invoke-Checked $Python @("generate.py", "-f", $OutputDir, "-o", ".", "-e", (Join-Path $OutputDir "rustdesk.exe"))
    } finally {
        Pop-Location
    }
    $installerExe = Join-Path $RepoRoot "rustdesk-$version-install.exe"
    Move-Item -Force (Join-Path $RepoRoot "target\release\rustdesk-portable-packer.exe") $installerExe
    Write-Host "Installer: $installerExe" -ForegroundColor Green
}

Write-Host ""
Write-Host "Done. Run: $(Join-Path $OutputDir 'rustdesk.exe')" -ForegroundColor Green
