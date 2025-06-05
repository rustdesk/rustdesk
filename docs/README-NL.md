<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Uw bureaublad op afstand"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Bouwen</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structuur</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Wij hebben uw hulp nodig om dit README bestand te vertalen, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> en <a href="https://github.com/rustdesk/doc.rustdesk.com">Doc</a> naar uw moedertaal</b>
</p>

Chat met ons: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Alweer een andere programma voor -bureaublad op afstand-, geschreven in Rust. Werkt -out of the box-, geen configuratie nodig. U heeft volledige controle over uw gegevens, en hoeft zich geen zorgen te maken over de beveiliging. U kunt onze rendez-vous/relay server gebruiken, [je eigen server opzetten](https://rustdesk.com/blog/id-relay-set), of [je eigen rendez-vous/relay-server schrijven](https://github.com/rustdesk/rustdesk-server-demo).

RustDesk verwelkomt bijdragen van iedereen. Zie [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) voor hulp om aan de slag te gaan.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

[**NIGHTLY BUILD**](https://github.com/rustdesk/rustdesk/releases/tag/nightly) (meest recente build)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Download het op F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Afhankelijkheden

Desktop versies gebruiken [sciter](https://sciter.com/) of Flutter voor GUI, deze handleiding is alleen voor Sciter.

Download zelf de dynamic library van Sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Ruwe stappen om te bouwen

- Bereid je Rust-ontwikkelomgeving en C++-bouwomgeving voor.

- Installeer [vcpkg](https://github.com/microsoft/vcpkg) en configureer de `VCPKG_ROOT` omgevingsvariabele op de juiste manier:

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- Voer uit: `cargo run`

## [Bouwen](https://rustdesk.com/docs/en/dev/build/)

## Bouwen op Linux

### Ubuntu 18 (Debian 10)

```sh
sudo apt install -y g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake
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

### Installatie van vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Fix voor libvpx (voor Fedora)

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

### Bouwen

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

## Bouwen met Docker

Begin met het klonen van de repository en het bouwen van de docker container:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Elke keer dat u de toepassing moet bouwen, voert u het volgende commando uit:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Let op dat de eerste build langer kan duren omdat de dependencies nog niet zijn gecached; latere builds zullen sneller zijn. Als je extra command line arguments wilt toevoegen aan het build-commando, dan kun je dat doen aan het einde van de opdrachtregel in plaats van `<OPTIONAL-ARGS>`. Bijvoorbeeld: als je een geoptimaliseerde releaseversie wilt bouwen, draai dan het bovenstaande commando gevolgd door `--release`.

 Het uitvoerbare bestand, in debug-modus, zal verschijnen in de target-map, en kan als volgt worden uitgevoerd:

```sh
target/debug/rustdesk
```

Als je een release-versie hebt gebouwd, is het commando als volgt:

```sh
target/release/rustdesk
```

Zorg ervoor dat je deze commando's van de root van de RustDesk-repository uitvoert, anders kan het programma de nodige afhankelijkheden mogelijk niet vinden. Let ook op dat andere cargo-subcommando's zoals `install` en `run` zijn momenteel niet ondersteund, aangezien deze zouden worden uitgevoerd in een container in plaats van op de host.

## Bestandsstructuur

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: videocodec, configuratie, TCP/UDP-wrapper, protobuf, bestandssysteemfuncties voor bestandsoverdracht en nog wat andere nuttige functies
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: schermopname
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platformspecifieke muis- en toetsenbordbeheer
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: geluids-, klembord-, invoer- en video-services, netwerkverbindingen
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: voor het opzetten van peer-verbindingen
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Communicatie met [rustdesk-server](https://github.com/rustdesk/rustdesk-server), afwachten van redirect op afstand (TCP hole punching) of een relayed verbinding
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platformspecifieke code

## Snapshot

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
