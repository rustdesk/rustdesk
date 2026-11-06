<#
.SYNOPSIS
    Frees the disk space used by build_windows.ps1, keeping system-wide tools.

.DESCRIPTION
    Removes by default:
      * <repo>\target and <repo>\flutter\build (Rust and Flutter build outputs)
      * <repo>\rustdesk-*-install.exe (from build_windows.ps1 -Installer)
      * <ToolsDir>\vcpkg, <ToolsDir>\flutter, <ToolsDir>\downloads
      * <ToolsDir>\LLVM, through its uninstaller, so it also leaves "Apps & features"
    Never touched: Git, Python, Visual Studio, rustup itself, Developer Mode, an LLVM
    installed elsewhere, and SDKs passed to build_windows.ps1 with -VcpkgRoot / -FlutterRoot.

    Opt-in, because other projects may use them too:
      -IncludeSharedCaches   vcpkg binary cache and Flutter pub cache under %LOCALAPPDATA%
      -RemoveRustToolchain   the Rust 1.75 toolchain and cargo-expand / flutter_rust_bridge_codegen

    Running build_windows.ps1 again restores everything.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\cleanup_windows.ps1 -WhatIf

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\cleanup_windows.ps1 -KeepOutput -Yes
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    # Must match the -ToolsDir given to build_windows.ps1.
    [string]$ToolsDir = "C:\rustdesk-deps",
    # Must match the -OutputDir given to build_windows.ps1. Defaults to <repo>\target\rustdesk-windows.
    [string]$OutputDir,
    # Keep the built app in -OutputDir and the installer exe; remove everything else.
    [switch]$KeepOutput,
    [switch]$IncludeSharedCaches,
    [switch]$RemoveRustToolchain,
    # Do not ask for confirmation.
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

# Keep in sync with build_windows.ps1.
$RustVersion = "1.75"

$RepoRoot = $PSScriptRoot
if (-not $OutputDir) { $OutputDir = Join-Path $RepoRoot "target\rustdesk-windows" }
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
$TargetDir = Join-Path $RepoRoot "target"

function Get-SizeMB([string]$Path) {
    if (Test-Path $Path -PathType Leaf) { return (Get-Item $Path).Length / 1MB }
    $sum = (Get-ChildItem $Path -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
    if ($sum) { return $sum / 1MB } else { return 0 }
}

# rd copes with read-only files (git packs) and is much faster than Remove-Item on big trees;
# robocopy /MIR from an empty folder handles paths beyond MAX_PATH (vcpkg buildtrees).
function Remove-Tree([string]$Path) {
    if (-not (Test-Path $Path)) { return }
    if (Test-Path $Path -PathType Leaf) {
        Remove-Item -Force $Path
        return
    }
    & cmd.exe /c rd /s /q "`"$Path`"" 2>$null
    if (Test-Path $Path) {
        $empty = Join-Path $env:TEMP ("rustdesk-empty-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory $empty | Out-Null
        & robocopy.exe $empty $Path /MIR /NFL /NDL /NJH /NJS /NP /R:1 /W:1 | Out-Null
        Remove-Item -Force $empty
        & cmd.exe /c rd /s /q "`"$Path`"" 2>$null
    }
    if (Test-Path $Path) { Write-Warning "Could not remove all of $Path (files in use? close RustDesk, IDEs and terminals using it)." }
}

# ---------------------------------------------------------------------------
# Plan
$items = New-Object System.Collections.Generic.List[object]
function Add-CleanupItem([string]$Path, [string]$What) {
    if (Test-Path $Path) { $items.Add([pscustomobject]@{ Path = $Path; What = $What }) }
}

