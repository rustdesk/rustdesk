#!/usr/bin/env bash
cargo build --features flutter,hwcodec --release --target aarch64-apple-ios --lib
