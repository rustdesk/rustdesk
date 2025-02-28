<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#freie-öffentliche-server">Server</a> •
  <a href="#grobe-schritte-zum-kompilieren">Kompilieren</a> •
  <a href="#auf-docker-kompilieren">Docker</a> •
  <a href="#dateistruktur">Dateistruktur</a> •
  <a href="#screenshots">Screenshots</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Wir brauchen Ihre Hilfe, um dieses README, die <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk-Benutzeroberfläche</a> und die <a href="https://github.com/rustdesk/doc.rustdesk.com">Dokumentation</a> in Ihre Muttersprache zu übersetzen.</b>
</p>

Reden Sie mit uns auf: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

RustDesk ist eine in Rust geschriebene Remote-Desktop-Software, die out of the box ohne besondere Konfiguration funktioniert. Sie haben die volle Kontrolle über Ihre Daten und müssen sich keine Sorgen um die Sicherheit machen. Sie können unseren Rendezvous/Relay-Server nutzen, [einen eigenen Server aufsetzen](https://rustdesk.com/server) oder [einen eigenen Server programmieren](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk heißt jegliche Mitarbeit willkommen. Schauen Sie sich [CONTRIBUTING-DE.md](CONTRIBUTING-DE.md) an, wenn Sie Unterstützung beim Start brauchen.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**Programm herunterladen**](https://github.com/rustdesk/rustdesk/releases)

[**Nächtliche Erstellung**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Abhängigkeiten

Desktop-Versionen verwenden [Sciter](https://sciter.com/) oder Flutter für die GUI, dieses Tutorial ist nur für Sciter.

Bitte laden Sie die dynamische Bibliothek Sciter selbst herunter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Grobe Schritte zum Kompilieren

- Bereiten Sie Ihre Rust-Entwicklungsumgebung und C++-Build-Umgebung vor

- Installieren Sie [vcpkg](https://github.com/microsoft/vcpkg) und fügen Sie die Systemumgebungsvariable `VCPKG_ROOT` hinzu

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
  - Linux/macOS: `vcpkg install libvpx libyuv opus aom`

- Nutzen Sie `cargo run`

## [Erstellen](https://rustdesk.com/docs/de/dev/build/)

## Kompilieren auf Linux

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

### vcpkg installieren

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx reparieren (für Fedora)

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

### Kompilieren

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

## Auf Docker kompilieren

Beginnen Sie damit, das Repository zu klonen und den Docker-Container zu bauen:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Führen Sie jedes Mal, wenn Sie das Programm kompilieren müssen, folgenden Befehl aus:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Bedenken Sie, dass das erste Kompilieren länger dauern kann, bis die Abhängigkeiten zwischengespeichert sind. Nachfolgende Kompiliervorgänge sind schneller. Wenn Sie verschiedene Argumente für den Kompilierbefehl angeben müssen, können Sie dies am Ende des Befehls an der Position `<OPTIONAL-ARGS>` tun. Wenn Sie zum Beispiel eine optimierte Releaseversion kompilieren wollen, können Sie `--release` am Ende des Befehls anhängen. Das daraus entstehende Programm finden Sie im Zielordner auf Ihrem System. Sie können es mit folgendem Befehl ausführen:

```sh
target/debug/rustdesk
```

Oder, wenn Sie eine Releaseversion benutzen:

```sh
target/release/rustdesk
```

Bitte stellen Sie sicher, dass Sie diese Befehle im Stammverzeichnis des RustDesk-Repositorys nutzen. Ansonsten kann es passieren, dass das Programm die Ressourcen nicht finden kann. Bitte bedenken Sie auch, dass andere Cargo-Unterbefehle wie `install` oder `run` aktuell noch nicht unterstützt werden, da sie das Programm innerhalb des Containers starten oder installieren würden, anstatt auf Ihrem eigentlichen System.

> [!Vorsicht]
> **Haftungsausschluss bei Missbrauch::** <br>
> Die Entwickler von RustDesk billigen oder unterstützen keine unethische oder illegale Nutzung dieser Software. Missbrauch, wie unbefugter Zugriff, unbefugte Kontrolle oder Verletzung der Privatsphäre, verstößt strikt gegen unsere Richtlinien. Die Autoren sind nicht verantwortlich für jeglichen Missbrauch der Anwendung.

## Dateistruktur

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: Video-Codec, Konfiguration, TCP/UDP-Wrapper, Protokoll-Puffer, fs-Funktionen für Dateitransfer und ein paar andere nützliche Funktionen
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: Bildschirmaufnahme
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: Plattformspezifische Maus- und Tastatursteuerung
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: Audio/Zwischenablage/Eingabe/Videodienste und Netzwerkverbindungen
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: Starten einer Peer-Verbindung
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Mit [rustdesk-server](https://github.com/rustdesk/rustdesk-server) kommunizieren, warten auf direkte (TCP hole punching) oder weitergeleitete Verbindung
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: Plattformspezifischer Code
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter-Code für Handys
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript für Flutter-Webclient

## Screenshots

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
