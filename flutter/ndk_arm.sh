#!/usr/bin/env bash
echo armv7-linux-androideabi
cargo ndk --platform 21 --target armv7-linux-androideabi build --release --features flutter,hwcodec
