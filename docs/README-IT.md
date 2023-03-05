<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#server-pubblici-gratuiti">Servers</a> •
  <a href="#passaggi-per-la-compilazione">Compilazione</a> •
  <a href="#come-compilare-con-docker">Docker</a> •
  <a href="#struttura-dei-file">Struttura</a> •
  <a href="#screenshots">Screenshots</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Abbiamo bisogno del tuo aiuto per tradurre questo README e la <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> nella tua lingua nativa</b>
</p>

Chatta con noi: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Ancora un altro software per il controllo remoto del desktop, scritto in Rust. Funziona immediatamente, nessuna configurazione richiesta. Hai il pieno controllo dei tuoi dati, senza preoccupazioni per la sicurezza. Puoi utilizzare il nostro server rendezvous/relay, [configurare il tuo](https://rustdesk.com/server) o [scrivere il tuo rendezvous/relay server](https://github.com/rustdesk/rustdesk-server-demo).

RustDesk accoglie il contributo di tutti. Per ulteriori informazioni su come inizare a contribuire, vedere [`docs/CONTRIBUTING.md`](CONTRIBUTING.md).

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## Server pubblici gratuiti

Qui sotto trovate i server che possono essere usati gratuitamente, la lista potrebbe cambiare nel tempo. Se non si è vicini a uno di questi server, la vostra connessione potrebbe essere lenta.
| Posizione | Vendor | Specifiche |
| --------- | ------------- | ------------------ |
| Seoul | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Germany | Hetzner | 2 vCPU / 4GB RAM |
| Germany | Codext | 4 vCPU / 8GB RAM |
| Finland (Helsinki) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| USA (Ashburn) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |

## Dipendenze

La versione Desktop utilizza [sciter](https://sciter.com/) per la GUI, per favore scarica sciter dynamic library.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Passaggi per la compilazione

- Prepara l'ambiente per lo sviluppo e compilazione in Rust e C++

- Installa [vcpkg](https://github.com/microsoft/vcpkg), e imposta correttamente la variabile d'ambiente `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus

- Esegui `cargo run`

## Come compilare su Linux

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

### Installare vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
```

### Fix libvpx (Per Fedora)

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

### Cambiare Wayland a X11 (Xorg)

RustDesk non supporta Wayland. Controlla [questo](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) per configurare Xorg come sessione di default di GNOME.

## Come compilare con Docker

Cominciare clonando il repository e compilare i container docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Quindi, ogni volta che devi compilare l'applicazione, esegui il comando seguente:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Tieni presente che la prima build potrebbe richiedere più tempo prima che le dipendenze vengano memorizzate nella cache, le build successive saranno più veloci. Inoltre, se hai bisogno di specificare argomenti diversi per il comando build, puoi farlo alla fine del comando nella posizione `<OPTIONAL-ARGS>`. Ad esempio, se si desidera creare una versione di rilascio ottimizzata, eseguire il comando sopra seguito da `--release`. L'eseguibile generato sarà creato nella cartella di destinazione del proprio sistema e può essere eseguito con:

```sh
target/debug/rustdesk
```

Oppure, se si sta eseguendo un eseguibile di rilascio:

```sh
target/release/rustdesk
```

Assicurati di eseguire questi comandi dalla radice del repository RustDesk, altrimenti l'applicazione potrebbe non essere in grado di trovare le risorse richieste. Notare inoltre che altri sottocomandi cargo come `install` o `run` non sono attualmente supportati tramite questo metodo poiché installerebbero o eseguirebbero il programma all'interno del container anziché nell'host.

## Struttura dei file

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs funzioni per il trasferimento file, e altre funzioni utili.
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: cattura dello schermo
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: controllo tastiera/mouse specifico della piattaforma
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: servizi audio/appunti/input/video e connessioni di rete
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: avviare una connessione peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Comunica con [rustdesk-server](https://github.com/rustdesk/rustdesk-server), attende la connessione remota diretta (TCP hole punching) oppure indiretta (relayed)
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: codice specifico della piattaforma

## Screenshots

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
