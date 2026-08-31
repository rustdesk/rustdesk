#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
core_root="$(cd "${RUSTDESK_CORE_ROOT:-$repo_root}" && pwd)"
bridge_manifest="$repo_root/native/ohos_bridge/Cargo.toml"
cd "$repo_root"
export PATH="$HOME/.cargo/bin:$PATH"

source "$repo_root/scripts/ohos-env.sh"

if ! command -v flutter >/dev/null 2>&1 && [[ -x "$HOME/flutter-ohos/bin/flutter" ]]; then
  export PATH="$HOME/flutter-ohos/bin:$PATH"
fi
if ! command -v flutter >/dev/null 2>&1; then
  echo "Flutter-OH is not available on PATH" >&2
  exit 2
fi

frb_codegen="${FLUTTER_RUST_BRIDGE_CODEGEN:-$HOME/.cargo/bin/flutter_rust_bridge_codegen}"
if [[ ! -x "$frb_codegen" ]]; then
  echo "Flutter Rust bridge generator is unavailable: $frb_codegen" >&2
  exit 2
fi
frb_version="$($frb_codegen --version)"
if [[ "$frb_version" != "flutter_rust_bridge_codegen 1.80.1" ]]; then
  echo "Expected flutter_rust_bridge_codegen 1.80.1, got: $frb_version" >&2
  exit 2
fi
frb_llvm_opts="--sysroot=$OHOS_NDK_HOME/native/sysroot -isystem $OHOS_NDK_HOME/native/sysroot/usr/include/aarch64-linux-ohos -DWireSyncReturn=void*"
rust_output="$repo_root/native/ohos_bridge/src/bridge_generated.rs"
rust_tmp_dir="$(mktemp -d "$repo_root/native/ohos_bridge/.frb.XXXXXX")"
rust_output_tmp="$rust_tmp_dir/bridge_generated.rs"
rust_io_output_tmp="$rust_tmp_dir/bridge_generated.io.rs"
dart_output="$repo_root/flutter/lib/generated_bridge.dart"
inline_stub="$core_root/src/ui/inline.rs"
created_inline_stub=0
if [[ ! -e "$inline_stub" ]]; then
  # FRB 1.80.1 walks cfg-disabled modules while building its source graph.
  # The real inline module is generated only for desktop inline builds, so an
  # empty temporary source file is sufficient for the OHOS Flutter API scan.
  mkdir -p "$(dirname "$inline_stub")"
  printf '%s\n' '// Temporary FRB source-graph stub; removed by build-ohos-ffi.sh.' > "$inline_stub"
  created_inline_stub=1
fi
cleanup() {
  rm -rf "$rust_tmp_dir"
  if [[ "$created_inline_stub" == 1 ]]; then
    rm -f "$inline_stub"
  fi
}
trap cleanup EXIT

"$frb_codegen" \
  --skip-deps-check \
  --llvm-path "$OHOS_NDK_HOME/native/llvm" \
  --llvm-compiler-opts="$frb_llvm_opts" \
  --rust-input "$core_root/src/flutter_ffi.rs" \
  --rust-crate-dir "$core_root" \
  --rust-output "$rust_output_tmp" \
  --dart-output "$dart_output" \
  --skip-add-mod-to-lib

python3 "$repo_root/scripts/patch-ohos-frb-dart.py" \
  "$dart_output"
dart format "$dart_output"
dart analyze "$dart_output"

python3 "$repo_root/scripts/strip-ohos-frb-core-event-impl.py" \
  "$rust_output_tmp"
mv "$rust_output_tmp" "$rust_output"
mv "$rust_io_output_tmp" "${rust_output%.rs}.io.rs"

for dependency_script in \
  build-libvpx-ohos.sh \
  build-libaom-ohos.sh \
  build-libyuv-ohos.sh \
  build-libopus-ohos.sh; do
  bash "$repo_root/scripts/$dependency_script"
done

cargo build \
  --manifest-path "$bridge_manifest" \
  --target aarch64-unknown-linux-ohos \
  --release \
  --locked \
  --lib \
  "$@"

cargo_target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$cargo_target_dir" != /* ]]; then
  cargo_target_dir="$repo_root/$cargo_target_dir"
fi
artifact="$cargo_target_dir/aarch64-unknown-linux-ohos/release/librustdesk_ohos_flutter_bridge.so"
if [[ ! -s "$artifact" ]]; then
  echo "OHOS FFI build did not produce $artifact" >&2
  exit 1
fi
llvm_nm="$OHOS_NDK_HOME/native/llvm/bin/llvm-nm"
exported_symbols="$($llvm_nm -D --defined-only "$artifact")"
for symbol in \
  free_WireSyncReturn \
  init_frb_dart_api_dl \
  store_dart_post_cobject \
  get_dart_object \
  drop_dart_object \
  new_dart_opaque \
  wire_main_configure_ohos_host_display \
  wire_main_set_ohos_host_clipboard_enabled \
  wire_main_update_ohos_host_clipboard_text \
  wire_main_take_ohos_host_clipboard_text \
  wire_main_start_ohos_host \
  wire_main_stop_ohos_host \
  wire_main_ohos_host_is_started \
  wire_cm_close_connection_window; do
  if ! grep -qE " [A-Za-z] ${symbol}$" <<<"$exported_symbols"; then
    echo "OHOS FFI artifact is missing exported symbol: $symbol" >&2
    exit 1
  fi
done

flutter_lib_dir="$repo_root/flutter/ohos/entry/libs/arm64-v8a"
mkdir -p "$flutter_lib_dir"
cp "$artifact" "$flutter_lib_dir/liblibrustdesk.so"
libcxx="$OHOS_NDK_HOME/native/llvm/lib/aarch64-linux-ohos/libc++_shared.so"
if [[ ! -s "$libcxx" ]]; then
  echo "Missing OHOS C++ runtime: $libcxx" >&2
  exit 1
fi
cp "$libcxx" "$flutter_lib_dir/libc++_shared.so"

core_sha="$(git -C "$core_root" rev-parse HEAD)"
if [[ -n "$(git -C "$core_root" status --porcelain)" ]]; then
  core_state=dirty
else
  core_state=clean
fi
printf 'Done: %s\n' "$artifact"
printf 'Flutter OHOS library: %s\n' "$flutter_lib_dir/liblibrustdesk.so"
printf 'Authoritative Core: %s (%s)\n' "$core_sha" "$core_state"
