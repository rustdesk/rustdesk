#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

source "$repo_root/scripts/ohos-env.sh"

for dependency_script in \
  build-libvpx-ohos.sh \
  build-libaom-ohos.sh \
  build-libyuv-ohos.sh \
  build-libopus-ohos.sh; do
  bash "$repo_root/scripts/$dependency_script"
done

cargo_config="$repo_root/.cargo/ohos.toml"
if [[ ! -f "$cargo_config" ]]; then
  echo "Missing OHOS Cargo configuration: $cargo_config" >&2
  exit 2
fi

cargo --config "$cargo_config" build \
  --target aarch64-unknown-linux-ohos \
  --release \
  --locked \
  --lib \
  --features flutter \
  "$@"

artifact="$repo_root/target/aarch64-unknown-linux-ohos/release/liblibrustdesk.so"
if [[ ! -s "$artifact" ]]; then
  echo "OHOS FFI build did not produce $artifact" >&2
  exit 1
fi

printf 'Done: %s\n' "$artifact"
