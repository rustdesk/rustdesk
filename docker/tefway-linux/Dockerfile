# Reproducible Linux builder for Tefway Remote (Rust + Flutter).
#
# Build from the repository root:
#   podman build -t tefway-linux-builder:bookworm -f docker/tefway-linux/Dockerfile .
#
# The image intentionally does not use the project's upstream vcpkg tree: the
# Tefway build enables the existing `linux-pkg-config` cargo feature, so the
# codec libraries come from the Debian packages below. That keeps the image
# small, the dependency set reviewable, and the build inside the low-RAM host's
# budget. The produced AppImage bundles the few non-desktop libraries it needs.
#
# Everything is pinned by version and, for downloaded single-file tools, by
# SHA-256. The Flutter SDK tarball is pinned by version and SHA-256 as well.

FROM rust:1.99.1-bookworm

ARG DEBIAN_FRONTEND=noninteractive
ARG FLUTTER_VERSION=3.44.8
ARG FLUTTER_SHA256=672089e001571a9fbb209a495c583580c0c6c73ef98999264ba07fa93ace332d
ARG FRB_CODEGEN_VERSION=1.80.1
ARG FRB_CODEGEN_SHA256=20c97979ececa378db89b0bdf641e60702e3c00af3779ffadb5ad21f5782c0db
ARG APPIMAGETOOL_VERSION=1.9.1
ARG APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
ARG LINUXDEPLOY_VERSION=1-alpha-20251107-1
ARG LINUXDEPLOY_SHA256=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d

ENV LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    TZ=UTC \
    APPIMAGE_EXTRACT_AND_RUN=1 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        file \
        git \
        jq \
        libaom-dev \
        libasound2-dev \
        libayatana-appindicator3-dev \
        libclang-dev \
        libdbus-1-dev \
        libdrm-dev \
        libegl1-mesa-dev \
        libgbm-dev \
        libglib2.0-dev \
        libgles2-mesa-dev \
        libgtk-3-dev \
        libgstreamer-plugins-base1.0-dev \
        libgstreamer1.0-dev \
        liblzma-dev \
        libopus-dev \
        libpulse-dev \
        libssl-dev \
        libsystemd-dev \
        libudev-dev \
        libva-dev \
        libvpx-dev \
        libwayland-dev \
        libx11-dev \
        libxcb-randr0-dev \
        libxcb-render-util0-dev \
        libxcb-shape0-dev \
        libxcb-util-dev \
        libxcb-xfixes0-dev \
        libxcb1-dev \
        libxcursor-dev \
        libxfixes-dev \
        libxi-dev \
        libxinerama-dev \
        libxkbcommon-dev \
        libxrandr-dev \
        libxtst-dev \
        libyuv-dev \
        llvm-dev \
        nasm \
        ninja-build \
        patchelf \
        pax-utils \
        pkg-config \
        python3 \
        rsync \
        unzip \
        wget \
        xz-utils \
        yasm \
        zip \
    && rm -rf /var/lib/apt/lists/*

# Flutter 3.44 is required by this tree: pubspec.lock asks for Dart >= 3.10 and
# the 3.44 theme API is already used in flutter/lib/common.dart.
RUN curl -fsSL -o /tmp/flutter.tar.xz \
        "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
    && echo "${FLUTTER_SHA256}  /tmp/flutter.tar.xz" | sha256sum -c - \
    && tar -xf /tmp/flutter.tar.xz -C /opt \
    && rm -f /tmp/flutter.tar.xz \
    && git config --global --add safe.directory /opt/flutter \
    && git config --global --add safe.directory /work

ENV PATH=/opt/flutter/bin:/usr/local/cargo/bin:${PATH}

RUN flutter config --no-analytics --enable-linux-desktop \
    && flutter precache --linux \
    && flutter --version

# Debian ships libyuv headers and library but no pkg-config file, and the
# `linux-pkg-config` cargo feature probes `libyuv` by that name. Provide the
# standard descriptor instead of switching the whole build back to vcpkg.
RUN printf '%s\n' \
        'prefix=/usr' \
        'exec_prefix=${prefix}' \
        'libdir=${exec_prefix}/lib/x86_64-linux-gnu' \
        'includedir=${prefix}/include' \
        '' \
        'Name: libyuv' \
        'Description: YUV conversion and scaling library' \
        'Version: 0.0.1888' \
        'Libs: -L${libdir} -lyuv' \
        'Cflags: -I${includedir}' \
        > /usr/lib/x86_64-linux-gnu/pkgconfig/libyuv.pc \
    && pkg-config --libs libyuv opus vpx aom

# Prebuilt generator: compiling 1.80.1 from source on a modern toolchain is a
# known source of breakage, and CI pins exactly this release binary.
RUN curl -fsSL -o /tmp/frb.tgz \
        "https://github.com/fzyzcjy/flutter_rust_bridge/releases/download/v${FRB_CODEGEN_VERSION}/flutter_rust_bridge_codegen-x86_64-unknown-linux-gnu-v${FRB_CODEGEN_VERSION}.tgz" \
    && echo "${FRB_CODEGEN_SHA256}  /tmp/frb.tgz" | sha256sum -c - \
    && tar -xzf /tmp/frb.tgz -C /usr/local/bin \
    && chmod 0755 /usr/local/bin/flutter_rust_bridge_codegen \
    && rm -f /tmp/frb.tgz \
    && flutter_rust_bridge_codegen --version

# AppImage tooling. Both are AppImages themselves; APPIMAGE_EXTRACT_AND_RUN
# lets them run inside the container without /dev/fuse and without --privileged.
RUN mkdir -p /opt/appimage \
    && curl -fsSL -o /opt/appimage/appimagetool \
        "https://github.com/AppImage/appimagetool/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-x86_64.AppImage" \
    && echo "${APPIMAGETOOL_SHA256}  /opt/appimage/appimagetool" | sha256sum -c - \
    && chmod 0755 /opt/appimage/appimagetool \
    && curl -fsSL -o /opt/appimage/linuxdeploy \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-x86_64.AppImage" \
    && echo "${LINUXDEPLOY_SHA256}  /opt/appimage/linuxdeploy" | sha256sum -c - \
    && chmod 0755 /opt/appimage/linuxdeploy \
    && /opt/appimage/appimagetool --version \
    && /opt/appimage/linuxdeploy --version

# flutter_rust_bridge_codegen formats the generated Rust with rustfmt, which the
# official rust image does not install by default.
RUN rustup component add rustfmt \
    && rustfmt --version

WORKDIR /work
