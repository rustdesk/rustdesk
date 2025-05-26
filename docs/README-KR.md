<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>]<br>
  <b>이 README와 <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> 및 <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk 문서</a>를 여러분의 모국어로 번역하는 데 도움이 필요합니다.</b>
</p>

> [!Caution]
> **오용 관련 면책 조항:** <br>
> RustDesk 개발자는 이 소프트웨어의 비윤리적이거나 불법적인 사용을 용납하거나 지원하지 않습니다. 무단 액세스, 제어 또는 사생활 침해와 같은 오용은 당사의 가이드라인에 엄격히 위배됩니다. 개발자는 애플리케이션의 오용에 대해 책임을 지지 않습니다.

채팅하기: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)


[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Rust로 작성되었고, 설정 없이 바로 사용할 수 있는 원격 데스크톱 소프트웨어입니다. 자신의 데이터를 완전히 제어할 수 있고, 보안 염려도 없습니다. 저희 rendezvous/relay 서버를 사용하거나, [직접 설정](https://rustdesk.com/server)하거나 [자체 rendezvous/relay 서버를 구축](https://github.com/rustdesk/rustdesk-server-demo)할 수도 있습니다.

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk는 모든 기여를 환영합니다. 기여하고 싶다면 [`CONTRIBUTING-KR.md`](CONTRIBUTING-KR.md)를 참고해 주세요.

[**자주 묻는 질문 (FAQ)**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**바이너리 다운로드**](https://github.com/rustdesk/rustdesk/releases)

[**나이틀리 빌드**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="F-Droid에서 다운로드"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Flathub에서 다운로드"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## 의존성

데스크톱 버전은 GUI에 Flutter 또는 Sciter (지원 중단됨)를 사용합니다. 이 튜토리얼은 Sciter 전용이며, 시작하기 더 쉽고 친숙하기 때문입니다. Flutter 버전 빌드는 [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml)를 확인하세요.

Sciter 동적 라이브러리를 직접 다운로드하세요.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## 기본 빌드 방법

- Rust 개발 환경과 C++ 빌드 환경을 준비하세요.

- [vcpkg](https://github.com/microsoft/vcpkg)를 설치하고 `VCPKG_ROOT` 환경변수를 정확히 설정하세요.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- `cargo run`을 실행합니다.

## [빌드](https://rustdesk.com/docs/en/dev/build/)

## Linux에서 빌드 방법

### Ubuntu 18 (Debian 10)

```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev \
        libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
        libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpam0g-dev
```

### openSUSE Tumbleweed

```sh
sudo zypper install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libXfixes-devel cmake alsa-lib-devel gstreamer-devel gstreamer-plugins-base-devel xdotool-devel pam-devel
```

### Fedora 28 (CentOS 8)

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel pam-devel
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
git clone --recurse-submodules https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## Docker로 빌드하는 방법

먼저 리포지토리를 복제하고 Docker 컨테이너를 빌드합니다:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

그 다음, 애플리케이션을 빌드하려면 다음 명령을 실행하세요:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

첫 빌드 시에는 의존성이 캐시되느라 시간이 더 걸릴 수 있지만, 그 이후 빌드부터는 더 빨라집니다. 빌드 명령에 다른 인수를 추가하고 싶다면, 명령 끝의 `<OPTIONAL-ARGS>` 부분에 지정하세요. 예를 들어, 최적화된 릴리즈 버전을 빌드하고 싶다면 위 명령 뒤에 `--release`를 붙여 실행합니다. 결과 실행 파일은 시스템의 target 폴더에 생성되며, 다음 명령으로 실행할 수 있습니다:

```sh
target/debug/rustdesk
```

또는, 릴리즈 실행 파일을 실행하는 경우:

```sh
target/release/rustdesk
```

이 명령들은 RustDesk 리포지토리의 루트 디렉토리에서 실행해야 합니다. 그렇지 않으면 애플리케이션이 필요한 리소스를 찾지 못할 수 있습니다. 또한, `install` 또는 `run`과 같은 cargo 하위 명령은 호스트가 아닌 컨테이너 내부에 프로그램을 설치하거나 실행하므로 현재 이 방식은 지원되지 않습니다. 이 점에 유의해 주세요.

## 파일 구조

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: 비디오 코덱, 설정, TCP/UDP 래퍼, Protobuf, 파일 전송을 위한 fs 함수 및 기타 유틸리티 함수
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: 화면 캡처
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: 플랫폼별 키보드/마우스 제어
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: Windows, Linux, macOS용 파일 복사 및 붙여넣기 구현
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: 더 이상 사용되지 않는 Sciter UI (지원 중단됨)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: 오디오/클립보드/입력/비디오 서비스 및 네트워크 연결
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: 피어 연결 시작
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server)와 통신, Remote Direct (TCP Hole Punching) 또는 Relayed Connection 대기
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: 플랫폼별 코드
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: 데스크톱 및 모바일용 Flutter 코드
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/v1/js)**: Flutter 웹 클라이언트용 JavaScript

## 스냅샷

![Connection Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connected to a Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![File Transfer](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)

