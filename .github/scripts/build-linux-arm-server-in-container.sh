#!/usr/bin/env bash
set -euo pipefail

RUST_VERSION="${RUST_VERSION:-1.75}"
RUST_TOOLCHAIN_VERSION="${RUST_TOOLCHAIN_VERSION:-1.75.0}"
FLUTTER_VERSION="${FLUTTER_VERSION:-3.24.5}"
BRIDGE_FLUTTER_VERSION="${BRIDGE_FLUTTER_VERSION:-3.22.3}"
FLUTTER_RUST_BRIDGE_VERSION="${FLUTTER_RUST_BRIDGE_VERSION:-1.80.1}"
CARGO_EXPAND_VERSION="${CARGO_EXPAND_VERSION:-1.0.95}"
TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
ARCH="${ARCH:-aarch64}"
DEB_ARCH="${DEB_ARCH:-arm64}"
VCPKG_BASE_TRIPLET="${VCPKG_BASE_TRIPLET:-arm64-linux}"
VCPKG_TRIPLET="${VCPKG_TRIPLET:-arm64-linux-clang}"
VCPKG_COMMIT_ID="${VCPKG_COMMIT_ID:-120deac3062162151622ca4860575a33844ba10b}"
VCPKG_ROOT="${VCPKG_ROOT:-/opt/artifacts/vcpkg}"
VCPKG_OVERLAY_TRIPLETS="${VCPKG_OVERLAY_TRIPLETS:-/opt/artifacts/vcpkg-overlay-triplets}"
BRIDGE_ARTIFACT_DIR="${BRIDGE_ARTIFACT_DIR:-}"
GENERATE_BRIDGE="${GENERATE_BRIDGE:-1}"
JOBS="${JOBS:---jobs 3}"

if [[ "$(uname -m)" != "aarch64" && "$(uname -m)" != "arm64" ]]; then
  echo "This script is intended to run inside an aarch64/arm64 Linux container." >&2
  exit 1
fi

if [[ ! -f Cargo.toml || ! -d flutter ]]; then
  echo "Run this script from the RustDesk repository root." >&2
  exit 1
fi
REPO_ROOT="$(pwd)"

echo "Build container:"
cat /etc/os-release || true
echo "Architecture: $(uname -m)"
echo "Rust: ${RUST_VERSION} (${RUST_TOOLCHAIN_VERSION})"
echo "Flutter: ${FLUTTER_VERSION}"
echo "Target: ${TARGET}"
echo "vcpkg triplet: ${VCPKG_TRIPLET}"

if [[ ${EUID} -eq 0 ]]; then
  APT_GET="apt-get"
else
  APT_GET="sudo apt-get"
fi

${APT_GET} update -y
${APT_GET} install -y \
  build-essential \
  ca-certificates \
  clang \
  cmake \
  curl \
  gcc \
  git \
  g++ \
  libayatana-appindicator3-dev \
  libasound2-dev \
  libclang-10-dev \
  libclang-dev \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  libgtk-3-dev \
  libpam0g-dev \
  libpulse-dev \
  libva-dev \
  libxcb-randr0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxdo-dev \
  libxfixes-dev \
  llvm-10-dev \
  llvm-dev \
  nasm \
  ninja-build \
  pkg-config \
  tree \
  python3 \
  zip \
  unzip \
  tar \
  wget \
  xz-utils \
  libssl-dev
if ! ${APT_GET} install -y clang-10; then
  echo "clang-10 is not available; falling back to the default clang package."
fi
${APT_GET} remove -y libopus-dev || true

git config --global --add safe.directory "*"

if [[ ! -x "${VCPKG_ROOT}/vcpkg" ]]; then
  mkdir -p "$(dirname "${VCPKG_ROOT}")"
  git clone https://github.com/microsoft/vcpkg.git "${VCPKG_ROOT}"
  pushd "${VCPKG_ROOT}"
    git checkout "${VCPKG_COMMIT_ID}"
    ./bootstrap-vcpkg.sh
  popd
fi

CLANG_CC="$(command -v clang-10 || command -v clang)"
CLANG_CXX="$(command -v clang++-10 || command -v clang++)"
VCPKG_BASE_TRIPLET_FILE="${VCPKG_ROOT}/triplets/${VCPKG_BASE_TRIPLET}.cmake"
if [[ ! -f "${VCPKG_BASE_TRIPLET_FILE}" ]]; then
  VCPKG_BASE_TRIPLET_FILE="${VCPKG_ROOT}/triplets/community/${VCPKG_BASE_TRIPLET}.cmake"
