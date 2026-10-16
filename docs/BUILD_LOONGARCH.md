# Building RustDesk on LoongArch Linux

RustDesk consumes a LoongArch-enabled Flutter SDK. The Flutter Framework,
Dart VM, Engine, and Linux embedder ports are maintained separately by the
[Flutter-Dart-loong64 project](https://github.com/Flutter-Dart-loong64).
This repository only integrates those SDK artifacts and carries the Rust and
packaging changes needed by RustDesk.

The Rust target name is `loongarch64-unknown-linux-gnu`. Flutter uses the
custom target name `linux-loong64` and writes its release bundle under
`build/linux/loong64/release/bundle`. The Rust target triple does not identify
which LoongArch userspace ABI is in use.

## ABI identification and isolation

Old-world and new-world LoongArch Linux binaries are not interchangeable.
Do not infer the ABI from `uname -m`: both ABI families can report
`loongarch64`, and a distribution can use `loongarch64` as its package
architecture name while using the new-world ABI.

Inspect the runtime and package metadata instead:

```bash
uname -m
readelf -l /bin/sh | grep 'Requesting program interpreter'
dpkg --print-architecture
rustc -vV
```

Typical values are:

| ABI | Dynamic loader | Debian architecture examples |
| --- | --- | --- |
| Old-world | `/lib64/ld.so.1` | `loongarch64` |
| New-world | `/lib64/ld-linux-loongarch-lp64d.so.1` | `loong64` or `loongarch64`, depending on the distribution |

Use separate source, Cargo target, Flutter cache, sysroot, staging, and output
directories for each ABI. The compiler, Rust standard library, native codec
libraries, Flutter Engine, generated runner, plugins, and final package must
all target the same ABI. Never copy a new-world `librustdesk.so`, runner, or
Flutter Engine into an old-world bundle, or the reverse.

`build.py` selects the Debian architecture in this order:

1. a non-empty `DEB_ARCH` value;
2. `dpkg --print-architecture`;
3. a bounded machine-name fallback for non-LoongArch hosts.

If `dpkg` is unavailable on LoongArch, set `DEB_ARCH` from the target
distribution's package metadata. The build script deliberately refuses to
guess it from `loong64` or `loongarch64`.

## Flutter SDK revisions

Official Flutter SDK archives do not currently provide the custom
`linux-loong64` target. Use an ABI-matched community SDK or build the SDK from
the Flutter-Dart-loong64 source forks.

The following published SDKs have reproducible release metadata:

| ABI | Release | Framework | Engine | Dart | Release repository |
| --- | --- | --- | --- | --- | --- |
| Old-world | `v3.46.0-1.0.pre-341` | `857f1349584c839798db5b9f8e47ceb0557b88d6` | `1215a75c452ad31502e146f8e621c70b3f2b3689` | `bede5b5178ecb4e0307a9ad81a8026abcf34df24` | [`flutter-loongarch64-releases`](https://github.com/Flutter-Dart-loong64/flutter-loongarch64-releases/releases/tag/v3.46.0-1.0.pre-341) |
| New-world | `v3.46.0-1.0.pre-327` | `69c87127a40b5c0d735611f9026c3b16a2c02369` | `69c87127a40b5c0d735611f9026c3b16a2c02369` | `2ea45c8966a8f1769ce4313a80cd829f46144583` | [`flutter-loong64-releases`](https://github.com/Flutter-Dart-loong64/flutter-loong64-releases/releases/tag/v3.46.0-1.0.pre-327) |

The old-world release was built and exercised on UOS 20-class hardware. The
new-world release was built for UOS/deepin 25-class LP64D systems. Both release
projects publish checksums and their own source-build records. Before merging
this RustDesk change, the exact SDK tag used for the final old-world and
new-world RustDesk acceptance packages must be recorded in the PR test report;
that final package-level record is currently **TODO**.

Example old-world installation:

```bash
wget https://github.com/Flutter-Dart-loong64/flutter-loongarch64-releases/releases/download/v3.46.0-1.0.pre-341/flutter-3.46.0-1.0.pre-341-loongarch64-oldworld-uos20.tar.gz
wget https://github.com/Flutter-Dart-loong64/flutter-loongarch64-releases/releases/download/v3.46.0-1.0.pre-341/flutter-3.46.0-1.0.pre-341-loongarch64-oldworld-uos20.tar.gz.sha256
sha256sum -c flutter-3.46.0-1.0.pre-341-loongarch64-oldworld-uos20.tar.gz.sha256
tar -xzf flutter-3.46.0-1.0.pre-341-loongarch64-oldworld-uos20.tar.gz
export FLUTTER_ROOT="$PWD/flutter"
export PATH="$FLUTTER_ROOT/bin:$PATH"
```

Example new-world installation:

```bash
wget https://github.com/Flutter-Dart-loong64/flutter-loong64-releases/releases/download/v3.46.0-1.0.pre-327/flutter-sdk-linux-loong64-3.46.0-1.0.pre-327-69c87127a40b.tar.xz
wget https://github.com/Flutter-Dart-loong64/flutter-loong64-releases/releases/download/v3.46.0-1.0.pre-327/SHA256SUMS
sha256sum -c SHA256SUMS --ignore-missing
tar -xf flutter-sdk-linux-loong64-3.46.0-1.0.pre-327-69c87127a40b.tar.xz
export FLUTTER_ROOT="$PWD/flutter"
export PATH="$FLUTTER_ROOT/bin:$PATH"
```

Verify the selected SDK rather than relying on its archive name:

```bash
flutter --version --no-version-check
flutter doctor -v
flutter config --enable-linux-desktop
flutter config --enable-loong64

tmp_app="$(mktemp -d)/loongarch_sdk_probe"
flutter create "$tmp_app"
cd "$tmp_app"
flutter build linux --release --target-platform linux-loong64
file build/linux/loong64/release/bundle/*
readelf -l build/linux/loong64/release/bundle/loongarch_sdk_probe \
  | grep 'Requesting program interpreter'
```

The SDK cache must contain matching debug, profile, and release
`linux-loong64` Engine artifacts. At minimum, the release build requires a
matching runner/embedding library, `libflutter_linux_gtk.so`, `gen_snapshot`,
`icudtl.dat`, and the Linux embedder headers and CMake metadata. Do not combine
`gen_snapshot`, `libapp.so`, or an Engine library from different Dart or Engine
revisions.

## RustDesk prerequisites

Install the normal Linux dependencies used by RustDesk, including GTK 3,
Clang, a C/C++ compiler, CMake, Ninja, pkg-config, PulseAudio, GStreamer,
libva, libvdpau, and X11 development files. Package names differ by
distribution. Old-world builds also need an old-world Rust toolchain and all
native codec libraries built for the old-world ABI; the same rule applies to
new-world builds.

Initialize the source tree:

```bash
git submodule update --init --recursive
```

RustDesk uses flutter-rust-bridge 1.80.1. Generate bridge files from the exact
RustDesk revision being built:

```bash
cargo install cargo-expand --version 1.0.95 --locked
cargo install flutter_rust_bridge_codegen \
  --version 1.80.1 --features uuid --locked

# Required by this RustDesk revision when Flutter 3.44 or newer is used.
bash .github/patches/apply_flutter_3.44_source_patches.sh

(cd flutter && flutter pub get)
flutter_rust_bridge_codegen \
  --rust-input ./src/flutter_ffi.rs \
  --dart-output ./flutter/lib/generated_bridge.dart
```

This generates `src/bridge_generated.rs`, `src/bridge_generated.io.rs`,
`flutter/lib/generated_bridge.dart`, and
`flutter/lib/generated_bridge.freezed.dart`. Generated files, the native
library, plugins, Flutter runner, and Engine must all come from the same
RustDesk revision and ABI.

Confirm the selected system codec libraries, then build the Rust library:

```bash
pkg-config --modversion libyuv vpx aom
cargo build --locked \
  --features flutter,linux-pkg-config \
  --lib --release
```

Some old-world Loongnix Rust distributions replace crates.io with an
ABI-specific registry. Their `libc`, `linux-raw-sys`, and `rustix` packages can
have the same versions but different checksums. In that environment, preserve
the upstream lockfile and audit the translated result:

```bash
cp Cargo.lock Cargo.lock.upstream
cargo build \
  --features flutter,linux-pkg-config \
  --lib --release
git diff --no-index Cargo.lock.upstream Cargo.lock
```

Only checksum fields for the ABI-specific registry packages may differ. Do not
accept version, source, or dependency changes, and do not commit the translated
lockfile. Builds using crates.io must use `--locked`.

Build the Flutter release bundle:

```bash
cd flutter
flutter build linux --release --target-platform linux-loong64
cd ..
```

Package the already-built Rust library and Flutter bundle without rebuilding
Rust with a different feature set:

```bash
DEB_ARCH="$(dpkg --print-architecture)" \
  python3 build.py --flutter --skip-cargo
```

`--skip-cargo` is required in this flow because the preceding native build used
`linux-pkg-config`. Omitting it can rebuild against a different dependency
source.

Audit the result before installation:

```bash
dpkg-deb -f rustdesk-*.deb Package Version Architecture Depends
dpkg-deb -x rustdesk-*.deb package-root
find package-root -type f -exec file {} + | grep -E 'ELF|LoongArch'
find package-root -type f -exec sh -c '
  file "$1" | grep -q ELF || exit 0
  readelf -l "$1" | grep "Requesting program interpreter" || true
  ldd -r "$1"
' sh {} \;
```

## Dependency override audit

Cargo patches are resolved for the complete workspace dependency graph. They
cannot be made LoongArch-only by placing `[patch.crates-io]` under a target
table. The following overrides therefore change source provenance on every
platform, even where the patched code is selected only by a LoongArch `cfg`.

Audit date: 2026-11-03. The dependency paths were confirmed with
`cargo metadata --locked` and `cargo tree --locked -i`.

| Crate | Current source | LoongArch reason | Upstream status | Other-platform impact | Decision and evidence |
| --- | --- | --- | --- | --- | --- |
| `nix 0.23.2` | `Flutter-Dart-loong64/nix@03d60be7` | `evdev 0.11.5` requires this old release, whose Linux ioctl constants omit `target_arch = "loongarch64"`. | Upstream added LoongArch in `2589f437`, released in `nix 0.27.0`; it was not backported to 0.23.2. | The Git source is used globally, but the fork differs from the matching release by one LoongArch-only cfg line. | Keep until the RustDesk `evdev` fork can move to a LoongArch-aware `nix`. Evidence: `cargo tree -i nix@0.23.2` and [fork commit](https://github.com/Flutter-Dart-loong64/nix/commit/03d60be7e04b8b41175d78f0c5e41a74ae23a230). |
| `nix 0.25.1` | `Flutter-Dart-loong64/nix@cc26a733` | `portable-pty 0.8.1` requires the old ioctl implementation. | Same upstream support boundary as above. | Global source replacement; the diff is one LoongArch-only cfg line. | Keep until the RustDesk `portable-pty` fork upgrades `nix`. Evidence: `cargo tree -i nix@0.25.1` and [fork commit](https://github.com/Flutter-Dart-loong64/nix/commit/cc26a733b0314a3e9a5db9ec180386893d22e772). |
| `nix 0.26.4` | `Flutter-Dart-loong64/nix@87b10c3a` | `webrtc-util 0.11.0` and `zbus 3.15.2` resolve this version, which predates upstream LoongArch support. | Same upstream support boundary as above. | Global source replacement; the diff is one LoongArch-only cfg line. | Keep until all locked users can move to `nix >= 0.27`. Evidence: `cargo tree -i nix@0.26.4` and [fork commit](https://github.com/Flutter-Dart-loong64/nix/commit/87b10c3ae8ceb18ea7fe890bdffab955ebae1e6e). |
| `mozjpeg 0.10.13` | `Flutter-Dart-loong64/mozjpeg-rust@f9970f2` | The crates.io 0.10.13 manifest requests `mozjpeg-sys 2.2.3`; the LoongArch backend is carried by the forked `2.2.4-beta.1`, so the wrapper must permit that exact prerelease. | Upstream `mozjpeg 0.10.13` still requests `mozjpeg-sys 2.2.3`. | The fork replaces the wrapper source globally. Its only code change is the dependency version, but it changes the selected native JPEG implementation on all targets. | Keep paired with the sys fork. Remove it when an upstream wrapper release accepts a mozjpeg-sys release containing the LoongArch backend. Evidence: `cargo tree -i mozjpeg@0.10.13` and [fork commit](https://github.com/Flutter-Dart-loong64/mozjpeg-rust/commit/f9970f2ba88e136db0f5532983c14573ecb0b037). |
| `mozjpeg-sys 2.2.4-beta.1` | `Flutter-Dart-loong64/mozjpeg-sys@11cfd420` with vendor `Flutter-Dart-loong64/mozjpeg@0856be24` | Adds LSX/LASX dispatch and scalar fallback interfaces required on compilers without modern SIMD flags. | `kornelski/mozjpeg-sys@662cd766` and `mozilla/mozjpeg@08265790` do not contain an equivalent LoongArch backend. | Highest-risk override: it changes the vendored MozJPEG revision and build script globally, including 12/16-bit object generation, while the SIMD source itself is LoongArch-only. | Retain for LoongArch buildability and run non-LoongArch Cargo CI on every update. Follow-up is to upstream the vendor backend and build integration, then return to crates.io. Evidence: `cargo tree -i mozjpeg-sys@2.2.4-beta.1`, [sys fork](https://github.com/Flutter-Dart-loong64/mozjpeg-sys/commit/11cfd4201d9b1e0d6f9de2fc0ba5204065ce9225), and [vendor fork](https://github.com/Flutter-Dart-loong64/mozjpeg/commit/0856be246826dd68c9adef74a3ddf24e34648ba1). |

The `nix` overrides are small and target-gated but still require x86_64 and
ARM64 lockfile/build regression coverage because their source is global. The
MozJPEG pair requires stronger review: replacing it with crates.io today drops
the LoongArch backend, while retaining it means RustDesk assumes maintenance of
a globally selected native codec fork.

The dependency audit found two non-LoongArch regressions in the previous
`mozjpeg-sys@5489d3b8` vendor/build integration:

- x86/x86_64 builds without a usable NASM refer to the removed
  `vendor/jsimd_none.c`; with NASM enabled, `WITH_SIMD` is not visible while
  compiling `jpeg_nbits.c`, which duplicates the table exported by the x86
  assembly object;
- enabling the optional arithmetic encoder together with the fork's
  multi-precision objects duplicates `quantize_trellis_arith` between the
  8-bit and 12-bit objects.

The pinned `11cfd420` revision fixes the first item and was host-tested with
and without NASM. RustDesk's resolved graph uses the default `mozjpeg-sys`
features and does not enable the arithmetic encoder. The second item remains a
fork issue and must be fixed before advertising the fork's arithmetic or
all-features configuration as supported.

## Texture rendering compatibility

On Linux LoongArch, RustDesk defaults to the pixel-buffer path. Texture
rendering remains available as an explicit opt-in. The option behavior is:

| Platform | `OPTION_TEXTURE_RENDER=Y` | `N` | Unset or another value |
| --- | --- | --- | --- |
| Linux LoongArch with Flutter | texture | pixel buffer | pixel buffer |
| Other Linux architectures with Flutter | texture | pixel buffer | texture, preserving existing behavior |
| Any Linux build without the Flutter feature | disabled | disabled | disabled |

This fallback is based on a reproduced compatibility failure, not an assertion
that all LoongArch GPUs lack texture support. On the tested UOS 20/UOS 25
LoongGPU systems using Mesa 18.3.6 and OpenGL 2.1, the VP9 decoder, connection,
and input remained active with texture rendering enabled, but the remote image
area was black. Setting the option to `N` made the same session visible;
setting it back to `Y` reproduced the black image. The failure boundary is the
Flutter Linux texture upload/presentation path on that tested stack. The exact
failing Engine, driver, or plugin API remains to be isolated.

Acceptance on a new LoongArch graphics stack must exercise both settings and
record image correctness, stability, CPU use, and latency. No performance
result should be inferred from the architecture mapping unit tests.

## LoongArch GCC warning exception

The Linux CMake file keeps `-Werror` and adds only
`-Wno-error=maybe-uninitialized` when both conditions hold:

- the compiler is GNU;
- `CMAKE_SYSTEM_PROCESSOR` is `loongarch64` or `loong64`.

It does not affect Clang or other architectures. The exception was introduced
for generated GLib cleanup code in LoongArch GCC builds. The original warning
log, exact GCC version, target, and generated source location are not preserved
in this PR, so narrowing the option to one source file is still a manual
follow-up. Capture those four values during the next old-world and new-world
native build before changing the scope; do not disable `-Werror` globally.

## Verification status

The architecture helper tests and metadata checks are host-independent:

```bash
python3 res/test_build_arch.py
python3 -m py_compile build.py res/test_build_arch.py
cargo metadata --locked --no-deps
```

On 2026-11-03 all three commands above passed at the audited PR head; the
architecture suite contained 11 tests. Full x86_64 `cargo check --locked`
advanced through the patched `nix` crates but could not complete on the audit
host because the GStreamer development package and `gstreamer-1.0.pc` were not
installed. That is an unexecuted full-build check, not a successful build.

The pinned `mozjpeg-sys` revision was tested separately on x86_64: its
default NASM path passed eight tests, its no-default-features scalar path
passed six tests, and a default build with NASM unavailable completed using
the scalar fallback. Its `--all-features` test still fails on the arithmetic
multi-precision duplicate described above.

Historical native validation for the dependency and rendering changes includes
an old-world UOS 20 RustDesk 1.5.0 build/install/GUI run and an AOSC new-world
RustDesk 1.5.0 build, package audit, dynamic-link check, and command-line
startup. The LoongGPU texture comparison described above was performed on real
UOS 20/UOS 25 systems. Those results establish the compatibility reason for
the current defaults, but they do not replace acceptance testing of the current
PR head.

Before merge, run the following separately on old-world and new-world systems:

- locked Rust release build with the intended native codec source;
- Flutter release bundle and Debian package build;
- `file`, `readelf -l`, `ldd -r`, and `dpkg-deb -f` ABI checks;
- installation, service startup, controller and controlled modes;
- keyboard/mouse input and file transfer;
- pixel-buffer and texture-rendering sessions.

CI architecture mapping tests do not substitute for these native tests.
