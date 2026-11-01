<#
.SYNOPSIS
Installs RustDesk for unattended access and prints the connection info.

.DESCRIPTION
Installs RustDesk as a Windows service, so it runs from boot whether or not
anyone opens the app, sets a permanent password and prints the ID and password
needed to connect. Run it from an elevated PowerShell prompt. Safe to re-run.

.PARAMETER Installer
RustDesk .exe or .msi, as a local path or http(s) URL. Omit if RustDesk is
already installed.

.PARAMETER Password
Permanent password to set. A random 16-character one is generated if omitted.
Avoid double quotes: Windows PowerShell does not pass them through intact.

.PARAMETER Config
Server config: "host=...,key=..." (optionally ",api=...,relay=...") or the
encoded config string from your server console.

.PARAMETER ReportUrl
URL to POST the connection info to as JSON.

.PARAMETER OutFile
File to write the connection info to as JSON. It contains the password.

.PARAMETER AppName
Name of the installed app, for renamed builds.

.EXAMPLE
.\install-windows.ps1 -Installer https://example.com/rustdesk-x86_64.exe -Config "host=rd.example.com,key=YOUR_PUBLIC_KEY"
#>
[CmdletBinding()]
param(
    [string]$Installer,
    [string]$Password,
    [string]$Config,
    [string]$ReportUrl,
    [string]$OutFile,
    [string]$AppName = "RustDesk"
)

$ErrorActionPreference = "Stop"
# Windows PowerShell's download progress bar slows Invoke-WebRequest to a crawl.
$ProgressPreference = "SilentlyContinue"

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this script from an elevated (Administrator) PowerShell prompt."
}

$exe = Join-Path $env:ProgramFiles "$AppName\$AppName.exe"

function New-RandomPassword([int]$Length = 16) {
    $chars = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789".ToCharArray()
    $limit = 256 - (256 % $chars.Length)
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    $byte = New-Object byte[] 1
    $result = ""
    while ($result.Length -lt $Length) {
        $rng.GetBytes($byte)
        if ($byte[0] -lt $limit) {
            $result += $chars[$byte[0] % $chars.Length]
        }
    }
    $result
}

function Invoke-RustDesk([string[]]$Arguments) {
    # rustdesk.exe is a GUI-subsystem binary; piping its output is what makes
    # PowerShell wait for it to exit and capture what it prints.
    $lines = @(& $exe @Arguments | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
    if ($lines.Count) { $lines[-1] } else { "" }
}

# The CLI talks to the running service over this pipe. Until it answers,
# --get-id falls back to a freshly generated local ID and settings would be
# written to this admin's profile instead of the service's.
function Test-ServiceIpc {
    $pipe = New-Object IO.Pipes.NamedPipeClientStream(".", "$AppName\query", [IO.Pipes.PipeDirection]::InOut)
    try {
        $pipe.Connect(1000)
        $true
    } catch {
        $false
    } finally {
        $pipe.Dispose()
    }
}

function Wait-Until([scriptblock]$Condition, [int]$TimeoutSeconds, [string]$What) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (-not (& $Condition)) {
        if ((Get-Date) -gt $deadline) {
            throw "Timed out after $TimeoutSeconds s waiting for $What."
        }
        Start-Sleep -Seconds 2
    }
}

if ($Installer) {
    if ($Installer -match "^https?://") {
        $name = [IO.Path]::GetFileName([Uri]::UnescapeDataString(([Uri]$Installer).AbsolutePath))
        if (-not $name) { $name = "rustdesk-setup.exe" }
        $downloadDir = Join-Path $env:TEMP ("rustdesk-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Path $downloadDir | Out-Null
        $localInstaller = Join-Path $downloadDir $name
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Write-Host "Downloading $Installer"
        Invoke-WebRequest -Uri $Installer -OutFile $localInstaller -UseBasicParsing
        $Installer = $localInstaller
    }
    $Installer = (Resolve-Path $Installer).Path

    Write-Host "Installing $Installer"
    if ($Installer -like "*.msi") {
        $msi = Start-Process -FilePath msiexec.exe -ArgumentList @("/i", "`"$Installer`"", "/qn", "/norestart") -Wait -PassThru
        if ($msi.ExitCode -ne 0 -and $msi.ExitCode -ne 3010) {
            throw "msiexec failed with exit code $($msi.ExitCode)."
        }
    } else {
        # Not -Wait: the installer starts long-lived processes that -Wait would also wait on.
        Start-Process -FilePath $Installer -ArgumentList "--silent-install" | Out-Null
    }
} elseif (-not (Test-Path $exe)) {
    throw "$exe not found. Pass -Installer to install RustDesk."
}

Wait-Until { Test-Path $exe } 180 "$exe to be installed"
Wait-Until { Get-Service -Name $AppName -ErrorAction SilentlyContinue } 120 "the $AppName service (if it was stopped from the RustDesk menu, choose Start service there and re-run)"
$service = Get-Service -Name $AppName
if ($service.StartType -ne "Automatic") {
    Set-Service -Name $AppName -StartupType Automatic
}
if ($service.Status -ne "Running") {
    Start-Service -Name $AppName
}
Wait-Until { Test-ServiceIpc } 120 "the $AppName service to start answering"

if ($Config) {
    Write-Host "Applying server config"
    Invoke-RustDesk @("--config", $Config) | Out-Null
}

if ((Invoke-RustDesk @("--option", "approve-mode")) -eq "click") {
    Write-Host "Switching approve mode from click-only to password"
    Invoke-RustDesk @("--option", "approve-mode", "password") | Out-Null
}
if ((Invoke-RustDesk @("--option", "verification-method")) -eq "use-temporary-password") {
    Write-Host "Enabling the permanent password alongside the one-time password"
    Invoke-RustDesk @("--option", "verification-method", "use-both-passwords") | Out-Null
}

if (-not $Password) {
    $Password = New-RandomPassword
}
$result = Invoke-RustDesk @("--password", $Password)
if ($result -ne "Done!") {
    throw "Setting the password failed: $result"
}

$id = Invoke-RustDesk @("--get-id")
if (-not $id) {
    throw "RustDesk did not report its ID."
}
$server = Invoke-RustDesk @("--option", "custom-rendezvous-server")
if (-not $server) { $server = "public RustDesk server" }

$info = [ordered]@{
    computer = $env:COMPUTERNAME
    id       = $id
    password = $Password
    server   = $server
    text     = "$($env:COMPUTERNAME): RustDesk ID $id, password $Password, server $server"
}
$json = $info | ConvertTo-Json

Write-Host ""
Write-Host "RustDesk unattended access is ready."
Write-Host "  Computer : $($info.computer)"
Write-Host "  ID       : $($info.id)"
Write-Host "  Password : $($info.password)"
Write-Host "  Server   : $($info.server)"
Write-Host ""

if ($OutFile) {
    Set-Content -Path $OutFile -Value $json -Encoding UTF8
    Write-Host "Saved to $OutFile"
}

if ($ReportUrl) {
    if ($ReportUrl -notmatch "^https://") {
        Write-Warning "ReportUrl is not https; the password will be sent unencrypted."
    }
    try {
        Invoke-RestMethod -Method Post -Uri $ReportUrl -ContentType "application/json" -Body ([Text.Encoding]::UTF8.GetBytes($json)) | Out-Null
        Write-Host "Reported to $ReportUrl"
    } catch {
        Write-Warning "Reporting to $ReportUrl failed: $_"
        exit 1
    }
}