fi
if [[ ! -f "${VCPKG_BASE_TRIPLET_FILE}" ]]; then
  echo "Could not find vcpkg base triplet: ${VCPKG_BASE_TRIPLET}" >&2
  exit 1
fi
mkdir -p "${VCPKG_OVERLAY_TRIPLETS}"
cp "${VCPKG_BASE_TRIPLET_FILE}" "${VCPKG_OVERLAY_TRIPLETS}/${VCPKG_TRIPLET}.cmake"
cat >> "${VCPKG_OVERLAY_TRIPLETS}/${VCPKG_TRIPLET}.cmake" <<EOF
set(VCPKG_CHAINLOAD_TOOLCHAIN_FILE "\${CMAKE_CURRENT_LIST_DIR}/${VCPKG_TRIPLET}-toolchain.cmake")
EOF
cat > "${VCPKG_OVERLAY_TRIPLETS}/${VCPKG_TRIPLET}-toolchain.cmake" <<EOF
set(CMAKE_C_COMPILER "${CLANG_CC}" CACHE FILEPATH "")
set(CMAKE_CXX_COMPILER "${CLANG_CXX}" CACHE FILEPATH "")
EOF

export VCPKG_ROOT
export VCPKG_OVERLAY_TRIPLETS
export VCPKG_DEFAULT_TRIPLET="${VCPKG_TRIPLET}"
export VCPKG_DEFAULT_HOST_TRIPLET="${VCPKG_TRIPLET}"
export VCPKG_BINARY_SOURCES="${VCPKG_BINARY_SOURCES:-clear}"
export CC="${CLANG_CC}"
export CXX="${CLANG_CXX}"

if ! "${VCPKG_ROOT}/vcpkg" install --triplet "${VCPKG_TRIPLET}" --overlay-triplets="${VCPKG_OVERLAY_TRIPLETS}" --x-install-root="${VCPKG_ROOT}/installed"; then
  find "${VCPKG_ROOT}" -name "*.log" -print -exec sh -c 'echo "======"; cat "$1"; echo "======"' sh {} \;
  exit 1
fi
head -n 100 "${VCPKG_ROOT}/buildtrees/ffmpeg/build-${VCPKG_TRIPLET}-rel-out.log" || true

if ! command -v cargo >/dev/null 2>&1 || [[ "$(cargo --version | awk '{print $2}')" != "${RUST_TOOLCHAIN_VERSION}" ]]; then
  pushd /opt
    wget -O rust.tar.gz "https://static.rust-lang.org/dist/rust-${RUST_TOOLCHAIN_VERSION}-${TARGET}.tar.gz"
    tar -zxf rust.tar.gz
    rm rust.tar.gz
    pushd "rust-${RUST_TOOLCHAIN_VERSION}-${TARGET}"
      ./install.sh
    popd
    rm -rf "rust-${RUST_TOOLCHAIN_VERSION}-${TARGET}"
  popd
fi

mkdir -p ~/.cargo
cat > ~/.cargo/config <<'CARGO_CONFIG'
[source.crates-io]
registry = 'https://github.com/rust-lang/crates.io-index'
CARGO_CONFIG

if [[ ! -f src/bridge_generated.rs || ! -f flutter/lib/generated_bridge.dart ]] && [[ -n "${BRIDGE_ARTIFACT_DIR}" ]]; then
  cp "${BRIDGE_ARTIFACT_DIR}/src/bridge_generated.rs" ./src/
  cp "${BRIDGE_ARTIFACT_DIR}/src/bridge_generated.io.rs" ./src/
  cp "${BRIDGE_ARTIFACT_DIR}/flutter/lib/generated_bridge.dart" ./flutter/lib/
  cp "${BRIDGE_ARTIFACT_DIR}/flutter/lib/generated_bridge.freezed.dart" ./flutter/lib/
  cp "${BRIDGE_ARTIFACT_DIR}/flutter/macos/Runner/bridge_generated.h" ./flutter/macos/Runner/
  cp "${BRIDGE_ARTIFACT_DIR}/flutter/ios/Runner/bridge_generated.h" ./flutter/ios/Runner/
fi

