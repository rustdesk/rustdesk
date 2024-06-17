#!/usr/bin/env bash

if [ "$1" == "debug" ];
then
    cargo ndk --platform 21 --target aarch64-linux-android build --features flutter,hwcodec
else
    cargo ndk --platform 21 --target aarch64-linux-android build --release --features flutter,hwcodec
fi
