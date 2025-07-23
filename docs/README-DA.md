<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#gratis-offentlige-servere">Servere</a> •
  <a href="#rå-trin-til-at-bygge">Byg</a> •
  <a href="#sådan-bygger-du-med-docker">Docker</a> •
  <a href="#filstruktur">Filstruktur</a> •
  <a href="#skærmbilleder">Skærmbilleder</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Vi har brug for din hjælp til at oversætte denne README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> og <a href=" https://github.com/rustdesk/doc.rustdesk.com">Dokument</a> til dit modersmål</b>
</p>

Chat med os: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Endnu en fjernskrivebordssoftware, skrevet i Rust. Fungerer ud af æsken, ingen konfiguration påkrævet. Du har fuld kontrol over dine data uden bekymringer om sikkerhed. Du kan bruge vores rendezvous/relay-server, [opsætte din egen](https://rustdesk.com/server), eller [skrive din egen rendezvous/relay-server](https://github.com/rustdesk/rustdesk- server-demo).

RustDesk hilser bidrag fra alle velkommen. Se [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) for at få hjælp til at komme i gang.

[**PROGRAM DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## Afhængigheder

Desktopversioner bruger [sciter](https://sciter.com/) eller Flutter til GUI, denne vejledning er kun for Sciter.

Hent venligst sciter dynamic library selv.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Rå trin til at bygge

- Forbered din Rust-udviklings-env og C++ build-env

- Installer [vcpkg](https://github.com/microsoft/vcpkg), og indstil env-variabelen "VCPKG_ROOT" korrekt

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- kør `cargo run`

## [Byg](https://rustdesk.com/docs/en/dev/build/)

## Sådan bygger du på Linux

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

### vcpkg installation

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx rettelse (For Fedora)

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

### Byg

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
cargo run
```

## Sådan bygger du med Docker

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Kør derefter følgende kommando, hver gang du skal bygge applikationen:
```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Bemærk, at den første bygning kan tage længere tid, før afhængigheder cachelagres, efterfølgende bygninger vil være hurtigere. Derudover, hvis du har brug for at angive forskellige argumenter til bygge-kommandoen, kan du gøre det i slutningen af kommandoen i `<VALGFRI-ARGS>`-positionen. For eksempel, hvis du ville bygge en optimeret udgivelsesversion, ville du køre kommandoen ovenfor efterfulgt af `--release`. Den resulterende eksekverbare vil være tilgængelig i målmappen på dit system og kan køres med:

```sh
target/debug/rustdesk
```

Eller, hvis du kører en udgivelses eksekverbar:

```sh
target/release/rustdesk
```

Sørg for, at du kører disse kommandoer fra roden af RustDesk-lageret, ellers kan applikationen muligvis ikke finde de nødvendige ressourcer. Bemærk også, at andre cargo underkommandoer såsom 'install' eller 'run' i øjeblikket ikke understøttes via denne metode, da de ville installere eller køre programmet inde i containeren i stedet for værten.

## Filstruktur

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs funktioner til filoverførsel og nogle andre hjælpefunktioner
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: Skærmbillede
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platform specifik tastatur/mus kontrol
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: lyd/udklipsholder/input/videotjenester og netværksforbindelser
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: starte en peer-forbindelse
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Kommuniker med [rustdesk-server](https://github.com/rustdesk/rustdesk-server), vent på direkte fjernforbindelse (TCP-hulning) eller relæforbindelse
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: Javascript til Flutter webklient

## Skærmbilleder

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
