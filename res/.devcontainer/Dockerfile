FROM mcr.microsoft.com/devcontainers/base:ubuntu-22.04
ENV HOME=/home/vscode
ENV WORKDIR=$HOME/rustdesk

WORKDIR $HOME
RUN sudo apt update -y && sudo apt install -y g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake unzip zip sudo libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
WORKDIR /

RUN git clone https://github.com/microsoft/vcpkg 
WORKDIR vcpkg
RUN git checkout 2023.04.15
RUN /vcpkg/bootstrap-vcpkg.sh -disableMetrics
ENV VCPKG_ROOT=/vcpkg
RUN $VCPKG_ROOT/vcpkg --disable-metrics install libvpx libyuv opus aom

WORKDIR /
RUN wget https://github.com/rustdesk/doc.rustdesk.com/releases/download/console/dep.tar.gz && tar xzf dep.tar.gz


USER vscode
WORKDIR $HOME
RUN wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
RUN chmod +x rustup.sh
RUN $HOME/rustup.sh -y
RUN $HOME/.cargo/bin/rustup target add aarch64-linux-android
RUN $HOME/.cargo/bin/cargo install cargo-ndk

# Install Flutter
RUN wget https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.10.1-stable.tar.xz
RUN tar xf flutter_linux_3.10.1-stable.tar.xz && rm flutter_linux_3.10.1-stable.tar.xz
ENV PATH="$PATH:$HOME/flutter/bin"
RUN dart pub global activate ffigen 5.0.1


# Install packages
RUN sudo apt-get install -y libclang-dev
RUN sudo apt install -y gcc-multilib

WORKDIR $WORKDIR
ENV ANDROID_NDK_HOME=/opt/android/ndk/22.1.7171670

# Somehow try to automate flutter pub get
# https://rustdesk.com/docs/en/dev/build/android/
# Put below steps in entrypoint.sh
# cd flutter
# wget https://github.com/rustdesk/doc.rustdesk.com/releases/download/console/so.tar.gz
# tar xzf so.tar.gz

# own /opt/android
