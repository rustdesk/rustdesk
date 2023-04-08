<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Twój zdalny pulpit"><br>
  <a href="#darmowe-serwery-publiczne">Serwery</a> •
  <a href="#podstawowe-kroki-do-kompilacji">Kompilacja</a> •
  <a href="#jak-kompilować-za-pomocą-dockera">Docker</a> •
  <a href="#struktura-plików">Struktura</a> •
  <a href="#migawkisnapshoty">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Potrzebujemy twojej pomocy w tłumaczeniu README na twój ojczysty język</b>
</p>

Porozmawiaj z nami na: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Kolejny program do zdalnego pulpitu, napisany w Rust. Działa od samego początku, nie wymaga konfiguracji. Masz pełną kontrolę nad swoimi danymi, bez obaw o bezpieczeństwo. Możesz skorzystać z naszego darmowego serwera publicznego , [skonfigurować własny](https://rustdesk.com/server), lub [napisać własny serwer](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png) 

RustDesk zaprasza do współpracy każdego. Zobacz [`docs/CONTRIBUTING-PL.md`](CONTRIBUTING-PL.md) pomoc w uruchomieniu programu.

[**PYTANIA I ODPOWIEDZI (FAQ)**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**POBIERANIE BINARIÓW**](https://github.com/rustdesk/rustdesk/releases)

[**WERSJE TESTOWE (NIGHTLY)**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Darmowe Serwery Publiczne

Poniżej znajdują się serwery, z których można korzystać za darmo, może się to zmienić z upływem czasu. Jeśli nie znajdujesz się w pobliżu jednego z nich, Twoja prędkość połączenia może być niska.
| Lokalizacja | Dostawca | Specyfikacja |
| --------- | ------------- | ------------------ |
| Korea Płd. (Seul) | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Niemcy | Hetzner | 2 vCPU / 4GB RAM |
| Niemcy | Codext | 4 vCPU / 8GB RAM |
| Finlandia (Helsinki) | [Netlock](https://netlockendpoint.com) | 4 vCPU / 8GB RAM |
| USA (Ashburn) | [Netlock](https://netlockendpoint.com) | 4 vCPU / 8GB RAM |
| Ukraina (Kijów) | [dc.volia](https://dc.volia.com) | 2 vCPU / 4GB RAM |

## Konterner Programisty (Dev Container)

[![Otwórz w Kontenerze programisty](https://img.shields.io/static/v1?label=Dev%20Container&message=Open&color=blue&logo=visualstudiocode)](https://vscode.dev/redirect?url=vscode://ms-vscode-remote.remote-containers/cloneInVolume?url=https://github.com/rustdesk/rustdesk)

Jeżeli masz zainstalowany VS Code i Docker, możesz kliknąć w powyższy link, aby rozpocząć. Kliknięcie spowoduje automatyczną instalację rozszrzenia Kontenera Programisty w VS Code (jeżeli wymagany), sklonuje kod źródłowy do kontenera, i przygotuje kontener do użycia.

Więcej informacji w pliku [DEVCONTAINER-PL.md](docs/DEVCONTAINER-PL.md) for more info.

## Zależności

Wersje desktopowe używają [sciter](https://sciter.com/) dla GUI, proszę pobrać samodzielnie bibliotekę sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Podstawowe kroki do kompilacji

- Przygotuj środowisko programistyczne Rust i środowisko programowania C++

- Zainstaluj [vcpkg](https://github.com/microsoft/vcpkg), i ustaw prawidłowo zmienną `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus

- uruchom `cargo run`

## Jak Kompilować na Linuxie

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

### Zainstaluj vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
```

### Popraw libvpx (Dla Fedora)

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

### Kompilacja

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

### Zmień Wayland na X11 (Xorg)

RustDesk nie obsługuje Waylanda. Sprawdź [tutaj](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/), jak skonfigurować Xorg jako domyślną sesję GNOME.

## Wspracie Wayland

Wygląda na to, że Wayland nie wspiera żadnego API do wysyłania naciśnięć klawiszy do innych okien. Dlatego rustdesk używa API z niższego poziomu, urządzenia o nazwie `/dev/uinput` (poziom jądra Linux).

Gdy po stronie kontrolowanej pracuje Wayland, musisz uruchomić program w następujący sposób:
```bash
# Start uinput service
$ sudo rustdesk --service
$ rustdesk
```
**Uwaga**: Nagrywanie ekranu Wayland wykorzystuje różne interfejsy. RustDesk obecnie obsługuje tylko org.freedesktop.portal.ScreenCast.
```bash
$ dbus-send --session --print-reply       \
  --dest=org.freedesktop.portal.Desktop \
  /org/freedesktop/portal/desktop       \
  org.freedesktop.DBus.Properties.Get   \
  string:org.freedesktop.portal.ScreenCast string:version
# Not support
Error org.freedesktop.DBus.Error.InvalidArgs: No such interface “org.freedesktop.portal.ScreenCast”
# Support
method return time=1662544486.931020 sender=:1.54 -> destination=:1.139 serial=257 reply_serial=2
   variant       uint32 4
```

## Jak kompilować za pomocą Dockera

Rozpocznij od sklonowania repozytorium i stworzenia kontenera docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Następnie, za każdym razem, gdy potrzebujesz skompilować aplikację, uruchom następujące polecenie:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Zauważ, że pierwsza kompilacja może potrwać dłużej zanim zależności zostaną zbuforowane, kolejne będą szybsze. Dodatkowo, jeśli potrzebujesz określić inne argumenty dla polecenia budowania, możesz to zrobić na końcu komendy w miejscu `<OPTIONAL-ARGS>`. Na przykład, jeśli chciałbyś zbudować zoptymalizowaną wersję wydania, uruchomiłbyś powyższą komendę a następnie `--release`. Powstały plik wykonywalny będzie dostępny w folderze docelowym w twoim systemie, i może być uruchomiony z:

```sh
target/debug/rustdesk
```

Lub jeśli uruchamiasz plik wykonywalny wersji:

```sh
target/release/rustdesk
```

Upewnij się, że uruchamiasz te polecenia z katalogu głównego repozytorium RustDesk, w przeciwnym razie aplikacja może nie być w stanie znaleźć wymaganych zasobów. Należy również pamiętać, że inne podpolecenia ładowania, takie jak `install` lub `run` nie są obecnie obsługiwane za pomocą tej metody, ponieważ instalowałyby lub uruchamiały program wewnątrz kontenera zamiast na hoście.

## Struktura plików

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: kodek wideo, konfiguracja, obsługa tcp/udp, protobuf, funkcje systemu plików do transferu plików i kilka innych funkcji użytkowych
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: przechwytywanie ekranu
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: specyficzne dla danej platformy sterowanie klawiaturą/myszą
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: audio/schowek/wejście(input)/wideo oraz połączenia sieciowe
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: uruchamia połączenie bezpośrednie
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Komunikacja z [rustdesk-server](https://github.com/rustdesk/rustdesk-server), czekanie na bezpośrednie (odpytywanie TCP) lub przekazywane połączenie
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: kod specyficzny dla danej platformy
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: kod Flutter dla urządzeń mobilnych
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript dla Flutter - klient web

## Zrzuty ekranu

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
