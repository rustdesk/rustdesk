#!/usr/bin/env bash
cargo build --locked --features flutter,hwcodec --release --target aarch64-apple-ios --lib
