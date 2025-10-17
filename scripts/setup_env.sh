#!/bin/bash
set -e

# RustDesk CI-based Environment Setup Script
# Strictly follows .github/workflows/flutter-build.yml build-rustdesk-linux job

echo "==========================================="
echo "RustDesk CI Environment Setup (x86_64)"
echo "==========================================="

# Environment Variables (from CI)
export RUST_VERSION="1.75"
export FLUTTER_VERSION="3.24.5"
export VCPKG_COMMIT_ID="120deac3062162151622ca4860575a33844ba10b"
export VERSION="1.4.2"
export TARGET="x86_64-unknown-linux-gnu"
export VCPKG_TRIPLET="x64-linux"
export DEB_ARCH="amd64"
export ARCH="x86_64"

echo "Target: $TARGET"
echo "Architecture: $ARCH"
echo ""

# Step 1: Maximize build space (from CI line 1310-1319)
echo "Step 1: Maximizing build space..."
sudo rm -rf /opt/ghc || true
sudo rm -rf /usr/local/lib/android || true
sudo rm -rf /usr/share/dotnet || true

# Step 2: Install nasm and build dependencies (from CI line 1316)
echo ""
echo "Step 2: Installing build dependencies..."
sudo apt-get update -y
sudo apt-get install -y nasm libstdc++-10-dev g++ build-essential

# Fix libstdc++.so symlink for clang linker
if [ ! -f /usr/lib/x86_64-linux-gnu/libstdc++.so ]; then
    sudo ln -sf /usr/lib/x86_64-linux-gnu/libstdc++.so.6 /usr/lib/x86_64-linux-gnu/libstdc++.so
    echo "✓ Created libstdc++.so symlink"
fi

echo "✓ nasm installed: $(nasm --version | head -1)"
echo "✓ C++ toolchain ready"

# Step 3: Initialize git submodules (from CI line 1321-1324)
echo ""
echo "Step 3: Initializing git submodules..."
cd $(dirname $0)
git submodule update --init --recursive
echo "✓ Git submodules initialized"

# Step 4: Install Rust toolchain (from CI line 1337-1343)
echo ""
echo "Step 4: Installing Rust $RUST_VERSION..."
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain $RUST_VERSION
    source $HOME/.cargo/env
fi
rustup toolchain install $RUST_VERSION
rustup default $RUST_VERSION
rustup target add $TARGET
rustup component add rustfmt

# Save Rust toolchain version (from CI line 1345-1348)
RUST_TOOLCHAIN_VERSION=$(cargo --version | awk '{print $2}')
echo "RUST_TOOLCHAIN_VERSION=$RUST_TOOLCHAIN_VERSION"
export RUST_TOOLCHAIN_VERSION

echo "✓ Rust installed: rustc $(rustc --version)"

# Step 5: Configure Cargo.toml (from CI line 1350-1353)
echo ""
echo "Step 5: Configuring Cargo.toml..."
# Change library type to cdylib only
sed -i 's/\["cdylib", "staticlib", "rlib"\]/["cdylib"]/g' Cargo.toml
# Fix library name to avoid lib prefix duplication (librustdesk -> rustdesk)
sed -i 's/name = "librustdesk"/name = "rustdesk"/g' Cargo.toml
echo "✓ Cargo.toml configured (cdylib only, correct lib name)"

# Step 6: Setup vcpkg (from CI line 1362-1368)
echo ""
echo "Step 6: Setting up vcpkg..."
export VCPKG_ROOT=/opt/vcpkg

if [ ! -d "$VCPKG_ROOT" ]; then
    sudo mkdir -p /opt
    cd /opt
    sudo git clone https://github.com/Microsoft/vcpkg.git
    cd vcpkg
    sudo git checkout $VCPKG_COMMIT_ID
    sudo ./bootstrap-vcpkg.sh
    sudo chown -R $USER:$USER /opt/vcpkg
    cd -
else
    echo "vcpkg already exists at $VCPKG_ROOT"
fi

echo "✓ vcpkg ready: $(cd $VCPKG_ROOT && git rev-parse --short HEAD)"

