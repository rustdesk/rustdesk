#!/usr/bin/env bash
# apt-get install -y git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake libclang-dev ninja-build libappindicator3-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libvdpau-dev libva-dev libclang-dev llvm-dev pkg-config g++ gcc libvpx-dev

# For developers in China, we use sources from ustc.
# sed -i 's/archive.archive.ubuntu.com/mirrors.ustc.edu.cn/g' /etc/apt/sources.list
# sed -i 's/security.archive.ubuntu.com/mirrors.ustc.edu.cn/g' /etc/apt/sources.list

dpkg --add-architecture armhf

apt update -y
apt install -y libdbus-1-dev:armhf pkg-config nasm yasm libglib2.0-dev:armhf libxcb-randr0-dev:armhf libxdo-dev:armhf libxfixes-dev:armhf libxcb-shape0-dev:armhf libxcb-xfixes0-dev:armhf libasound2-dev:armhf libpulse-dev:armhf libgstreamer1.0-dev:armhf libgstreamer-plugins-base1.0-dev:armhf libappindicator3-dev:armhf libvpx-dev:armhf libvdpau-dev:armhf libva-dev:armhf libgtk-3-dev:armhf clang gcc libclang-dev
