# HarmonyOS Flutter bridge

This crate is the Flutter-owned `cdylib` adapter for HarmonyOS. It does not
contain a second RustDesk protocol or session core.

The unified repository keeps these boundaries:

- root `src/` and `libs/` are the only authoritative RustDesk Core;
- the external HAR project builds that Core with `ohos-har` for ArkTS;
- this crate builds the same Core with `ohos-flutter` and re-exports Core's
  `flutter_ffi` API to the generated Flutter Rust Bridge module;
- `flutter/` owns Dart, ArkUI host glue, packaging, permissions, and lifecycle;
- `scripts/build-ohos-ffi.sh` regenerates bindings from the current checkout,
  verifies the exported ABI, and stages the native library into the Flutter
  HarmonyOS module.

The generated `bridge_generated*.rs` and Dart bridge files are build outputs,
not committed sources. Run `scripts/build-ohos-ffi.sh` before invoking Cargo
directly for this adapter.

The two OHOS frontend features are mutually exclusive. Shared protocol,
session, capture, input, audio, clipboard, and codec behavior belongs in Core;
frontend-specific generated bindings and packaging stay under
`native/ohos_bridge` and `flutter/`.
