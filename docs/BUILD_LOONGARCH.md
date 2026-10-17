# Building RustDesk on LoongArch Linux

RustDesk uses the Rust target `loongarch64-unknown-linux-gnu` on both old-world
and new-world LoongArch systems. Flutter uses the target name
`linux-loong64` and writes its Linux bundle under
`build/linux/loong64/release/bundle`.

The two Linux ABI families are not binary compatible:

- old-world systems normally use `/lib64/ld.so.1` and the Debian architecture
  name `loongarch64`;
- new-world systems normally use
  `/lib64/ld-linux-loongarch-lp64d.so.1` and may use `loong64` as their Debian
  architecture name.

Build natively on the target ABI with a matching LoongArch-enabled Flutter SDK
and Rust toolchain. Do not copy a new-world Rust library or Flutter Engine into
an old-world package, or the reverse.

## Prerequisites

Install the normal Linux build dependencies used by RustDesk, including GTK 3,
Clang, CMake, Ninja, pkg-config, PulseAudio, GStreamer, libva, libvdpau, and X11
development packages. The LoongArch Flutter SDK must provide debug, profile,
and release `linux-loong64` Engine artifacts.

Confirm the host ABI and Debian architecture before building:

```bash
uname -m
readelf -l /bin/sh | grep interpreter
dpkg --print-architecture
rustc -vV
flutter --version
```

Initialize the submodules and generate the native bridge files. These files are
not tracked in the repository and must match the RustDesk revision being built.
The commands below mirror the versions used by the bridge CI job:

```bash
git submodule update --init --recursive
cargo install cargo-expand --version 1.0.95 --locked
cargo install flutter_rust_bridge_codegen \
  --version 1.80.1 --features uuid --locked

# Required when using Flutter 3.44 or newer.
bash .github/patches/apply_flutter_3.44_source_patches.sh

(cd flutter && flutter pub get)
flutter_rust_bridge_codegen \
  --rust-input ./src/flutter_ffi.rs \
  --dart-output ./flutter/lib/generated_bridge.dart
```

This generates `src/bridge_generated.rs`, `src/bridge_generated.io.rs`,
`flutter/lib/generated_bridge.dart`, and
`flutter/lib/generated_bridge.freezed.dart`. Then build the Rust library:

```bash
pkg-config --modversion libyuv vpx aom
cargo build --locked \
  --features flutter,linux-pkg-config \
  --lib --release
```

Some old-world Loongnix Rust distributions replace crates.io with an
ABI-specific registry. Its `libc`, `linux-raw-sys`, and `rustix` packages use
the same versions but different checksums, so Cargo cannot use the upstream
lockfile unchanged. On those systems, keep a copy of the upstream lockfile,
run the build once without `--locked`, and inspect the result:

```bash
cp Cargo.lock Cargo.lock.upstream
cargo build \
  --features flutter,linux-pkg-config \
  --lib --release
git diff --no-index Cargo.lock.upstream Cargo.lock
```

Only checksum lines for those ABI packages should change. Do not accept
version, source, or dependency changes, and do not commit the translated
lockfile. New-world builds should always use `--locked`.

Generate the Flutter bundle with the Rust bridge files used by the selected
RustDesk revision:

```bash
cd flutter
flutter build linux --release --target-platform linux-loong64
cd ..
```

For the standard Debian packaging path, `build.py` detects the Flutter bundle
directory and reads the package architecture from `dpkg`. `DEB_ARCH` remains
available as an explicit override:

```bash
DEB_ARCH="$(dpkg --print-architecture)" \
  python3 build.py --flutter --skip-cargo
```

`--skip-cargo` reuses the Rust library built above with the
`linux-pkg-config` feature instead of rebuilding it with the default feature
set.

The old `nix` versions still used by transitive dependencies and the mozjpeg
backend are pinned to LoongArch-compatible revisions in `Cargo.toml`.
