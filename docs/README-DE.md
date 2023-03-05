<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#freie-öffentliche-server">Server</a> •
  <a href="#grobe-schritte-zum-kompilieren">Kompilieren</a> •
  <a href="#auf-docker-kompilieren">Docker</a> •
  <a href="#dateistruktur">Dateistruktur</a> •
  <a href="#screenshots">Screenshots</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Wir brauchen deine Hilfe, um dieses README, die <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk-Benutzeroberfläche</a> und die <a href="https://github.com/rustdesk/doc.rustdesk.com">Dokumentation</a> in deine Muttersprache zu übersetzen.</b>
</p>

Rede mit uns auf: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

RustDesk ist eine in Rust geschriebene Remote-Desktop-Software, die out of the box ohne besondere Konfiguration funktioniert. Du hast die volle Kontrolle über deine Daten und musst dir keine Sorgen um die Sicherheit machen. Du kannst unseren Rendezvous/Relay-Server nutzen, [einen eigenen Server aufsetzen](https://rustdesk.com/server) oder [einen eigenen Server programmieren](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk heißt jegliche Mitarbeit willkommen. Schau dir [CONTRIBUTING-DE.md](CONTRIBUTING-DE.md) an, wenn du Unterstützung beim Start brauchst.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**Programm herunterladen**](https://github.com/rustdesk/rustdesk/releases)

[**Nächtliche Erstellung**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Freie öffentliche Server

Nachfolgend sind die Server gelistet, die du kostenlos nutzen kannst. Es kann sein, dass sich diese Liste immer mal wieder ändert. Falls du nicht in der Nähe einer dieser Server bist, kann es sein, dass deine Verbindung langsam sein wird.
| Standort | Anbieter | Spezifikation |
| --------- | ------------- | ------------------ |
| Südkorea (Seoul) | AWS lightsail | 1 vCPU / 0,5 GB RAM |
| Deutschland | Hetzner | 2 vCPU / 4 GB RAM |
| Deutschland | Codext | 4 vCPU / 8 GB RAM |
| Finnland (Helsinki) | 0x101 Cyber Security | 4 vCPU / 8 GB RAM |
| USA (Ashburn) | 0x101 Cyber Security | 4 vCPU / 8 GB RAM |
| Ukraine (Kiew) | dc.volia (2VM) | 2 vCPU / 4 GB RAM |

## Dev-Container

[![In Dev-Containern öffnen](https://img.shields.io/static/v1?label=Dev%20Container&message=Open&color=blue&logo=visualstudiocode)](https://vscode.dev/redirect?url=vscode://ms-vscode-remote.remote-containers/cloneInVolume?url=https://github.com/rustdesk/rustdesk)

Wenn du VS Code und Docker bereits installiert hast, kannst du auf das Abzeichen oben klicken, um loszulegen. Wenn du darauf klickst, wird VS Code automatisch die Dev-Container-Erweiterung installieren, den Quellcode in ein Container-Volume klonen und einen Dev-Container für die Verwendung aufsetzen.

Weitere Informationen findest du in [DEVCONTAINER-DE.md](DEVCONTAINER-DE.md).

## Abhängigkeiten

Desktop-Versionen verwenden [Sciter](https://sciter.com/) oder Flutter für die GUI, dieses Tutorial ist nur für Sciter.

Bitte lade die dynamische Bibliothek Sciter selbst herunter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Grobe Schritte zum Kompilieren

- Bereite deine Rust-Entwicklungsumgebung und C++-Build-Umgebung vor

- Installiere [vcpkg](https://github.com/microsoft/vcpkg) und füge die Systemumgebungsvariable `VCPKG_ROOT` hinzu

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static`
  - Linux/macOS: `vcpkg install libvpx libyuv opus`

- Nutze `cargo run`

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
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
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

### Wayland zu X11 (Xorg) ändern

RustDesk unterstützt Wayland nicht. Siehe [hier](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/), um Xorg als Standard-GNOME-Sitzung zu nutzen.

## Wayland-Unterstützung

Wayland scheint keine API für das Senden von Tastatureingaben an andere Fenster zu bieten. Daher verwendet RustDesk eine API von einer niedrigeren Ebene, nämlich dem Gerät `/dev/uinput` (Linux-Kernelebene).

Wenn Wayland die kontrollierte Seite ist, müssen Sie wie folgt vorgehen:
```bash
# Dienst uinput starten
$ sudo rustdesk --service
$ rustdesk
```
**Hinweis**: Die Wayland-Bildschirmaufnahme verwendet verschiedene Schnittstellen. RustDesk unterstützt derzeit nur org.freedesktop.portal.ScreenCast.
```bash
$ dbus-send --session --print-reply       \
  --dest=org.freedesktop.portal.Desktop \
  /org/freedesktop/portal/desktop       \
  org.freedesktop.DBus.Properties.Get   \
  string:org.freedesktop.portal.ScreenCast string:version
# Keine Unterstützung
Error org.freedesktop.DBus.Error.InvalidArgs: No such interface “org.freedesktop.portal.ScreenCast”
# Unterstützung
method return time=1662544486.931020 sender=:1.54 -> destination=:1.139 serial=257 reply_serial=2
   variant       uint32 4
```

## Auf Docker kompilieren

Beginne damit, das Repository zu klonen und den Docker-Container zu bauen:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Führe jedes Mal, wenn du das Programm kompilieren musst, folgenden Befehl aus:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Bedenke, dass das erste Kompilieren länger dauern kann, bis die Abhängigkeiten zwischengespeichert sind. Nachfolgende Kompiliervorgänge sind schneller. Wenn du verschiedene Argumente für den Kompilierbefehl angeben musst, kannst du dies am Ende des Befehls an der Position `<OPTIONAL-ARGS>` tun. Wenn du zum Beispiel eine optimierte Releaseversion kompilieren willst, kannst du `--release` am Ende des Befehls anhängen. Das daraus entstehende Programm findest du im Zielordner auf deinem System. Du kannst es mit folgendem Befehl ausführen:

```sh
target/debug/rustdesk
```

Oder, wenn du eine Releaseversion benutzt:

```sh
target/release/rustdesk
```

Bitte stelle sicher, dass du diese Befehle im Stammverzeichnis des RustDesk-Repositorys nutzt. Ansonsten kann es passieren, dass das Programm die Ressourcen nicht finden kann. Bitte bedenke auch, dass andere Cargo-Unterbefehle wie `install` oder `run` aktuell noch nicht unterstützt werden, da sie das Programm innerhalb des Containers starten oder installieren würden, anstatt auf deinem eigentlichen System.

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
