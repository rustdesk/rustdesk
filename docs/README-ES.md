<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#servidores-gratis-de-uso-público">Servidores</a> •
  <a href="#pasos-para-compilar-desde-el-inicio">Compilar</a> •
  <a href="#como-compilar-con-docker">Docker</a> •
  <a href="#estructura-de-archivos">Estructura</a> •
  <a href="#capturas-de-pantalla">Capturas de pantalla</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Necesitamos tu ayuda para traducir este README a tu idioma</b>
</p>

> [!Caution]
> **Descargo de responsabilidad por mal uso:** <br>
> Los desarrolladores de RustDesk no aprueban ni apoyan ningún uso no ético o ilegal de este software. El mal uso, como el acceso no autorizado, el control o la invasión de la privacidad, va estrictamente en contra de nuestras directrices. Los autores no se hacen responsables de ningún uso indebido de la aplicación.

Chatea con nosotros: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Caracter%C3%ADsticas%20Avanzadas-blue)](https://rustdesk.com/pricing.html)

Otro software de escritorio remoto, escrito en Rust. Funciona de forma inmediata, sin necesidad de configuración. Tienes el control total de tus datos, sin preocupaciones sobre la seguridad. Puedes utilizar nuestro servidor de rendezvous/relay, [instalar el tuyo](https://rustdesk.com/server), o [escribir tu propio servidor rendezvous/relay](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk agradece la contribución de todo el mundo. Lee [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) para ayuda para empezar.

[**¿Cómo funciona rustdesk?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[**DESCARGA DE BINARIOS**](https://github.com/rustdesk/rustdesk/releases)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Dependencias

Las versiones de escritorio utilizan Flutter o Sciter (obsoleto) para GUI, este tutorial es sólo para Sciter, ya que es más fácil y más amigable para empezar. Echa un vistazo a nuestro [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) para la construcción de la versión Flutter.

Por favor descarga la librería dinámica de Sciter tú mismo.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Pasos para compilar desde el inicio

- Prepara el entorno de desarrollo de Rust y el entorno de compilación de C++ y Rust.

- Instala [vcpkg](https://github.com/microsoft/vcpkg), y configura la variable de entono `VCPKG_ROOT` correctamente.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/Osx: vcpkg install libvpx libyuv opus aom

- Corre `cargo run`

## Como compilar en linux

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

### Instala vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Arregla libvpx (Para Fedora)

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

### Compila

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

## Como compilar con Docker

Empieza clonando el repositorio y compilando el contenedor de docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Entonces, cada vez que necesites compilar la aplicación, ejecuta el siguiente comando:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Ten en cuenta que la primera compilación puede tardar más tiempo antes de que las dependencias se almacenen en la caché, las siguientes compilaciones serán más rápidas. Además, si necesitas especificar diferentes argumentos al comando de compilación, puedes hacerlo al final del comando en la posición `<OPTIONAL-ARGS>`. Por ejemplo, si deseas compilar una versión optimizada para publicación, deberas ejecutar el comando anterior seguido de `--release`. El ejecutable resultante estará disponible en la carpeta de destino en tu sistema, y puede ser ejecutado con:

```sh
target/debug/rustdesk
```

O si estas ejecutando una versión para su publicación:

```sh
target/release/rustdesk
```

Por favor, asegurate de que estás ejecutando estos comandos desde la raíz del repositorio de RustDesk, de lo contrario la aplicación puede ser incapaz de encontrar los recursos necesarios. También ten en cuenta que otros subcomandos de cargo como `install` o `run` no estan actualmente soportados usando este metodo, ya que instalarían o ejecutarían el programa dentro del contenedor en lugar del host.

## Estructura de archivos

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**:  codec de video, configuración, tcp/udp wrapper, protobuf, funciones para transferencia de archivos, y otras funciones de utilidad.
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: captura de pantalla
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: control del teclado/mouse especificos de cada plataforma
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: sonido/portapapeles/input/servicios de video, y conexiones de red
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: iniciar una conexión "peer to peer"
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Comunicación con [rustdesk-server](https://github.com/rustdesk/rustdesk-server), esperar la conexión remota directa ("TCP hole punching") o conexión indirecta ("relayed")
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: código específico de cada plataforma
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter, código para moviles
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Javascript para el cliente web Flutter

> [!Precaución]
> **Descargo de responsabilidad por uso indebido:** <br>
> Los desarrolladores de RustDesk no aprueban ni apoyan ningún uso no ético o ilegal de este software. El uso indebido, como el acceso no autorizado, el control o la invasión de la privacidad, está estrictamente en contra de nuestras directrices. Los autores no son responsables de ningún uso indebido de la aplicación.

## Capturas de pantalla

![Connection Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connected to a Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![File Transfer](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
