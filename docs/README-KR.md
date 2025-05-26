<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>README를 모국어로 번역하는 데 당신의 도움이 필요합니다.</b>
</p>

채팅하기: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)


[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Rust로 작성되었고, 설정 없이 바로 사용할 수 있는 원격 데스크톱 소프트웨어입니다. 자신의 데이터를 완전히 제어할 수 있고, 보안 염려도 없습니다. 저희 rendezvous/relay 서버를 사용하거나, [직접 설정](https://rustdesk.com/server)하거나 [자체 rendezvous/relay 서버를 구축](https://github.com/rustdesk/rustdesk-server-demo)할 수도 있습니다.

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk는 모든 기여를 환영합니다. 기여하고 싶다면 [`docs/CONTRIBUTING.md`](CONTRIBUTING.md)를 참고해 주세요.

[**RustDesk는 어떻게 작동하는가?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## 의존관계

데스크톱 버전은 GUI에 [sciter](https://sciter.com/)를 사용합니다. sciter dynamic library를 다운로드해 주세요. 

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

모바일 버전은 Flutter를 사용합니다. 데스크톱 버전 또한 Sciter에서 Flutter로 이전할 예정입니다.

## 빌드 순서

- Rust 개발 환경과 C++ 빌드 환경을 준비합니다.

- [vcpkg](https://github.com/microsoft/vcpkg) 설치하고 `VCPKG_ROOT` 환경변수를 정확히 설정합니다.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- `cargo run`을 실행합니다.

## [빌드](https://rustdesk.com/docs/en/dev/build/)

## Linux에서 빌드 순서

### Ubuntu 18 (Debian 10)

```sh
sudo apt install -y g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake
```

### Fedora 28 (CentOS 8)

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel
```

### Arch (Manjaro)

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### vcpkg 설치

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx 수정 (For Fedora용)

```sh
cd vcpkg/buildtrees/libvpx/src
cd *
./configure
sed -i 's/CFLAGS+=-I/CFLAGS+=-fPIC -I/g' Makefile
sed -i 's/CXXFLAGS+=-I/CXXFLAGS+=-fPIC -I/g' Makefile
make
cp libvpx.a $HOME/vcpkg/installed/x64-linux/lib/
cd
```

### 빌드

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## Docker에 빌드하는 방법

리포지토리를 복제하고 Docker 컨테이너를 구성하는 것으로 시작합니다.

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

이후 애플리케이션을 빌드해야 할 때마다 아래 명령을 실행합니다.

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

첫 빌드 시에는 의존성이 캐시될 때까지 시간이 걸릴 수 있지만, 이후 빌드부터는 빨라집니다. 또한 빌드 명령에 다른 인수를 지정해야 한다면, 명령 끝의 `<OPTIONAL-ARGS>`에 지정할 수 있습니다. 예를 들어, 최적화된 출시 버전을 빌드하고 싶다면 위 명령 뒤에 `--release`를 붙여 실행합니다. 성공하면 실행 파일은 시스템의 대상 폴더에 저장되며, 다음 명령으로 실행할 수 있습니다.

```sh
target/debug/rustdesk
```

혹은 출시용 실행 파일을 실행할 수도 있습니다.

```sh
target/release/rustdesk
```

명령을 RustDesk 리포지토리 루트에서 실행하는지 확인해 주세요. 그렇지 않으면 애플리케이션이 필요한 리소스를 찾지 못할 수 있습니다. 또한 `install`, `run`과 같은 cargo 하위 명령은 호스트가 아닌 컨테이너 내에서 프로그램을 설치하고 실행하기 위한 것이므로, 현재 이 방법은 지원되지 않는다는 점에 유의해 주십시오.

## 파일 구조

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: 비디오 코덱, 설정, TCP/UDP 래퍼, Protobuf, 파일 전송을 위한 fs 함수, 그 외 유틸리티 함수
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: 화면 캡처
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: 플랫폼 고유 키보드/마우스 제어
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: 오디오, 클립보드, 입력, 비디오 서비스 및 네트워크 연결
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: 피어 연결 시작
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server)와 통신하여 리모트 다이렉트 (TCP Hole Punching) 또는 Relayed 연결
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: 플랫폼 고유의 코드
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: 모바일용 Flutter 코드
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Flutter 웹 클라이언트용 JavaScript

> [!주의]
> **오용에 대한 면책 조항:** <br>
> RustDesk 개발팀은 이 소프트웨어의 비윤리적이거나 불법적인 사용을 용납하거나 지원하지 않습니다. 무단 접근, 제어 또는 개인 정보 침해와 같은 오용은 저희 지침을 엄격히 위반하는 것입니다. 개발팀은 애플리케이션의 오용에 대해 책임을 지지 않습니다.

## 스냅샷

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)

