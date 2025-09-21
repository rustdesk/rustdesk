param(
    [string]$Relay,
    [string]$ID,
    [string]$API,
    [string]$Key,
    [string]$Host
)
$ErrorActionPreference = "Stop"

function Ensure-Tool($name, $test, $install) {
    if (-not (Invoke-Expression $test)) {
        Write-Host "Installing $name..."
        Invoke-Expression $install
    } else { Write-Host "$name OK" }
}

# Prereqs (VS C++ Build Tools, Rust, Flutter). We only check presence and print guidance.
Write-Host "Checking prerequisites..."
try { rustc --version | Out-Null } catch { Write-Warning "Rust not found. Install from https://rustup.rs" }
try { flutter --version | Out-Null } catch { Write-Warning "Flutter not found. Install from https://docs.flutter.dev/get-started/install" }

# Derive endpoints from -Host if given
if ($Host) {
    if (-not $Relay) { $Relay = "$Host:21117" }
    if (-not $ID)    { $ID    = "$Host:21116" }
    if (-not $API)   { $API   = "http://$Host:21114" }
}
if (-not $Key) { throw "Please supply -Key (your server key)." }
if (-not $Relay -or -not $ID -or -not $API) { throw "Please supply -Relay, -ID and -API or -Host." }

# Clone RustDesk
if (-not (Test-Path rustdesk)) {
    git clone https://github.com/rustdesk/rustdesk.git
}
cd rustdesk

# Apply patch
git reset --hard
git checkout -B lite-support
$patchPath = Join-Path (Split-Path (Split-Path $PSScriptRoot)) "rustdesk-lite.patch"
git apply $patchPath

# Env for compile-time injection
$env:RDLITE_RELAY=$Relay
$env:RDLITE_ID_SERVER=$ID
$env:RDLITE_API=$API
$env:RDLITE_KEY=$Key
$env:FLUTTER_WINDOWS="1"
$env:FLUTTER_BUILD_DIRECTIVE="--dart-define=RDLITE_INCOMING_ONLY=true"

# Build
# Use cargo build to compile core with the incoming_only feature
cargo build --release --features incoming_only

# Build Flutter windows app
cd flutter
flutter config --enable-windows-desktop | Out-Null
flutter pub get
flutter build windows --dart-define=RDLITE_INCOMING_ONLY=true

# Stage portable
cd ..
$dst = Join-Path $PWD "dist\lite-win"
New-Item -Force -ItemType Directory $dst | Out-Null
Copy-Item -Recurse "target\release\rustdesk.exe" $dst
Copy-Item -Recurse "flutter\build\windows\x64\runner\Release\*" $dst -ErrorAction SilentlyContinue

# Add a small launcher if built
$launcherDir = "packaging\windows\lite_launcher"
if (Test-Path $launcherDir) {
    pushd $launcherDir
    cl /O2 /EHsc RustDeskLite.cpp /Fe:RustDeskLite.exe
    popd
    Copy-Item "$launcherDir\RustDeskLite.exe" $dst -ErrorAction SilentlyContinue
} else {
    # fallback: copy as RustDeskLite.exe
    Copy-Item $dst\rustdesk.exe "$dst\RustDeskLite.exe"
}

# Harden config at runtime (optional)
$cfg = "$env:APPDATA\RustDesk\config\RustDesk2.toml"
New-Item -Force -ItemType Directory (Split-Path $cfg) | Out-Null
$toml = @"
rendezvous_server = '$ID'
nat_type = 1
serial = 0

[options]
custom-rendezvous-server = '$ID'
key = '$Key'
relay-server = '$Relay'
api-server = '$API'
"@
$toml | Set-Content $cfg -Encoding UTF8

# Lock down: make read-only
icacls $cfg /inheritance:r /grant:r "$($env:USERNAME):(R)" /t | Out-Null

# Optional firewall block to prevent unwanted outbound except to your host
$hostOnly = ($Relay.Split(":")[0])
New-NetFirewallRule -DisplayName "RustDeskLite-Restrict" -Program "$dst\RustDeskLite.exe" -Direction Outbound -Action Block -Profile Any -Enabled True | Out-Null
New-NetFirewallRule -DisplayName "RustDeskLite-Allow-Server" -Program "$dst\RustDeskLite.exe" -Direction Outbound -Action Allow -RemoteAddress $hostOnly -Profile Any -Enabled True | Out-Null

Write-Host "`nBuilt at: $dst"