if ($KeepOutput -and $OutputDir.StartsWith($TargetDir + "\", [StringComparison]::OrdinalIgnoreCase)) {
    # Remove target's children except the branch that leads to the output folder.
    $keepTop = Join-Path $TargetDir ($OutputDir.Substring($TargetDir.Length + 1).Split("\")[0])
    if ($keepTop -ne $OutputDir) { throw "-KeepOutput needs -OutputDir to be a direct child of $TargetDir or outside it." }
    Get-ChildItem $TargetDir -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -ne $keepTop } |
        ForEach-Object { Add-CleanupItem $_.FullName "Rust build output" }
} else {
    Add-CleanupItem $TargetDir "Rust build output"
    if (-not $KeepOutput -and -not $OutputDir.StartsWith($TargetDir + "\", [StringComparison]::OrdinalIgnoreCase)) {
        Add-CleanupItem $OutputDir "Built RustDesk app"
    }
}
Add-CleanupItem (Join-Path $RepoRoot "flutter\build") "Flutter build output"
if (-not $KeepOutput) {
    Get-ChildItem $RepoRoot -Filter "rustdesk-*-install.exe" -File -ErrorAction SilentlyContinue |
        ForEach-Object { Add-CleanupItem $_.FullName "Installer exe" }
}

$llvmDir = Join-Path $ToolsDir "LLVM"
Add-CleanupItem $llvmDir "LLVM (uninstalled first)"
Add-CleanupItem (Join-Path $ToolsDir "vcpkg") "vcpkg and its built packages"
Add-CleanupItem (Join-Path $ToolsDir "flutter") "Flutter SDK (RustDesk-patched)"
Add-CleanupItem (Join-Path $ToolsDir "downloads") "Downloaded installers and archives"

if ($IncludeSharedCaches) {
    Add-CleanupItem (Join-Path $env:LOCALAPPDATA "vcpkg\archives") "vcpkg binary cache (shared)"
    Add-CleanupItem (Join-Path $env:LOCALAPPDATA "Pub\Cache") "Flutter pub cache (shared)"
}

$cargoBin = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME "bin" } else { Join-Path $env:USERPROFILE ".cargo\bin" }
$rustup = Get-Command rustup -ErrorAction SilentlyContinue
if (-not $rustup -and (Test-Path (Join-Path $cargoBin "rustup.exe"))) { $rustup = Get-Command (Join-Path $cargoBin "rustup.exe") }
$cargoTools = @("flutter_rust_bridge_codegen", "cargo-expand") | Where-Object { Test-Path (Join-Path $cargoBin "$_.exe") }
$hasToolchain = $false
if ($RemoveRustToolchain -and $rustup) {
    $hasToolchain = [bool](@(& $rustup.Source toolchain list) -match "^$([regex]::Escape($RustVersion))(\.0)?-")
}

if ($items.Count -eq 0 -and -not ($RemoveRustToolchain -and ($hasToolchain -or $cargoTools))) {
    Write-Host "Nothing to clean."
    return
}

Write-Host "Measuring..."
$total = 0
Write-Host ""
foreach ($i in $items) {
    $mb = Get-SizeMB $i.Path
    $total += $mb
    Write-Host ("{0,10:N0} MB  {1}  ({2})" -f $mb, $i.Path, $i.What)
}
if ($RemoveRustToolchain) {
    if ($hasToolchain) { Write-Host ("{0,13}  Rust toolchain {1} (rustup)" -f "", $RustVersion) }
    foreach ($t in $cargoTools) { Write-Host ("{0,13}  {1} (cargo install)" -f "", $t) }
}
Write-Host ("{0,10:N0} MB  total" -f $total) -ForegroundColor Cyan
Write-Host ""

if ($WhatIfPreference) { return }
if (-not $Yes) {
    $answer = Read-Host "Delete the items above? [y/N]"
    if ($answer -notmatch "^(y|yes)$") {
        Write-Host "Cancelled."
        return
    }
}

# ---------------------------------------------------------------------------
# Execute
$uninstaller = Join-Path $llvmDir "Uninstall.exe"
if (Test-Path $uninstaller) {
    Write-Host "Uninstalling LLVM from $llvmDir"
    # _?= runs the NSIS uninstaller in place, so -Wait really waits for it.
    $proc = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$llvmDir" -Wait -PassThru
    if ($proc.ExitCode -ne 0) { Write-Warning "LLVM uninstaller exited with $($proc.ExitCode); deleting the folder anyway." }
}

foreach ($i in $items) {
    Write-Host "Removing $($i.Path)"
    Remove-Tree $i.Path
}

if ((Test-Path $ToolsDir) -and -not (Get-ChildItem $ToolsDir -Force)) {
    Remove-Item -Force $ToolsDir
}

if ($RemoveRustToolchain) {
    foreach ($t in $cargoTools) {
        Write-Host "Uninstalling $t"
        & (Join-Path $cargoBin "cargo.exe") uninstall $t
    }
    if ($hasToolchain) {
        Write-Host "Uninstalling Rust toolchain $RustVersion"
        & $rustup.Source toolchain uninstall $RustVersion
    }
}

Write-Host ""
Write-Host "Done. Run build_windows.ps1 again to rebuild." -ForegroundColor Green
