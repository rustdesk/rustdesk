# Tefway Remote — Debian/Ubuntu x86_64 (.deb)

Produces `tefway-remote-master_<versao>_amd64.deb` and
`tefway-remote-client_<versao>_amd64.deb`.

## Environment

Validated target: Debian 12 or Ubuntu 22.04/24.04, x86_64.

| Tool | Version |
| --- | --- |
| Rust | 1.75 (stable) — CI pin; newer toolchains build but are not the validated one |
| Flutter | 3.24.5 (stable) |
| vcpkg | `VCPKG_ROOT` set, baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d` |
| Python | 3.10+ |
| dpkg-dev | provides `dpkg-deb` |

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config dpkg-dev curl git python3 \
  clang libclang-dev cmake ninja-build \
  libgtk-3-dev libxcb-randr0-dev libxdo-dev libxfixes-dev \
  libasound2-dev libpulse-dev libva-dev libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libsystemd-dev libdrm-dev libwayland-dev libxkbcommon-dev \
  libayatana-appindicator3-dev librust-alsa-sys-dev
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.75
```

Flutter 3.24.5, matching the pinned version:

```bash
cd /opt && sudo tar xf flutter_linux_3.24.5-stable.tar.xz && export PATH=/opt/flutter/bin:$PATH
```

vcpkg:

```bash
git clone https://github.com/microsoft/vcpkg /opt/vcpkg
/opt/vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=/opt/vcpkg
$VCPKG_ROOT/vcpkg install --x-install-root="$VCPKG_ROOT/installed"
```

## One-time: generate the bridge

`flutter/lib/generated_bridge.dart` and `src/bridge_generated*.rs` are git
ignored. Generate them with `flutter_rust_bridge_codegen` pinned to 1.80.1 (the
version in `flutter/pubspec.yaml`), then apply the ffigen workaround from
`build.py` (`ffi_bindgen_function_refactor`).

## Build

```bash
export TEFWAY_PROFILE=master   # or: client — package.py sets it again per run
python3 res/tefway/package.py --profile master --format deb --build
python3 res/tefway/package.py --profile client --format deb --build
```

`--build` runs `cargo build --locked --features flutter --lib --release` and
`flutter build linux --release`; packaging then stages the Flutter bundle with a
per-profile package name, control file, systemd unit, desktop entry and the
16–512 px icons.

Add `--with-hwcodec` for hardware codec support and
`--with-unix-file-copy-paste` if the deployment needs file copy/paste on Linux.

## Output

```
build/tefway/master/tefway-remote-master_<versao>_amd64.deb
build/tefway/client/tefway-remote-client_<versao>_amd64.deb
```

Installed layout (both profiles): `/usr/share/<pkg>/` (bundle), `/usr/bin/<bin>`
symlink, `/usr/share/applications/<pkg>.desktop`,
`/usr/share/icons/hicolor/{16..512}x{...}/apps/<pkg>.png`,
`/etc/systemd/system/<pkg>.service`. The two packages `Conflicts:` each other and
`rustdesk`, so a stock RustDesk install is never mixed with a Tefway one.

## Icon verification

```bash
dpkg-deb -c build/tefway/master/tefway-remote-master_*.deb | grep -E "icons|desktop|service"
# after installing:
gtk-update-icon-cache -f /usr/share/icons/hicolor
```
Then check the application menu, the dock, the window icon, and
`ls /usr/share/icons/hicolor/*/apps/tefway-remote-master.png`.

## Smoke test

```bash
sudo dpkg -i build/tefway/client/tefway-remote-client_*.deb
systemctl status tefway-remote-client.service
tefway-remote-client        # or launch it from the application menu
```

1. Client shows its ID; there is no field to connect to a peer.
2. Master connects → Client shows the accept dialog → accept, session opens;
   decline, the attempt ends and no control is granted.
3. Reconnect → the Client asks again.
4. Master refuses incoming logins (check `/var/log/rustdesk` or the app log).
5. Settings → Network shows `remote.tefway.com.br` and the Tefway key, read-only.

Remove with `sudo apt remove tefway-remote-client` (the unit and desktop entry
are removed by the package scripts).

## Known issues and limits

- The systemd unit starts `/usr/share/<pkg>/<bin> --service`, the same service
  model as upstream. It is what makes the accept dialog reachable from the login
  screen; without it, sessions from the lock screen cannot be captured.
- Wayland: capture on a Wayland session needs the PipeWire portal and, on some
  desktops, a one-time permission dialog. X11 remains the reliable path; this is
  an upstream RustDesk limitation, not a Tefway change.
- 256/512 px icons are upscaled from the 128 px source; see the Windows document.
- Technician exclusivity is not implemented; see `TEFWAY-REMOTE-AUTH-01`.