if [[ ! -f src/bridge_generated.rs || ! -f flutter/lib/generated_bridge.dart ]]; then
  if [[ "${GENERATE_BRIDGE}" != "1" ]]; then
    echo "Missing flutter-rust-bridge generated files." >&2
    echo "Set GENERATE_BRIDGE=1, or set BRIDGE_ARTIFACT_DIR to the unpacked bridge-artifact directory." >&2
    exit 1
  fi

  echo "Generating flutter-rust-bridge files with Flutter ${BRIDGE_FLUTTER_VERSION}."
  if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}"
    # shellcheck disable=SC1091
    . "${HOME}/.cargo/env"
  fi
  rustup toolchain install "${RUST_VERSION}" --component rustfmt
  rustup default "${RUST_VERSION}"

  if [[ ! -x /opt/flutter-bridge/bin/flutter ]]; then
    pushd /opt
      wget -O "flutter_bridge_${BRIDGE_FLUTTER_VERSION}.tar.xz" "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${BRIDGE_FLUTTER_VERSION}-stable.tar.xz"
      tar xf "flutter_bridge_${BRIDGE_FLUTTER_VERSION}.tar.xz"
      mv flutter flutter-bridge
      rm "flutter_bridge_${BRIDGE_FLUTTER_VERSION}.tar.xz"
    popd
  fi
  export PATH="/opt/flutter-bridge/bin:${PATH}"
  if ! flutter --version; then
    echo "Flutter ${BRIDGE_FLUTTER_VERSION} could not run in this container." >&2
    echo "Generate the bridge files once in an x86_64 Ubuntu 22.04 container, then pass BRIDGE_ARTIFACT_DIR to this ARM build." >&2
    exit 1
  fi

  cargo install cargo-expand --version "${CARGO_EXPAND_VERSION}" --locked
  cargo install flutter_rust_bridge_codegen --version "${FLUTTER_RUST_BRIDGE_VERSION}" --features "uuid" --locked
  sed -i -e 's/extended_text: 14.0.0/extended_text: 13.0.0/g' flutter/pubspec.yaml
  pushd flutter
    flutter pub get
  popd
  "${HOME}/.cargo/bin/flutter_rust_bridge_codegen" \
    --rust-input ./src/flutter_ffi.rs \
    --dart-output ./flutter/lib/generated_bridge.dart \
    --c-output ./flutter/macos/Runner/bridge_generated.h
  cp ./flutter/macos/Runner/bridge_generated.h ./flutter/ios/Runner/bridge_generated.h
fi

export RUSTDESK_ID_SERVER="${RUSTDESK_ID_SERVER:-}"
export RUSTDESK_RELAY_SERVER="${RUSTDESK_RELAY_SERVER:-}"
export RUSTDESK_SERVER_KEY="${RUSTDESK_SERVER_KEY:-}"

cargo build --locked --lib ${JOBS} --features hwcodec,flutter,unix-file-copy-paste --release
rm -rf target/release/deps target/release/build
rm -rf ~/.cargo

export PATH="/opt/flutter-elinux/bin:${PATH}"
sed -i "s/flutter build linux --release/flutter-elinux build linux --verbose/g" ./build.py
sed -i "s/x64\/release/arm64\/release/g" ./build.py

pushd /opt
  if [[ ! -d flutter-elinux ]]; then
    wget -O "flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
    tar xf "flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
    git clone https://github.com/sony/flutter-elinux.git
    pushd flutter-elinux
      git fetch
      git reset --hard "${FLUTTER_VERSION}"
      bin/flutter-elinux doctor -v
      bin/flutter-elinux precache --linux
    popd
    cp -R flutter/bin/cache/artifacts/engine/linux-x64/shader_lib flutter-elinux/flutter/bin/cache/artifacts/engine/linux-arm64
    rm -rf flutter "flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
  fi
popd

if [[ "${FLUTTER_VERSION}" == "3.24.5" ]]; then
  pushd /opt/flutter-elinux/flutter
    if git apply --check "${REPO_ROOT}/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"; then
      git apply "${REPO_ROOT}/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"
    fi
  popd
fi

export CARGO_INCREMENTAL=0
export DEB_ARCH
python3 ./build.py --flutter --skip-cargo
for name in rustdesk*??.deb; do
  [[ -e "${name}" ]] || continue
  mv "${name}" "${name%%.deb}-${ARCH}.deb"
done

ls -lh rustdesk-*.deb
