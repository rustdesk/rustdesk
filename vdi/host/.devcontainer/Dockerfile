FROM rockylinux:9.1
ENV HOME=/home/vscode
ENV WORKDIR=$HOME/rustdesk/vdi/host

# https://ciq.co/blog/top-10-things-to-do-after-rocky-linux-9-install/ also gpu driver install
WORKDIR $HOME
RUN dnf -y install epel-release
RUN dnf config-manager --set-enabled crb
RUN dnf -y install cargo libvpx-devel opus-devel usbredir-devel git cmake gcc-c++ pkg-config nasm yasm ninja-build automake libtool libva-devel libvdpau-devel llvm-devel
WORKDIR /

RUN git clone https://chromium.googlesource.com/libyuv/libyuv
WORKDIR /libyuv
RUN cmake . -DCMAKE_INSTALL_PREFIX=/user
RUN make -j4 && make install
WORKDIR /