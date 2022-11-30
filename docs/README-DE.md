<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#kostenlose-öffentliche-server">Server</a> •
  <a href="#die-groben-schritte-zum-kompilieren">Kompilieren</a> •
  <a href="#auf-docker-kompilieren">Docker</a> •
  <a href="#dateistruktur">Dateistruktur</a> •
  <a href="#screenshots">Screenshots</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>]<br>
  <b>Wir brauchen deine Hilfe um diese README Datei zu verbessern und aktualisieren</b>
</p>

Rede mit uns: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Das hier ist ein Programm was, man nutzen kann, um einen Computer fernzusteuern, es wurde in Rust geschrieben. Es funktioniert ohne Konfiguration oder ähnliches, man kann es einfach direkt nutzen. Du hast volle Kontrolle über deine Daten und brauchst dir daher auch keine Sorgen um die Sicherheit dieser Daten zu machen. Du kannst unseren Rendezvous/Relay Server nutzen, [einen eigenen Server eröffnen](https://rustdesk.com/server) oder [einen neuen eigenen Server programmieren](https://github.com/rustdesk/rustdesk-server-demo).

RustDesk heißt jegliche Mitarbeit willkommen. Schau dir [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) an, wenn du Hilfe brauchst für den Start.

[**PROGRAMM DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## Kostenlose öffentliche Server

Hier sind die Server, die du kostenlos nutzen kannst, es kann sein das sich diese Liste immer mal wieder ändert. Falls du nicht in der Nähe einer dieser Server bist, kann es sein, dass deine Verbindung langsam sein wird.

| Standort  | Serverart     | Spezifikationen    | Kommentare |
| --------- | ------------- | ------------------ | ---------- |
| Seoul     | AWS lightsail | 1 vCPU / 0.5GB RAM |            |
| Germany   | Codext        | 2 vCPU / 4GB RAM   |
| Germany   | Hetzner       | 4 vCPU / 8GB RAM   |
| Finland (Helsinki)   | 0x101 Cyber Security       | 4 vCPU / 8GB RAM   |
| USA (Ashburn)   | 0x101 Cyber Security       | 4 vCPU / 8GB RAM   |

## Abhängigkeiten

Die Desktop-Versionen nutzen [Sciter](https://sciter.com/) für die Oberfläche, bitte lade die dynamische Sciter Bibliothek selbst herunter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Die groben Schritte zum Kompilieren

- Bereite deine Rust Entwicklungsumgebung und C++ Entwicklungsumgebung vor

- Installiere [vcpkg](https://github.com/microsoft/vcpkg) und füge die `VCPKG_ROOT` Systemumgebungsvariable hinzu

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static`
  - Linux/MacOS: `vcpkg install libvpx libyuv opus`

- Nutze `cargo run`

## Kompilieren auf Linux

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

### libvpx reparieren (Für Fedora)

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
cargo run
```

### Ändere Wayland zu X11 (Xorg)

RustDesk unterstützt "Wayland" nicht. Siehe [hier](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) um Xorg als Standard GNOME Session zu nutzen.

## Auf Docker Kompilieren

Beginne damit das Repository zu klonen und den Docker Container zu bauen:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Jedes Mal, wenn du das Programm Kompilieren musst, nutze diesen Befehl:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Bedenke, dass das erste Mal Kompilieren länger dauern kann, da die Abhängigkeiten erst kompiliert werden müssen bevor sie zwischengespeichert werden können. Darauf folgende Kompiliervorgänge werden schneller sein. Falls du zusätzliche oder andere Argumente für den Kompilierbefehl angeben musst, kannst du diese am Ende des Befehls an der `<OPTIONAL-ARGS>` Position machen. Wenn du zum Beispiel eine optimierte Releaseversion kompilieren willst, kannst du das tun, indem du `--release` am Ende des Befehls anhängst. Das daraus entstehende Programm kannst du im “target” Ordner auf deinem System finden. Du kannst es mit folgenden Befehlen ausführen:

```sh
target/debug/rustdesk
```

Oder, wenn du eine Releaseversion benutzt:

```sh
target/release/rustdesk
```

Bitte gehe sicher, dass du diese Befehle vom Stammverzeichnis vom RustDesk Repository nutzt, sonst kann es passieren, dass das Programm die Ressourcen nicht finden kann. Bitte bedenke auch, dass Unterbefehle von Cargo, wie z. B. `install` oder `run` aktuell noch nicht unterstützt werden, da sie das Programm innerhalb des Containers starten oder installieren würden, anstatt auf deinem eigentlichen System.

## Dateistruktur

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: Video Codec, Konfiguration, TCP/UDP Wrapper, Protokoll Puffer, fs Funktionen für Dateitransfer, und ein paar andere nützliche Funktionen
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: Bildschirmaufnahme
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: Plattformspezifische Maus und Tastatur Steuerung
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: Audio/Zwischenablage/Eingabe/Videodienste und Netzwerk Verbindungen
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: Starten einer Peer-Verbindung
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Mit [rustdesk-server](https://github.com/rustdesk/rustdesk-server) kommunizieren, für Verbindung von außen warten, direkt (TCP hole punching) oder weitergeleitet
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: Plattformspezifischer Code

## Screenshots

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
