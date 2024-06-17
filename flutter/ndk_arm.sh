#!/usr/bin/env bash

if [ "$1" == "debug" ];
then
    cargo ndk --platform 21 --target armv7-linux-androideabi build --features flutter,hwcodec
else
    cargo ndk --platform 21 --target armv7-linux-androideabi build --release --features flutter,hwcodec
fi
