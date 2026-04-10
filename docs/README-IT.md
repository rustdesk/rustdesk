<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - il tuo desktop remoto"><br>
  <a href="#server-pubblici-gratuiti">Server</a> •
  <a href="#passaggi-per-la-compilazione">Compilazione</a> •
  <a href="#come-compilare-con-docker">Docker</a> •
  <a href="#struttura-dei-file">Struttura</a> •
  <a href="#schermate">Schermate</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>]<br>
  <b>Abbiamo bisogno del tuo aiuto per tradurre questo file README e la <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">UI RustDesk</a> nella tua lingua nativa</b>
</p>

Chatta con noi su: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Funzionalit%C3%A0%20Avanzate-blue)](https://rustdesk.com/pricing.html)

[![Bounties aperti](https://img.shields.io/endpoint?url=https%3A%2F%2Fconsole.algora.io%2Fapi%2Fshields%2Frustdesk%2Fbounties%3Fstatus%3Dopen)](https://console.algora.io/org/rustdesk/bounties?status=open)

Ancora un altro software per il controllo remoto del desktop, scritto in Rust. Funziona immediatamente, nessuna configurazione richiesta. Hai il pieno controllo dei tuoi dati, senza preoccupazioni per la sicurezza. Puoi usare il nostro server rendezvous/relay, [configurare il tuo server](https://rustdesk.com/server) o [realizzare il tuo server rendezvous/relay](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk accoglie il contributo di tutti. Per ulteriori informazioni su come iniziare a contribuire, vedi [CONTRIBUTING.md](CONTRIBUTING-IT.md).

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**SCARICA PROGRAMMA**](https://github.com/rustdesk/rustdesk/releases)

[**SCARICA NIGHTLY**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Dipendenze

Le versioni desktop utilizzano Flutter o Sciter (deprecato) per l'interfaccia utente, questo tutorial è solo per Sciter, poiché è più facile per iniziare. Controlla il nostro [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) per la compilazione della versione Flutter.

Scarica la libreria dinamica Sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Passaggi per la compilazione

- Prepara l'ambiente per lo sviluppo e compilazione in Rust e C++

- Installa [vcpkg](https://github.com/microsoft/vcpkg), e imposta correttamente la variabile d'ambiente `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- Esegui `cargo run`

## [Build](https://rustdesk.com/docs/en/dev/build/)

## Come compilare in Linux

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

### Installa vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Correzione libvpx (per Fedora)

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

### Compilazione

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

## Come compilare con Docker

Clona il repository e compila i container docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Quindi, ogni volta che devi compilare l'applicazione, esegui il seguente comando:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Tieni presente che la prima build potrebbe richiedere più tempo prima che le dipendenze vengano memorizzate nella cache, le build successive saranno più veloci. Inoltre, se hai bisogno di specificare argomenti diversi per il comando build, puoi farlo alla fine del comando nella posizione `<OPTIONAL-ARGS>`. Ad esempio, se vuoi creare una versione di rilascio ottimizzata, esegui il comando precedentemente indicato seguito da `--release`. L'eseguibile generato sarà creato nella cartella destinazione del sistema e può essere eseguito con:

```sh
target/debug/rustdesk
```

Oppure, se stai avviando un eseguibile di rilascio:

```sh
target/release/rustdesk
```

Assicurati di eseguire questi comandi dalla radice del repository RustDesk, altrimenti l'applicazione potrebbe non essere in grado di trovare le risorse richieste. Nota inoltre che altri sottocomandi cargo come `install` o `run` non sono attualmente supportati tramite questo metodo poiché installerebbero o eseguirebbero il programma all'interno del container anziché nell'host.

## Struttura dei file

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: codec video, config, wrapper tcp/udp, protobuf, funzioni per il trasferimento file, e altre funzioni utili.
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: cattura dello schermo
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: controllo tastiera/mouse specifico della piattaforma
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: implementazione del copia e incolla dei file per Windows, Linux, macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: Sciter UI obsoleto (deprecato)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: servizi audio/appunti/input/video e connessioni di rete
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: avvio di una connessione peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: comunica con [rustdesk-server](https://github.com/rustdesk/rustdesk-server), attende la connessione remota diretta (TCP hole punching) oppure indiretta (relayed)
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: codice specifico della piattaforma
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: codice Flutter per desktop e mobile
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript per client web Flutter

> [!Attenzione]
> **Dichiarazione di non responsabilità per uso improprio:** <br>
> Gli sviluppatori di RustDesk non approvano né supportano alcun uso non etico o illegale di questo software. L'uso improprio, come l'accesso non autorizzato, il controllo o l'invasione della privacy, è strettamente contro le nostre linee guida. Gli autori non sono responsabili per qualsiasi uso improprio dell'applicazione.

## Schermate

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
