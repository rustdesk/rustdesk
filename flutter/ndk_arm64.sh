#!/usr/bin/env bash
cargo ndk --platform 21 --target aarch64-linux-android build --locked --release --features flutter,hwcodec
