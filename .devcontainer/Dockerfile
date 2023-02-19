FROM mcr.microsoft.com/devcontainers/base:ubuntu-22.04
ENV HOME=/home/vscode
ENV WORKDIR=$HOME/rustdesk

WORKDIR $HOME
RUN sudo apt update -y && sudo apt install -y g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake unzip zip sudo libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
WORKDIR /

RUN git clone https://github.com/microsoft/vcpkg 
WORKDIR vcpkg
RUN git checkout 134505003bb46e20fbace51ccfb69243fbbc5f82
RUN /vcpkg/bootstrap-vcpkg.sh -disableMetrics
RUN /vcpkg/vcpkg --disable-metrics install libvpx libyuv opus

USER vscode
WORKDIR $HOME
RUN wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh
RUN chmod +x rustup.sh
RUN ./rustup.sh -y

# Install Flutter
RUN wget https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.7.3-stable.tar.xz
RUN tar xf flutter_linux_3.7.3-stable.tar.xz
RUN export PATH="$PATH:/home/user/flutter/bin"
