#! /usr/bin/env bash
sed -i "s/\b$1\b/$2/g" res/*spec res/PKGBUILD flutter/pubspec.yaml Cargo.toml .github/workflows/*yml flatpak/*json appimage/*yml libs/portable/Cargo.toml
cargo run # to bump version in cargo lock
