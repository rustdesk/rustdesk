<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - desktopul tău la distanță"><br>
  <a href="../README.md#raw-steps-to-build">Construire</a> •
  <a href="../README.md#how-to-build-with-docker">Docker</a> •
  <a href="../README.md#file-structure">Structură</a> •
  <a href="../README.md#snapshot">Capturi</a><br>
  [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>]<br>
  <b>Avem nevoie de ajutorul tău pentru a traduce acest README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> și <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a> în limba ta maternă</b>
</p>

> [!Atenție]
> **Declinare de responsabilitate privind utilizarea abuzivă:** <br>
> Dezvoltatorii RustDesk nu susțin sau aprobă utilizarea neetică sau ilegală a acestui software. Utilizarea abuzivă, cum ar fi accesul neautorizat, controlul sau invadarea intimității, este strict împotriva regulilor noastre. Autorii nu sunt responsabili pentru utilizarea necorespunzătoare a aplicației.


Conversați cu noi: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Încă o soluție de desktop la distanță scrisă în Rust. Funcționează imediat, fără configurare necesară. Ai control total asupra datelor tale, fără probleme de securitate. Poți folosi serverul nostru de rendezvous/relay, [să-ți configurezi propriul server](https://rustdesk.com/server) sau [să scrii propriul server de rendezvous/relay](https://github.com/rustdesk/rustdesk-server-demo).

![imagine](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk primește contribuții de la oricine. Vezi [CONTRIBUTING.md](../docs/CONTRIBUTING.md) pentru ajutor la început.

[**ÎNTREBĂRI FRECVENTE (FAQ)**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**DESCĂRCARE BINARE**](https://github.com/rustdesk/rustdesk/releases)

[**BUILD NIGHTLY**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Dependențe

Versiunile desktop folosesc Flutter sau Sciter (depreciat) pentru interfață; acest ghid este pentru Sciter doar, deoarece este mai ușor și mai prietenos pentru început. Vezi [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) pentru construire cu Flutter.

Te rugăm să descarci singur librăria dinamică Sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Pași pentru construire (Raw Steps to build)

- Pregătește mediul de dezvoltare Rust și mediul de construire C++

- Instalează [vcpkg](https://github.com/microsoft/vcpkg) și setează corect variabila de mediu `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- rulează `cargo run`

## [Construire](https://rustdesk.com/docs/en/dev/build/)

## Cum se construiește pe Linux

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

### Instalează vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Repară libvpx (Pentru Fedora)

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

### Build

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

## Cum să construiești cu Docker

Începe prin clonarea repository-ului și construirea imaginii Docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Apoi, de fiecare dată când trebuie să construiești aplicația, rulează comanda următoare:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Reține că prima construire poate dura mai mult până când dependențele sunt în cache; construirile ulterioare vor fi mai rapide. De asemenea, dacă trebuie să specifici argumente diferite comenzii de build, le poți adăuga la finalul comenzii în poziția `<OPTIONAL-ARGS>`. De exemplu, pentru a construi o versiune optimizată de release, adaugă `--release`. Executabilul rezultat va fi disponibil în folderul `target` pe sistemul tău, și poate fi rulat cu:

```sh
target/debug/rustdesk
```

Sau, dacă rulezi un executabil release:

```sh
target/release/rustdesk
```

Asigură-te că rulezi aceste comenzi din rădăcina repository-ului RustDesk, altfel aplicația poate să nu găsească resursele necesare. De asemenea, reține că alte subcomenzi cargo, cum ar fi `install` sau `run`, nu sunt acceptate în prezent prin această metodă, deoarece ar instala sau rula programul în interiorul containerului în loc de gazdă.

## Structura fișierelor

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: codec video, config, wrapper tcp/udp, protobuf, funcții fs pentru transfer de fișiere și alte funcții utilitare
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: capturare ecran
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: control tastatură/mouse specific platformei
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: implementare copy/paste pentru fișiere pentru Windows, Linux, macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: interfață Sciter învechită (depreciată)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: servicii audio/clipboard/input/video și conexiuni de rețea
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: inițiază o conexiune peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: comunică cu [rustdesk-server](https://github.com/rustdesk/rustdesk-server), așteaptă conexiune directă remote (TCP hole punching) sau prin relay
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: cod specific platformei
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: cod Flutter pentru desktop și mobil
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/v1/js)**: JavaScript pentru clientul Flutter web

## Capturi de ecran

![Connection Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connected to a Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![File Transfer](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
