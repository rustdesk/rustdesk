<#
.SYNOPSIS
Select the local Windows Privacy Mode 1 background for a customized DLL.
.EXAMPLE
.\set-privacy-background.ps1 -Mode black -RustDeskDirectory 'C:\Program Files\RustDesk'
.EXAMPLE
.\set-privacy-background.ps1 -Mode wallpaper -RustDeskDirectory 'C:\Program Files\RustDesk'
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('notice', 'black', 'wallpaper')]
    [string]$Mode,

    [Parameter(Mandatory = $true)]
    [string]$RustDeskDirectory,

    # Optional image for the wallpaper mode. Otherwise export this user's wallpaper.
    [string]$ImagePath
)

$ErrorActionPreference = 'Stop'
$directory = (Resolve-Path -LiteralPath $RustDeskDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $directory 'WindowInjection.dll') -PathType Leaf)) {
    throw "WindowInjection.dll not found in $directory"
}

if ($Mode -eq 'wallpaper') {
    if (-not $ImagePath) {
        $ImagePath = (Get-ItemProperty -LiteralPath 'HKCU:\Control Panel\Desktop' -Name WallPaper).WallPaper
    }
    if (-not $ImagePath -or -not (Test-Path -LiteralPath $ImagePath -PathType Leaf)) {
        throw 'No wallpaper image found. Supply -ImagePath with a local image.'
    }
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Image]::FromFile((Resolve-Path -LiteralPath $ImagePath).Path)
    $staging = Join-Path $directory 'privacy-wallpaper.new.png'
    try {
        $image.Save($staging, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $image.Dispose()
    }
    # Disable privacy mode before replacing the image; the DLL holds it while active.
    Move-Item -LiteralPath $staging -Destination (Join-Path $directory 'privacy-wallpaper.png') -Force
}

$configuration = Join-Path $directory 'privacy-background.ini'
Set-Content -LiteralPath $configuration -Encoding Unicode -Value "[Privacy]`r`nBackground=$Mode"
Write-Output "Privacy Mode 1 background: $Mode. Exit and re-enter privacy mode to apply. Ctrl+P still exits locally."
