#!/bin/bash

#
# Script to build F-Droid release of RustDesk
#
# Copyright (C) 2024, The RustDesk Authors
#               2024, Vasyl Gello <vasek.gello@gmail.com>
#

# The script is invoked by F-Droid builder system ste-by-step.
#
# It accepts the following arguments:
#
# - versionName from https://github.com/rustdesk/rustdesk/releases/download/fdroid-version/rustdesk-version.txt
# - versionCode from https://github.com/rustdesk/rustdesk/releases/download/fdroid-version/rustdesk-version.txt
# - Android architecture to build APK for: armeabi-v7a arm64-v8av x86 x86_64
# - The build step to execute:
#
#   + sudo-deps: as root, install needed Debian packages into builder VM
#   + prebuild: patch sources and do other stuff before the build
#   + build: perform actual build of APK file
#

# Start of functions

# Install Flutter of version `VERSION` from Github repository
# into directory `FLUTTER_DIR` and apply patches if needed

prepare_flutter() {
	VERSION="${1}"
	FLUTTER_DIR="${2}"

	if [ ! -f "${FLUTTER_DIR}/bin/flutter" ]; then
		git clone https://github.com/flutter/flutter "${FLUTTER_DIR}"
	fi

	pushd "${FLUTTER_DIR}"

	git restore .
	git checkout "${VERSION}"

	# Patch flutter

	if dpkg --compare-versions "${VERSION}" ge "3.24.4"; then
		git apply "${ROOTDIR}/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"
	fi

	flutter config --no-analytics

	popd # ${FLUTTER_DIR}
}

# Start of script

set -x

# Note current working directory as root dir for patches

ROOTDIR="${PWD}"

# Parse command-line arguments

VERNAME="${1}"
VERCODE="${2}"
ANDROID_ABI="${3}"
BUILDSTEP="${4}"

if [ -z "${VERNAME}" ] || [ -z "${VERCODE}" ] || [ -z "${ANDROID_ABI}" ] ||
	[ -z "${BUILDSTEP}" ]; then
	echo "ERROR: Command-line arguments are all required to be non-empty!" >&2
	exit 1
fi

# Set various architecture-specific identifiers

case "${ANDROID_ABI}" in
arm64-v8a)
	FLUTTER_TARGET=android-arm64
	NDK_TARGET=aarch64-linux-android
	RUST_TARGET=aarch64-linux-android
	RUSTDESK_FEATURES='flutter,hwcodec'
	;;
armeabi-v7a)
	FLUTTER_TARGET=android-arm
	NDK_TARGET=arm-linux-androideabi
	RUST_TARGET=armv7-linux-androideabi
	RUSTDESK_FEATURES='flutter,hwcodec'
	;;
x86_64)
	FLUTTER_TARGET=android-x64
	NDK_TARGET=x86_64-linux-android
	RUST_TARGET=x86_64-linux-android
	RUSTDESK_FEATURES='flutter'
	;;
x86)
	FLUTTER_TARGET=android-x86
	NDK_TARGET=i686-linux-android
	RUST_TARGET=i686-linux-android
	RUSTDESK_FEATURES='flutter'
	;;
*)
	echo "ERROR: Unknown Android ABI '${ANDROID_ABI}'!" >&2
	exit 1
	;;
esac

# Check ANDROID_SDK_ROOT and sdkmanager present on PATH

if [ ! -d "${ANDROID_SDK_ROOT}" ] || ! command -v sdkmanager 1>/dev/null; then
	echo "ERROR: Can not find Android SDK!" >&2
	exit 1
fi

# Export necessary variables

export PATH="${PATH}:${HOME}/flutter/bin:${HOME}/depot_tools"

export VCPKG_ROOT="${HOME}/vcpkg"

# Now act depending on build step

# NOTE: F-Droid maintainers require explicit declaration of dependencies
# as root via `Builds.sudo` F-Droid metadata directive:
# https://gitlab.com/fdroid/fdroiddata/-/merge_requests/15343#note_1988918695

