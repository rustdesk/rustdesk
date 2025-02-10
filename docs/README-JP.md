<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - あなたのためのリモートデスクトップ"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="docs/README-UA.md">Українська</a>] | [<a href="docs/README-CS.md">česky</a>] | [<a href="docs/README-ZH.md">中文</a>] | [<a href="docs/README-HU.md">Magyar</a>] | [<a href="docs/README-ES.md">Español</a>] | [<a href="docs/README-FA.md">فارسی</a>] | [<a href="docs/README-FR.md">Français</a>] | [<a href="docs/README-DE.md">Deutsch</a>] | [<a href="docs/README-PL.md">Polski</a>] | [<a href="docs/README-ID.md">Indonesian</a>] | [<a href="docs/README-FI.md">Suomi</a>] | [<a href="docs/README-ML.md">മലയാളം</a>] | [<a href="docs/README-JP.md">日本語</a>] | [<a href="docs/README-NL.md">Nederlands</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Русский</a>] | [<a href="docs/README-PTBR.md">Português (Brasil)</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">한국어</a>] | [<a href="docs/README-AR.md">العربي</a>] | [<a href="docs/README-VN.md">Tiếng Việt</a>] | [<a href="docs/README-DA.md">Dansk</a>] | [<a href="docs/README-GR.md">Ελληνικά</a>] | [<a href="docs/README-TR.md">Türkçe</a>]<br>
  <b>READMEや<a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a>、 <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a>の翻訳者を歓迎します！</b>
</p>

私たちと話す: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Rustで書かれた、設定不要ですぐに使えるリモートデスクトップソフトウェアです。自分のデータを完全にコントロールでき、セキュリティの心配もありません。私たちのランデブー/リレーサーバを使うことも、[自分でサーバーをセットアップする](https://rustdesk.com/server) ことも、 [自分でランデブー/リレーサーバを作成する](https://github.com/rustdesk/rustdesk-server-demo)こともできます。

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDeskは皆さんの貢献を歓迎します。  
貢献の方法については[CONTRIBUTING.md](docs/CONTRIBUTING.md)をご確認ください。

[**よくある質問**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**パッケージのダウンロード**](https://github.com/rustdesk/rustdesk/releases)

[**ナイトリービルド**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="F-Droidで入手する"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## 依存関係

デスクトップ版ではGUIにFlutterまたはSciter(非推奨)を使用しますが、チュートリアルでは分かりやすく、簡単なSciterのみを対象に解説しています。Flutterでのビルド方法については[CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml)をご覧ください。

Sciter dynamic libraryを事前にダウンロードしてください。

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## ビルド手順

- Rust開発環境とC++ビルド環境を準備します。

- [vcpkg](https://github.com/microsoft/vcpkg)をインストールし、環境変数に`VCPKG_ROOT`を設定します。  
その後、以下のコマンドを実行します。

  - Windowsの場合: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOSの場合: vcpkg install libvpx libyuv opus aom

- `cargo run`を実行します。

## [ビルド](https://rustdesk.com/docs/en/dev/build/)

## Linuxでのビルド方法

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
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel
```

### Arch (Manjaro)

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### vcpkgのインストール

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpxの修正 (Fedoraのみ)

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

### ビルド

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

## Dockerでのビルド方法

リポジトリをクローンし、Dockerコンテナを構築します:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

以下のコマンドを実行します:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```
このコマンドはRustDeskをビルドする度に実行する必要があります。  

初回ビルドは時間がかかるかもしれませんが、2回目以降は依存関係がキャッシュされるため、ビルドにかかる時間が短くなります。  
ビルドコマンドに追加の引数を指定する必要がある場合は、コマンドの最後(`<OPTIONAL-ARGS>`の位置)で指定することができます。例えば、最適化されたリリースバージョンをビルドしたい場合は、上記のコマンドの後に `--release` を追記し実行します。ビルドされた実行ファイルはあなたのシステムのターゲットフォルダに保存され、下記のコマンドで実行することができます。  

デバッグビルドを起動する場合:
```sh
target/debug/rustdesk
```

リリースビルドを起動する場合:

```sh
target/release/rustdesk
```

コマンドをRustDeskリポジトリのルートから実行していることを確認してください。また、`install` や `run` などの他のcargoサブコマンドは、ホストではなくコンテナ内でプログラムをインストール、実行するため、現在の方法ではサポートされていません。

## ファイル構造

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ビデオコーデック、設定、tcp/udpラッパー、protobuf、ファイル転送に利用されるfs関数やその他のユーティリティ関数
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: スクリーンキャプチャ
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: プラットフォーム固有のキーボード/マウス操作
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: Windows、Linux、macOS向けのファイルのコピーと貼り付けの実装
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: 廃止された Sciter UI (非推奨)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: 
オーディオ/クリップボード/入力/ビデオ サービスとネットワーク接続
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: ピア接続の開始
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server)と通信し、リモートの直接接続(TCPホールパンチング)や中継接続を担う。
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: プラットフォーム固有のコード
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: デスクトップとモバイル向けのFlutterコード
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Flutterウェブクライアント向けのJavaScript

> [!注意]
> **:不正使用に関する免責事項** <br>
> RustDeskの開発者は、このソフトウェアの非倫理的または違法な使用を容認または支持しません。不正アクセス、不正な制御、またはプライバシーの侵害などの不正使用は、当社のガイドラインに厳密に違反します。開発者は、アプリケーションの不正使用に対して一切の責任を負いません。

## スクリーンショット

![Connection Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connected to a Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![File Transfer](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
