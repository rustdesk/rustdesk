# Building Cyberdriver

Cyberdriver is a Rust application with a Flutter desktop UI, forked from RustDesk. The shipped artifact is a Windows x64 MSI; other platforms build but are not part of the release pipeline.

The authoritative build is [`.github/workflows/release-windows-msi.yml`](.github/workflows/release-windows-msi.yml). If anything here drifts, the workflow wins.

## Toolchain

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.75 | Pinned. 1.78+ has an i128 ABI change that breaks the Sciter build. |
| Flutter | 3.24.5 | Requires the RustDesk custom engine, see below. |
| Python | 3.12 | Runs `build.py` and the MSI preprocessor. |
| LLVM/Clang | 15.0.6 | Needed for bindgen. |
| vcpkg | commit `120deac3` | Pinned in CI for reproducible native deps. |

```bash
git clone --recurse-submodules https://github.com/cyberdesk-hq/cyberdriver-new
cd cyberdriver-new
```

Submodules are required. If you cloned without them, run `git submodule update --init --recursive`.

## Native dependencies

`libs/scrap` and `magnum-opus` need `libvpx`, `libyuv`, `opus`, and `aom`. `libyuv` is not in Homebrew or most distro repos, so use vcpkg on every platform:

```bash
git clone https://github.com/microsoft/vcpkg ~/vcpkg
cd ~/vcpkg && git checkout 2023.04.15 && ./bootstrap-vcpkg.sh
./vcpkg install libvpx libyuv opus aom
export VCPKG_ROOT=~/vcpkg
```

On Windows use the static triplet instead:

```powershell
vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
```

Add `VCPKG_ROOT` to your shell profile so it persists.

Without these, `cargo check` still compiles all Rust code before failing at the `scrap` and `magnum-opus` build scripts. That partial check is a useful syntax test — but any error mentioning our own sources is a real bug, not a missing dependency.

### Linux system packages

**Debian / Ubuntu**

```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev \
        libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
        libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpam0g-dev
```

**Fedora / CentOS**

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel pam-devel
```

**openSUSE Tumbleweed**

```sh
sudo zypper install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libXfixes-devel cmake alsa-lib-devel gstreamer-devel gstreamer-plugins-base-devel xdotool-devel pam-devel
```

**Arch / Manjaro**

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

On Fedora, vcpkg's `libvpx` builds without `-fPIC` and fails to link. Patch and rebuild it:

```sh
cd vcpkg/buildtrees/libvpx/src/*
./configure
sed -i 's/CFLAGS+=-I/CFLAGS+=-fPIC -I/g' Makefile
sed -i 's/CXXFLAGS+=-I/CXXFLAGS+=-fPIC -I/g' Makefile
make
cp libvpx.a $HOME/vcpkg/installed/x64-linux/lib/
```

## Local development build

The Cargo package is still named `rustdesk` upstream, so the binary lands at `target/debug/rustdesk`. It is only renamed to `Cyberdriver.exe` during MSI packaging.

The `cyberdesk` feature is on by default, so a plain build already includes the tunnel and branding overlay.

```bash
cargo run                      # legacy Sciter UI, needs the Sciter library
python3 build.py --flutter     # Flutter desktop UI
cargo check                    # fast syntax check
cargo test                     # Rust tests
```

To point a local build at the Cyberdesk development environment, build with `--features cyberdesk-dev`, or pass `--env dev` at runtime.

### Sciter (legacy UI only)

`cargo run` without Flutter needs the Sciter dynamic library next to the binary. Flutter builds do not.

```sh
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
```

macOS uses [`libsciter.dylib`](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib) and Windows uses [`sciter.dll`](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll).

### Flutter engine

Flutter builds require RustDesk's custom engine, which replaces the stock one in the Flutter cache. CI downloads `windows-x64-release.zip` from the [rustdesk/engine](https://github.com/rustdesk/engine/releases/tag/main) release, verifies its SHA256, and copies it over `bin/cache/artifacts/engine/windows-x64-release` in the Flutter SDK. CI also applies `.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff` to the SDK. Both steps are in the workflow.

## Windows MSI (release build)

This is the artifact users install. Requires the full toolchain above plus MSBuild, NuGet, and the WiX toolset.

```powershell
python .\build.py --portable --hwcodec --flutter --vram --skip-portable-pack
```

Then stage the bundle and package it:

```powershell
# Stage the Flutter release output and rename the executable
New-Item -ItemType Directory -Force msi-dist
Copy-Item -Recurse flutter\build\windows\x64\runner\Release\* msi-dist\
Rename-Item msi-dist\rustdesk.exe Cyberdriver.exe

# Build the MSI
Push-Location res\msi
python preprocess.py --arp -d ../../msi-dist --app-name Cyberdriver -m "Cyberdesk, Inc" -v <version>
nuget restore msi.sln
msbuild msi.sln -p:Configuration=Release -p:Platform=x64 /p:TargetVersion=Windows10
Pop-Location
```

The MSI is written to `res\msi\Package\bin` and released as `Cyberdriver-<version>-windows-x64.msi`. Release builds are unsigned, so Windows SmartScreen will warn on install.

The version comes from `[package].version` in `Cargo.toml` and must stay in sync with `flutter/pubspec.yaml`. Install-time behavior such as `INSTALL_AS_SERVICE`, `APIKEY`, and `REGISTER_NOW` is defined in [`res/msi/Package/Package.wxs`](res/msi/Package/Package.wxs) and documented in [`branding/msi-flags.md`](branding/msi-flags.md).

## Docker builder (Linux)

Builds Linux binaries without installing the toolchain on your host.

```sh
docker build -t cyberdriver-builder .
```

Then, for each build:

```sh
docker run --rm -it \
  -v $PWD:/home/user/rustdesk \
  -v cyberdriver-git-cache:/home/user/.cargo/git \
  -v cyberdriver-registry-cache:/home/user/.cargo/registry \
  -e PUID="$(id -u)" -e PGID="$(id -g)" \
  cyberdriver-builder
```

The container mount path is `/home/user/rustdesk` because of the upstream image layout — keep it as-is. Append extra cargo arguments such as `--release` to the end of the command. The first build is slow until dependencies are cached, and the resulting binary appears in `target/` on your host. Run these from the repository root, and note that `cargo install` and `cargo run` are not usable through this path since they would act inside the container.

## Feature flags

| Feature | Effect |
| --- | --- |
| `cyberdesk` | Cyberdesk tunnel, CLI, and branding. **On by default.** |
| `cyberdesk-dev` | Targets the Cyberdesk development environment. |
| `flutter` | Flutter UI instead of Sciter. |
| `hwcodec` | Hardware video encode/decode. |
| `vram` | VRAM optimization, Windows only. |
| `screencapturekit` | macOS ScreenCaptureKit. |
| `unix-file-copy-paste` | Unix file clipboard support. |

## See also

- [`AGENTS.md`](AGENTS.md) — repository layout and architecture.
- [`branding/README.md`](branding/README.md) — how Cyberdesk branding is applied at build time.
- [`branding/build_prerequisites.md`](branding/build_prerequisites.md) — notes on verifying the branding overlay compiles.
- [`docs/headless-install.md`](docs/headless-install.md) — deploying built artifacts to golden images.
