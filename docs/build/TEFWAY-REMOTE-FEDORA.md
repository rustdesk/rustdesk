# Tefway Remote — Fedora x86_64 (.rpm)

Produces `tefway-remote-master-<versao>-1.x86_64.rpm` and
`tefway-remote-client-<versao>-1.x86_64.rpm`.

Fedora's dependencies differ from Debian's, and `build.py`'s Fedora branch
(`/usr/bin/yum`) packages the Sciter build, not the Flutter one. Do not run
`build.py --flutter` on Fedora; use the commands below, which build the same
Flutter bundle `build.py` builds on its Debian path and package it with
`rpmbuild`.

## Environment

Validated target: Fedora 40/41, x86_64.

| Tool | Version |
| --- | --- |
| Rust | 1.75 (stable, `rustup`) — CI pin |
| Flutter | 3.24.5 (stable) |
| vcpkg | `VCPKG_ROOT` set, baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d` |
| Python | 3.12 |
| rpm-build | provides `rpmbuild` |

```bash
sudo dnf install -y \
  gcc gcc-c++ make pkgconf-pkg-config git curl python3 rpm-build \
  clang clang-devel llvm-devel cmake ninja-build \
  gtk3-devel libxcb-devel libXfixes-devel libxdo-devel \
  alsa-lib-devel pulseaudio-libs-devel libva-devel \
  gstreamer1-devel gstreamer1-plugins-base-devel \
  systemd-devel libdrm-devel wayland-devel libxkbcommon-devel \
  libayatana-appindicator-gtk3-devel
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.75
```

Flutter 3.24.5:

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

If the vcpkg build complains about the system compiler, point it at clang:

```bash
export CC=clang CXX=clang++
```

## One-time: generate the bridge

Same as Debian: `flutter_rust_bridge_codegen` 1.80.1 produces
`flutter/lib/generated_bridge.dart` and `src/bridge_generated*.rs`, which are git
ignored, then apply the `ffi_bindgen_function_refactor` workaround from
`build.py`.

## Build

```bash
python3 res/tefway/package.py --profile master --format rpm --build
python3 res/tefway/package.py --profile client --format rpm --build
```

The script runs `cargo build --locked --features flutter --lib --release`,
`flutter build linux --release`, then `rpmbuild -bb` with
`res/tefway/pkg/tefway-remote.spec.in` rendered per profile and
`--define "tefway_stage <stage dir>"`. No `%prep`/`%build` happens in the spec:
the bundle is already built.

## Output

```
build/tefway/master/rpm/RPMS/x86_64/tefway-remote-master-<versao>-1.x86_64.rpm
build/tefway/client/rpm/RPMS/x86_64/tefway-remote-client-<versao>-1.x86_64.rpm
```

The script prints the exact paths. Installed layout matches the .deb document;
the packages conflict with `rustdesk` and with each other.

## Icon verification

```bash
rpm -qlp build/tefway/client/rpm/RPMS/x86_64/tefway-remote-client-*.rpm | grep icons
sudo dnf install ./tefway-remote-client-*.rpm
gtk-update-icon-cache -f /usr/share/icons/hicolor
```
Check the application menu, the dock, and the window icon. On Fedora with
SELinux enforcing, verify nothing was denied:

```bash
sudo ausearch -m avc -ts recent | grep tefway
```

## Smoke test

```bash
sudo systemctl status tefway-remote-client.service
tefway-remote-client
```

Same acceptance flow as the Debian document: Master connects, Client clicks
accept, decline ends the attempt, reconnect asks again, Master refuses incoming
logins.

## Known issues and limits

- Container note: a Fedora container can build this, but the Flutter Linux
  desktop toolchain needs `gtk3-devel` and a writable `$HOME`; run
  `flutter config --enable-linux-desktop` inside the container first.
- Clipboard and screen capture under Wayland follow upstream RustDesk; X11
  sessions are the reliable path.
- The SELinux context of `/usr/share/<pkg>` is inherited from `/usr/share`, so no
  custom policy is shipped. If your policy is tightened, check `ausearch` above.
- Technician exclusivity is not implemented; see `TEFWAY-REMOTE-AUTH-01`.
