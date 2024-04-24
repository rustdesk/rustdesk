#!/usr/bin/env bash
echo aarch64-linux-android
cargo ndk --platform 21 --target aarch64-linux-android build --release --features flutter,hwcodec