# Step 7: Install vcpkg dependencies (from CI line 1370-1388)
echo ""
echo "Step 7: Installing vcpkg dependencies..."
echo "This will take 30-60 minutes..."
sudo apt-get install -y libva-dev

if ! $VCPKG_ROOT/vcpkg install --triplet $VCPKG_TRIPLET --x-install-root="$VCPKG_ROOT/installed"; then
    echo "ERROR: vcpkg installation failed"
    find "${VCPKG_ROOT}/" -name "*.log" | while read -r _1; do
        echo "$_1:"
        echo "======"
        cat "$_1"
        echo "======"
        echo ""
    done
    exit 1
fi

echo "✓ vcpkg dependencies installed"

# Step 8: Setup Flutter (from CI line 1482-1516)
echo ""
echo "Step 8: Setting up Flutter..."
export FLUTTER_DIR=/opt/flutter
export PATH=$FLUTTER_DIR/bin:$PATH

if [ ! -d "$FLUTTER_DIR" ]; then
    echo "Downloading Flutter $FLUTTER_VERSION..."
    cd /opt
    sudo wget https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz
    sudo tar xf flutter_linux_${FLUTTER_VERSION}-stable.tar.xz
    sudo rm flutter_linux_${FLUTTER_VERSION}-stable.tar.xz
    sudo chown -R $USER:$USER /opt/flutter
    cd -
else
    echo "Flutter already exists at $FLUTTER_DIR"
fi

# Apply Flutter patches if version matches
if [[ "$FLUTTER_VERSION" == "3.24.5" ]]; then
    echo "Applying Flutter patches..."
    cd $FLUTTER_DIR
    if [ -f "$(dirname $0)/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff" ]; then
        git apply "$(dirname $0)/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff" || echo "Patch already applied or not needed"
    fi
    cd -
fi

# Add Flutter to PATH in .bashrc
if ! grep -q "/opt/flutter/bin" "$HOME/.bashrc"; then
    echo "" >> "$HOME/.bashrc"
    echo "# Flutter (added by RustDesk ci_setup.sh)" >> "$HOME/.bashrc"
    echo "export PATH=\"/opt/flutter/bin:\$PATH\"" >> "$HOME/.bashrc"
    echo "✓ Added Flutter to PATH in ~/.bashrc"
else
    echo "✓ Flutter already in PATH in ~/.bashrc"
fi

# Run flutter doctor
echo "Running flutter doctor..."
flutter doctor -v 2>&1 | tee /tmp/flutter_doctor_output.log

echo "✓ Flutter installed: $(flutter --version | head -1)"
echo "✓ Flutter doctor output saved to: /tmp/flutter_doctor_output.log"

# Check if Flutter doctor found any issues
if grep -q "\[✗\]" /tmp/flutter_doctor_output.log; then
    echo ""
    echo "⚠ WARNING: Flutter doctor found some issues:"
    grep "\[✗\]" /tmp/flutter_doctor_output.log
    echo ""
    echo "For Linux desktop development, you need:"
    echo "  - Flutter (Linux toolchain)"
    echo "  - Linux toolchain (clang, cmake, ninja, pkg-config, gtk)"
    echo ""
    echo "Other platform toolchains (Android, Chrome, etc.) are optional."
else
    echo "✓ Flutter doctor checks passed"
fi

echo ""
echo "==========================================="
echo "Environment Setup Complete!"
echo "==========================================="
echo ""
echo "IMPORTANT: Set environment variables for current session:"
echo "  export VCPKG_ROOT=/opt/vcpkg"
echo "  export PATH=/opt/flutter/bin:\$HOME/.cargo/bin:\$PATH"
echo ""
echo "Or run: source <(tail -n 3 ci_setup.sh | head -n 2)"
echo ""
echo "Next steps:"
echo "  1. Run: ./ci_build.sh"
echo ""

# Export environment variables for sourcing
export VCPKG_ROOT=/opt/vcpkg
export PATH=/opt/flutter/bin:$HOME/.cargo/bin:$PATH