case "${BUILDSTEP}" in
prebuild)
	# prebuild: patch sources and do other stuff before the build

	#
	# Extract required versions for NDK, Rust, Flutter from
	# '.github/workflows/flutter-build.yml'
	#

	CARGO_NDK_VERSION="$(yq -r \
		.env.CARGO_NDK_VERSION \
		.github/workflows/flutter-build.yml)"

	# Flutter used to compile main Rustdesk library

	FLUTTER_VERSION="$(yq -r \
		.env.ANDROID_FLUTTER_VERSION \
		.github/workflows/flutter-build.yml)"

	if [ -z "${FLUTTER_VERSION}" ]; then
		FLUTTER_VERSION="$(yq -r \
			.env.FLUTTER_VERSION \
			.github/workflows/flutter-build.yml)"
	fi

	# Flutter used to compile Flutter<->Rust bridge files

	FLUTTER_BRIDGE_VERSION="$(yq -r \
		.env.FLUTTER_VERSION \
		.github/workflows/bridge.yml)"

	FLUTTER_RUST_BRIDGE_VERSION="$(yq -r \
		.env.FLUTTER_RUST_BRIDGE_VERSION \
		.github/workflows/bridge.yml)"

	NDK_VERSION="$(yq -r \
		.env.NDK_VERSION \
		.github/workflows/flutter-build.yml)"

	RUST_VERSION="$(yq -r \
		.env.RUST_VERSION \
		.github/workflows/flutter-build.yml)"

	VCPKG_COMMIT_ID="$(yq -r \
		.env.VCPKG_COMMIT_ID \
		.github/workflows/flutter-build.yml)"

	if [ -z "${CARGO_NDK_VERSION}" ] || [ -z "${FLUTTER_VERSION}" ] ||
		[ -z "${FLUTTER_BRIDGE_VERSION}" ] ||
		[ -z "${FLUTTER_RUST_BRIDGE_VERSION}" ] ||
		[ -z "${NDK_VERSION}" ] || [ -z "${RUST_VERSION}" ] ||
		[ -z "${VCPKG_COMMIT_ID}" ]; then
		echo "ERROR: Can not identify all required versions!" >&2
		exit 1
	fi

	# Map NDK version to revision

	NDK_VERSION="$(wget \
		-qO- \
		-H "Accept: application/vnd.github+json" \
		-H "X-GitHub-Api-Version: 2022-11-28" \
		'https://api.github.com/repos/android/ndk/releases' |
		jq -r ".[] | select(.tag_name == \"${NDK_VERSION}\") | .body | match(\"ndkVersion \\\"(.*)\\\"\").captures[0].string")"

	if [ -z "${NDK_VERSION}" ]; then
		echo "ERROR: Can not map Android NDK codename to revision!" >&2
		exit 1
	fi

	export ANDROID_NDK_HOME="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"
	export ANDROID_NDK_ROOT="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"

	#
	# Install the components
	#

	set -e

	# Install Android NDK

	if [ ! -d "${ANDROID_NDK_ROOT}" ]; then
		sdkmanager --install "ndk;${NDK_VERSION}"
	fi

	# Install Rust

	if [ ! -f "${HOME}/rustup/rustup-init.sh" ]; then
		pushd "${HOME}"

		git clone --depth 1 https://github.com/rust-lang/rustup

		popd # ${HOME}
	fi

	pushd "${HOME}/rustup"
	bash rustup-init.sh -y \
		--target "${RUST_TARGET}" \
		--default-toolchain "${RUST_VERSION}"
	popd

	if ! command -v cargo 1>/dev/null 2>&1; then
		. "${HOME}/.cargo/env"
	fi

	# Install cargo-ndk

	cargo install \
		cargo-ndk \
		--version "${CARGO_NDK_VERSION}" \
		--locked

	# Install rust bridge generator

	cargo install \
		cargo-expand \
		--locked
	cargo install flutter_rust_bridge_codegen \
		--version "${FLUTTER_RUST_BRIDGE_VERSION}" \
		--features "uuid" \
		--locked

	# Populate native vcpkg dependencies

	if [ ! -d "${VCPKG_ROOT}" ]; then
		pushd "${HOME}"

		git clone \
			https://github.com/Microsoft/vcpkg.git
		git clone \
			https://github.com/Microsoft/vcpkg-tool.git

		pushd vcpkg-tool

		mkdir build

		pushd build

		cmake \
			-DCMAKE_BUILD_TYPE=Release \
			-G 'Ninja' \
			-DVCPKG_DEVELOPMENT_WARNINGS=OFF \
			..

		cmake --build .

		popd # build

		popd # vcpkg-tool

		pushd vcpkg

		git reset --hard "${VCPKG_COMMIT_ID}"

		cp -a ../vcpkg-tool/build/vcpkg vcpkg

		# disable telemetry

		touch "vcpkg.disable-metrics"

		popd # vcpkg

		popd # ${HOME}
	fi

	# Install depot-tools for x86

	if [ "${ANDROID_ABI}" = "x86" ]; then
		if [ ! -d "${HOME}/depot_tools" ]; then
			pushd "${HOME}"

			git clone \
				--depth 1 \
				https://chromium.googlesource.com/chromium/tools/depot_tools.git

			popd # ${HOME}
		fi
	fi

	# Patch the RustDesk sources

	git apply res/fdroid/patches/*.patch

	# If Flutter version used to generate bridge files differs from Flutter
	# version used to compile Rustdesk library, generate bridge using the
	# `FLUTTER_BRIDGE_VERSION` an restore the pubspec later

	if [ "${FLUTTER_VERSION}" != "${FLUTTER_BRIDGE_VERSION}" ]; then
		# Install Flutter bridge version

		prepare_flutter "${FLUTTER_BRIDGE_VERSION}" "${HOME}/flutter"

		# Save changes

		git add .

		# Edit pubspec to make flutter bridge version work

		sed \
			-i \
			-e 's/extended_text: 14.0.0/extended_text: 13.0.0/g' \
			flutter/pubspec.yaml

		# Download Flutter dependencies

		pushd flutter

		flutter clean
		flutter packages pub get

		popd # flutter

		# Generate FFI bindings

		flutter_rust_bridge_codegen \
			--rust-input ./src/flutter_ffi.rs \
			--dart-output ./flutter/lib/generated_bridge.dart

		# Add bridge files to save-list

		git add -f ./flutter/lib/generated_bridge.* ./src/bridge_generated.*

		# Restore everything

		git checkout '*'
		git clean -dffx
		git reset
	fi

	# Install Flutter version for RustDesk library build

	prepare_flutter "${FLUTTER_VERSION}" "${HOME}/flutter"

	# gms is not in thoes files now, but we still keep the following line for future reference(maybe).

	sed \
		-i \
		-e '/gms/d' \
		flutter/android/build.gradle \
		flutter/android/app/build.gradle

	# `firebase_analytics` is not in these files now, but we still keep the following lines.

	sed \
		-i \
		-e '/firebase_analytics/d' \
		flutter/pubspec.yaml

	sed \
		-i \
		-e '/  firebase/,/    version/d' \
		flutter/pubspec.lock

	sed \
		-i \
		-e '/firebase/Id' \
		flutter/lib/main.dart

	;;
build)
	# build: perform actual build of APK file

	set -e

	#
	# Extract required versions for NDK, Rust, Flutter from
	# '.github/workflows/flutter-build.yml'
	#

	# Flutter used to compile main Rustdesk library

	FLUTTER_VERSION="$(yq -r \
		.env.ANDROID_FLUTTER_VERSION \
		.github/workflows/flutter-build.yml)"

	if [ -z "${FLUTTER_VERSION}" ]; then
		FLUTTER_VERSION="$(yq -r \
			.env.FLUTTER_VERSION \
			.github/workflows/flutter-build.yml)"
	fi

	NDK_VERSION="$(yq -r \
		.env.NDK_VERSION \
		.github/workflows/flutter-build.yml)"

	# Map NDK version to revision

	NDK_VERSION="$(wget \
		-qO- \
		-H "Accept: application/vnd.github+json" \
		-H "X-GitHub-Api-Version: 2022-11-28" \
		'https://api.github.com/repos/android/ndk/releases' |
		jq -r ".[] | select(.tag_name == \"${NDK_VERSION}\") | .body | match(\"ndkVersion \\\"(.*)\\\"\").captures[0].string")"

	if [ -z "${NDK_VERSION}" ]; then
		echo "ERROR: Can not map Android NDK codename to revision!" >&2
		exit 1
	fi

	export ANDROID_NDK_HOME="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"
	export ANDROID_NDK_ROOT="${ANDROID_SDK_ROOT}/ndk/${NDK_VERSION}"

	if ! command -v cargo 1>/dev/null 2>&1; then
		. "${HOME}/.cargo/env"
	fi

	# Download Flutter dependencies

	pushd flutter

	flutter clean
	flutter packages pub get

	popd # flutter

	# Build host android deps

	bash flutter/build_android_deps.sh "${ANDROID_ABI}"

	# Build rustdesk lib

	cargo ndk \
		--platform 21 \
		--target "${RUST_TARGET}" \
		--bindgen \
		build \
		--release \
		--features "${RUSTDESK_FEATURES}"

	mkdir -p "flutter/android/app/src/main/jniLibs/${ANDROID_ABI}"

	cp "target/${RUST_TARGET}/release/liblibrustdesk.so" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}/librustdesk.so"

	cp "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/${NDK_TARGET}/libc++_shared.so" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}/"

	"${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" \
		"flutter/android/app/src/main/jniLibs/${ANDROID_ABI}"/*

	# Build flutter-jit-release for x86

	if [ "${ANDROID_ABI}" = "x86" ]; then
		pushd flutter-sdk

		echo "## Sync flutter engine sources"
		echo "### We need fakeroot because chromium base image is unpacked with weird uid/gid ownership"

		sed -i "s/FLUTTER_VERSION_PLACEHOLDER/${FLUTTER_VERSION}/" .gclient

		export FAKEROOTDONTTRYCHOWN=1

		fakeroot gclient sync

		unset FAKEROOTDONTTRYCHOWN

		pushd src

		echo "## Patch away Google Play dependencies"

		rm \
			flutter/shell/platform/android/io/flutter/app/FlutterPlayStoreSplitApplication.java \
			flutter/shell/platform/android/io/flutter/embedding/engine/deferredcomponents/PlayStoreDeferredComponentManager.java flutter/shell/platform/android/io/flutter/embedding/android/FlutterPlayStoreSplitApplication.java

		sed \
			-i \
			-e '/PlayStore/d' \
			flutter/tools/android_lint/project.xml \
			flutter/shell/platform/android/BUILD.gn

		sed \
			-i \
			-e '/com.google.android.play/d' \
			flutter/tools/androidx/files.json

		echo "## Configure android engine build"

		flutter/tools/gn \
			--android --android-cpu x86 --runtime-mode=jit_release \
			--no-goma --no-enable-unittests

		echo "## Perform android engine build"

		ninja -C out/android_jit_release_x86

		echo "## Configure host engine build"

		flutter/tools/gn \
			--android-cpu x86 --runtime-mode=jit_release \
			--no-goma --no-enable-unittests

		echo "## Perform android engine build"

		ninja -C out/host_jit_release_x86

		echo "## Rename host engine"

		mv out/host_jit_release_x86 out/host_jit_release

		echo "## Mimic jit_release engine to debug to use with flutter build apk"

		pushd out/android_jit_release_x86

		sed \
			-e 's/jit_release/debug/' \
			flutter_embedding_jit_release.maven-metadata.xml \
			1>flutter_embedding_debug.maven-metadata.xml

		sed \
			-e 's/jit_release/debug/' \
			flutter_embedding_jit_release.pom \
			1>flutter_embedding_debug.pom

		sed \
			-e 's/jit_release/debug/' \
			x86_jit_release.maven-metadata.xml \
			1>x86_debug.maven-metadata.xml

		sed \
			-e 's/jit_release/debug/' \
			x86_jit_release.pom \
			1>x86_debug.pom

		cp -a \
			flutter_embedding_jit_release-sources.jar \
			flutter_embedding_debug-sources.jar

		cp -a \
			flutter_embedding_jit_release.jar \
			flutter_embedding_debug.jar

		cp -a \
			x86_jit_release.jar \
			x86_debug.jar

		popd # out/android_jit_release_x86

		popd # src

		popd # flutter-sdk

		echo "# Clean up intermediate engine files and show free space"

		rm -rf \
			flutter-sdk/src/out/android_jit_release_x86/obj \
			flutter-sdk/src/out/host_jit_release/obj

		mv flutter-sdk/src/out flutter-out

		rm -rf flutter-sdk

		mkdir -p flutter-sdk/src/

		mv flutter-out flutter-sdk/src/out
	fi

	# Build the apk

	pushd flutter

	if [ "${ANDROID_ABI}" = "x86" ]; then
		flutter build apk \
			--local-engine-src-path="$(readlink -mf "../flutter-sdk/src")" \
			--local-engine=android_jit_release_x86 \
			--debug \
			--build-number="${VERCODE}" \
			--build-name="${VERNAME}" \
			--target-platform "${FLUTTER_TARGET}"
	else
		flutter build apk \
			--release \
			--build-number="${VERCODE}" \
			--build-name="${VERNAME}" \
			--target-platform "${FLUTTER_TARGET}"
	fi

	popd # flutter

	rm -rf flutter-sdk

	# Special step for fdroiddata CI builds to remove .gitconfig

	rm -f /home/vagrant/.gitconfig

	;;
*)
	echo "ERROR: Unknown build step '${BUILDSTEP}'!" >&2
	exit 1
	;;
esac

# Report success

echo "All done!"
