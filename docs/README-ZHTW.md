<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - 您的遠端桌面"><br>
  <a href="#基本建置步驟">建置</a> •
  <a href="#如何使用-docker-建置">Docker</a> •
  <a href="#檔案結構">結構</a> •
  <a href="#螢幕截圖">螢幕截圖</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>] | [<a href="README-HI.md">हिन्दी</a>] | [<a href="README-BN.md">বাংলা</a>] | [<a href="README-UR.md">اردو</a>] | [<a href="README-TH.md">ไทย</a>]<br>
  <b>我們需要您的協助，將此 README、<a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk 使用者介面</a>和 <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk 文件</a>翻譯成您的母語</b>
</p>

> [!Caution]
> **濫用免責聲明：** <br>
> RustDesk 的開發者不容許也不支持任何不道德或非法使用本軟體的行為。未經授權的存取、控制或侵犯隱私等濫用行為，嚴重違反我們的準則。作者對應用程式的任何濫用概不負責。


與我們交流：[Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

又一個以 Rust 撰寫的遠端桌面解決方案。開箱即用，無需任何設定。您可以完全掌控自己的資料，無須擔心安全問題。您可以使用我們的 rendezvous/relay 伺服器、[架設您自己的伺服器](https://rustdesk.com/server)，或是[撰寫您自己的 rendezvous/relay 伺服器](https://github.com/rustdesk/rustdesk-server-demo)。

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk 歡迎所有人貢獻。請參閱 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何開始。

[**常見問題**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**下載執行檔**](https://github.com/rustdesk/rustdesk/releases)

[**每夜建置版本**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## 相依套件

桌面版本使用 Flutter 或 Sciter（已棄用）作為圖形介面。本教學僅適用於 Sciter，因為它更容易上手。若要建置 Flutter 版本，請參閱我們的 [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml)。

請自行下載 Sciter 動態函式庫。

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## 基本建置步驟

- 準備好 Rust 開發環境與 C++ 建置環境

- 安裝 [vcpkg](https://github.com/microsoft/vcpkg)，並正確設定環境變數 `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- 執行 `cargo run`

## [建置](https://rustdesk.com/docs/en/dev/build/)

## 如何在 Linux 上建置

### Ubuntu 18 (Debian 10)

```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev \
        libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
        libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

### openSUSE Tumbleweed

```sh
sudo zypper install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libXfixes-devel cmake alsa-lib-devel gstreamer-devel gstreamer-plugins-base-devel xdotool-devel
```

### Fedora 28 (CentOS 8)

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel
```

### Arch (Manjaro)

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### 安裝 vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### 修正 libvpx（適用於 Fedora）

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

### 建置

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

## 如何使用 Docker 建置

首先複製儲存庫並建置 Docker 容器：

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

之後，每次需要建置應用程式時，請執行以下指令：

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

請注意，第一次建置可能需要較長時間，因為相依套件尚未快取；之後的建置會更快。此外，如果需要為建置指令指定其他參數，可以將其加在指令結尾的 `<OPTIONAL-ARGS>` 位置。例如，若要建置最佳化的發行版本，請在上述指令後加上 `--release` 再執行。產生的執行檔會位於系統中的 target 資料夾，可以透過以下指令執行：

```sh
target/debug/rustdesk
```

或者，若您執行的是發行版本的執行檔：

```sh
target/release/rustdesk
```

請確認您是在 RustDesk 儲存庫的根目錄執行這些指令，否則應用程式可能找不到所需的資源。另請注意，此方法目前不支援 `install` 或 `run` 等其他 cargo 子指令，因為它們會在容器內而非主機上安裝或執行程式。

## 檔案結構

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**：視訊編解碼器、設定、tcp/udp 封裝，以及其他與伺服器共用的工具函式
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**：protobuf、檔案傳輸用的 fs 函式、僅供本應用程式使用的鍵盤與平台程式碼
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**：螢幕擷取
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**：特定平台的鍵盤/滑鼠控制
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**：Windows、Linux、macOS 的檔案複製與貼上實作。
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**：舊版 Sciter 使用者介面（已棄用）
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**：音訊/剪貼簿/輸入/視訊服務，以及網路連線
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**：建立點對點連線
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**：與 [rustdesk-server](https://github.com/rustdesk/rustdesk-server) 通訊，等待遠端直接連線（TCP 打洞）或中繼連線
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**：特定平台的程式碼
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**：桌面版與行動版的 Flutter 程式碼

## 螢幕截圖

![連線管理員](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![已連線至 Windows 電腦](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![檔案傳輸](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP 通道](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
