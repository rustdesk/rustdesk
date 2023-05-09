<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>このREADMEをあなたの母国語に翻訳するために、あなたの助けが必要です。</b>
</p>

Chat with us: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)


[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Rustで書かれた、設定不要ですぐに使えるリモートデスクトップソフトウェアです。自分のデータを完全にコントロールでき、セキュリティの心配もありません。私たちのランデブー/リレーサーバを使うことも、[自分で設定する](https://rustdesk.com/server) ことも、 [自分でランデブー/リレーサーバを書くこともできます](https://github.com/rustdesk/rustdesk-server-demo)。

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDeskは誰からの貢献も歓迎します。 貢献するには [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) を参照してください。

[**RustDeskはどの様に動くのか?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## 無料のパブリックサーバー

下記のサーバーは、無料で使用できますが、後々変更されることがあります。これらのサーバーから遠い場合、接続が遅い可能性があります。
| Location | Vendor | Specification |
| --------- | ------------- | ------------------ |
| Seoul | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Germany | Hetzner | 2 vCPU / 4GB RAM |
| Germany | Codext | 4 vCPU / 8GB RAM |
| Finland (Helsinki) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| USA (Ashburn) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |

## 依存関係

デスクトップ版ではGUIに [sciter](https://sciter.com/) が使われています。 sciter dynamic library をダウンロードしてください。

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

モバイル版はFlutterを利用します。デスクトップ版もSciterからFlutterへマイグレーション予定です。

## ビルド手順

- Rust開発環境とC ++ビルド環境を準備します

- [vcpkg](https://github.com/microsoft/vcpkg), をインストールし、 `VCPKG_ROOT` 環境変数を正しく設定します。

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- run `cargo run`



## [ビルド](https://rustdesk.com/docs/en/dev/build/)

## Linuxでのビルド手順

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

### Install vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Fix libvpx (For Fedora)

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

### Wayland の場合、X11（Xorg）に変更します

RustDeskはWaylandをサポートしていません。
 [こちら](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) を確認して、XorgをデフォルトのGNOMEセッションとして構成します。

## Dockerでビルドする方法

リポジトリのクローンを作成し、Dockerコンテナを構築することから始めます。

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

その後、アプリケーションをビルドする必要があるたびに、以下のコマンドを実行します。

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

なお、最初のビルドでは、依存関係がキャッシュされるまで時間がかかることがありますが、その後のビルドではより速くなります。さらに、ビルドコマンドに別の引数を指定する必要がある場合は、コマンドの最後にある `<OPTIONAL-ARGS>` の位置で指定することができます。例えば、最適化されたリリースバージョンをビルドしたい場合は、上記のコマンドの後に
`--release` を実行します。できあがった実行ファイルは、システムのターゲット・フォルダに格納され、次のコマンドで実行できます。

```sh
target/debug/rustdesk
```

あるいは、リリース用の実行ファイルを実行している場合:

```sh
target/release/rustdesk
```

これらのコマンドをRustDeskリポジトリのルートから実行していることを確認してください。そうしないと、アプリケーションが必要なリソースを見つけられない可能性があります。また、 `install` や `run` などの他の cargo サブコマンドは、ホストではなくコンテナ内にプログラムをインストールまたは実行するため、現在この方法ではサポートされていないことに注意してください。

## ファイル構造

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ビデオコーデック、コンフィグ、tcp/udpラッパー、protobuf、ファイル転送用のfs関数、その他のユーティリティ関数
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: スクリーンキャプチャ
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: プラットフォーム固有のキーボード/マウスコントロール
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: オーディオ/クリップボード/入力/ビデオサービス、ネットワーク接続
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: ピア接続の開始
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server), と通信し、リモートダイレクト (TCP hole punching) または中継接続を待つ。
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: プラットフォーム固有のコード

## スナップショット

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
