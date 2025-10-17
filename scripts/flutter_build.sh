#!/bin/bash
set -e

# RustDesk Flutter Build Script
# Builds Flutter application and packages
# Assumes Rust library is already built

echo "==========================================="
echo "RustDesk Flutter Build Script (x86_64)"
echo "==========================================="

# Environment setup
export FLUTTER_VERSION="3.24.5"
export VERSION="1.4.2"
export ARCH="x86_64"
export DEB_ARCH="amd64"
export CARGO_INCREMENTAL=0

# Fix C++ include paths for Flutter Linux build
# This ensures C++ standard library headers are found
if [ -z "$CPATH" ]; then
    GCC_VERSION=$(gcc -dumpversion | cut -d. -f1)
    export CPATH="/usr/include/c++/${GCC_VERSION}:/usr/include/x86_64-linux-gnu/c++/${GCC_VERSION}"
fi

cd $(dirname $0)/..
WORKSPACE=$(pwd)

echo "Workspace: $WORKSPACE"
echo ""

# Step 1: Setup Flutter
echo "Step 1: Setting up Flutter..."
git config --global --add safe.directory "*"

if [ ! -d "/opt/flutter" ]; then
    echo "ERROR: Flutter not found at /opt/flutter"
    echo "Please run ./ci_setup.sh first"
    exit 1
fi

cd $WORKSPACE

export PATH=/opt/flutter/bin:$PATH

# Run flutter doctor
flutter doctor -v

echo "✓ Flutter setup complete"

# Step 2: Apply Flutter patches
echo ""
echo "Step 2: Applying Flutter patches..."
if [[ "3.24.5" == "$FLUTTER_VERSION" ]]; then
    pushd /opt/flutter
    if [ -f "$WORKSPACE/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff" ]; then
        git apply "$WORKSPACE/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff" || echo "Patch already applied or failed"
    fi
    popd
fi
echo "✓ Flutter patches applied"

# Step 3: Build Flutter application
echo ""
echo "Step 3: Building Flutter application..."
cd $WORKSPACE

# Check if Rust library exists
if [ ! -f "target/release/liblibrustdesk.so" ]; then
    echo "ERROR: Rust library not found at target/release/liblibrustdesk.so"
    echo "Please build the Rust library first:"
    echo "  ./scripts/rust_build.sh"
    exit 1
fi

# Generate flutter_rust_bridge if not exists
if [ ! -f "flutter/lib/generated_bridge.dart" ]; then
    echo "Generating flutter_rust_bridge..."
    ./scripts/generate_bridge.sh
fi

echo "✓ Building flutter app ..."
python3 ./build.py --flutter --skip-cargo

echo "✓ Flutter application built"

# Step 4: Rename deb packages
echo ""
echo "Step 4: Packaging DEB..."
for name in rustdesk*??.deb; do
    if [ -f "$name" ]; then
        mv "$name" "${name%%.deb}-${ARCH}.deb"
        echo "✓ Created: ${name%%.deb}-${ARCH}.deb"
    fi
done

# Step 5: Build RPM package for Fedora/CentOS
echo ""
echo "Step 5: Building Fedora/CentOS RPM package..."
cd $WORKSPACE

if command -v rpmbuild &> /dev/null; then
    HBB=$(pwd) rpmbuild ./res/rpm-flutter.spec -bb

    pushd ~/rpmbuild/RPMS/$ARCH
    for name in rustdesk*??.rpm; do
        if [ -f "$name" ]; then
            mv "$name" "$WORKSPACE/${name%%.rpm}.rpm"
            echo "✓ Created: ${name%%.rpm}.rpm"
        fi
    done
    popd
else
    echo "⚠ rpmbuild not found, skipping RPM packages"
fi

# Step 6: Build RPM package for SUSE
echo ""
echo "Step 6: Building SUSE RPM package..."
cd $WORKSPACE

if command -v rpmbuild &> /dev/null; then
    HBB=$(pwd) rpmbuild ./res/rpm-flutter-suse.spec -bb

    pushd ~/rpmbuild/RPMS/$ARCH
    for name in rustdesk*??.rpm; do
        if [ -f "$name" ]; then
            mv "$name" "$WORKSPACE/${name%%.rpm}-suse.rpm"
            echo "✓ Created: ${name%%.rpm}-suse.rpm"
        fi
    done
    popd
fi

# Step 7: Build Arch Linux package (x86_64 only)
echo ""
echo "Step 7: Building Arch Linux package..."
if [ "$ARCH" == "x86_64" ]; then
    if command -v makepkg &> /dev/null; then
        cd $WORKSPACE
        sed -i "s/x86_64/$ARCH/g" res/PKGBUILD
        cd res
        HBB=$(pwd)/.. FLUTTER=1 makepkg -f
        cd ..
        mv res/rustdesk-*.pkg.tar.zst . 2>/dev/null || echo "No arch package generated"
    else
        echo "⚠ makepkg not found, skipping Arch Linux package"
    fi
else
    echo "⚠ Skipping Arch Linux package (only for x86_64)"
fi

# Summary
echo ""
echo "==========================================="
echo "Flutter Build Complete!"
echo "==========================================="
echo ""
echo "Generated packages:"
cd $WORKSPACE
ls -lh rustdesk-*.deb 2>/dev/null || true
ls -lh rustdesk-*.rpm 2>/dev/null || true
ls -lh rustdesk-*.pkg.tar.zst 2>/dev/null || true
echo ""
