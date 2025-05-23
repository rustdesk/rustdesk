<p align="center">
  <img src="res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#public-servers">Servere</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Struktur</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="docs/README-UA.md">Українська</a>] | [<a href="docs/README-CS.md">česky</a>] | [<a href="docs/README-ZH.md">中文</a>] | [<a href="docs/README-HU.md">Magyar</a>] | [<a href="docs/README-ES.md">Español</a>] | [<a href="docs/README-FA.md">فارسی</a>] | [<a href="docs/README-FR.md">Français</a>] | [<a href="docs/README-DE.md">Deutsch</a>] | [<a href="docs/README-PL.md">Polski</a>] | [<a href="docs/README-ID.md">Indonesian</a>] | [<a href="docs/README-FI.md">Suomi</a>] | [<a href="docs/README-ML.md">മലയാളം</a>] | [<a href="docs/README-JP.md">日本語</a>] | [<a href="docs/README-NL.md">Nederlands</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Русский</a>] | [<a href="docs/README-PTBR.md">Português (Brasil)</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">한국어</a>] | [<a href="docs/README-AR.md">العربي</a>] | [<a href="docs/README-VN.md">Tiếng Việt</a>] | [<a href="docs/README-DA.md">Dansk</a>] | [<a href="docs/README-GR.md">Ελληνικά</a>] | [<a href="docs/README-TR.md">Türkçe</a>] | [<a href="docs/README-NO.md">Norsk</a><br>
  <b>Vi trenger din hjelp til å oversette denne README-en, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> og <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a> tid ditt morsmål</b>
</p>

Snakk med oss: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Enda en annen fjernstyrt desktop programvare, skrevet i Rust. Virker rett ut av pakken, ingen konfigurasjon nødvendig. Du har full kontroll over din data, uten beskymring for sikkerhet. Du kan bruke vår rendezvous_mediator/relay server, [sett opp din egen](https://rustdesk.com/server), eller [skriv din egen rendezvous_mediator/relay server](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk er velkommen for bidrag fra alle. Se [CONTRIBUTING.md](docs/CONTRIBUTING-NO.md) for hjelp med oppstart.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**BINARY NEDLASTING**](https://github.com/rustdesk/rustdesk/releases)

[**NIGHTLY BUILD**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Få det på F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Få det på Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Avhengigheter

Desktop versjoner bruker Flutter eller Sciter (avviklet) for GUI, denne veiledningen er bare for Sciter, grunnet att det er letter og en mer venlig start. Skjekk ut vår [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) for bygging av Flutter versjonen.

Venligst last ned Sciters dynamiske bibliotek selv.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Rå steg for bygging

- Klargjør ditt Rust development env og C++ build env

- Installer [vcpkg](https://github.com/microsoft/vcpkg), og koriger `VCPKG_ROOT` env vaiabelen

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- Kjør `cargo run`

## [Bygg](https://rustdesk.com/docs/en/dev/build/)

## Hvordan Bygge til Linux

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

### Installer vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Fiks libvpx (For Fedora)

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

### Bygg

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

## Hvordan bygge med Docker

Start med å klone repositoret og bygg Docker konteineren:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Deretter, hver gang du trenger å bygge applikasjonen, kjør følgene kommando:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Det kan ta lengere tid før avhengighetene blir bufret første gang du bygger, senere bygg er raskere. Hvis du trenger å spesifisere forkjellige argumenter til bygge kommandoen, kan du gjøre det på slutten av kommandoen ved `<OPTIONAL-ARGS>` feltet. For eksempel, hvis du ville bygge en optimalisert release versjon, ville du kjørt kommandoen over fulgt `--release`. Den kjørbare filen vill være tilgjengelig i mål direktive på ditt system, og kan bli kjørt med:

```sh
target/debug/rustdesk
```

Eller, hvis du kjører ett release program:

```sh
target/release/rustdesk
```

Venligst pass på att du kjører disse kommandoene fra roten av RustDesk repositoret, eller kan det hende att applikasjon ikke finner de riktige ressursene. Pass også på att andre cargo subkommandoer som for eksempel `install` eller `run` ikke støttes med denne metoden da de vill installere eller kjøre programmet i konteineren istedet for verten.

## Fil Struktur

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video kodek, configurasjon, tcp/udp innpakning, protobuf, fs funksjon for fil overføring, og noen andre verktøy funksjoner
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: skjermfangst
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platform spesefik keyboard/mus kontroll
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: fil kopi og innliming implementasjon for Windows, Linux, macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: foreldret Sciter UI (avviklet)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: lyd/utklippstavle/input/video tjenester, og internett tilkobling
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: start en peer tilkobling
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Kommunikasjon med [rustdesk-server](https://github.com/rustdesk/rustdesk-server), vent på direkte fjernstyring (TCP hulling) eller vidresendt tilkobling
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platform spesefik kode
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter kode for desktop og mobil
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript for Flutter nettsted klient

## Skjermbilder

![Tilkoblings Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Koble til Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![Fil Overføring](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)

