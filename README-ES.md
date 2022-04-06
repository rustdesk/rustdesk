<p align="center">
  <img src="logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#servidores-gratis-de-uso-público">Servidores</a> •
  <a href="#pasos-para-compilar-desde-el-inicio">Compilar</a> •
  <a href="#como-compilar-con-docker">Docker</a> •
  <a href="#estructura-de-archivos">Estructura</a> •
  <a href="#captura-de-pantalla">Captura de pantalla</a><br>
  [<a href="README-ZH.md">中文</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>]<br>
  <b>Necesitamos tu ayuda para traducir este README a tu idioma</b>
</p>

Chat with us: [Discord](https://discord.gg/nDceKgxnkV) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Otro software de escritorio remoto, escrito en Rust. Funciona de forma inmediata, sin necesidad de configuración. Tienes el control total de sus datos, sin preocupaciones sobre la seguridad. Puedes utilizar nuestro servidor de rendezvous/relay, [set up your own](https://rustdesk.com/blog/id-relay-set/), o [escribir tu propio servidor rendezvous/relay](https://github.com/rustdesk/rustdesk-server-demo).

RustDesk agradece la contribución de todo el mundo. Ve [`CONTRIBUTING.md`](CONTRIBUTING.md) para ayuda inicial.

[**DESCARGA DE BINARIOS**](https://github.com/rustdesk/rustdesk/releases)

## Servidores gratis de uso público

A continuación se muestran los servidores que está utilizando de forma gratuita, puede cambiar en algún momento. Si no estás cerca de uno de ellos, tu red puede ser lenta.

- Seoul, AWS lightsail, 1 VCPU/0.5G RAM
- Singapore, Vultr, 1 VCPU/1G RAM
- Dallas, Vultr, 1 VCPU/1G RAM

## Dependencies

La versión Desktop usa [sciter](https://sciter.com/) para GUI, por favor bajate la librería sciter tu mismo..

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Pasos para compilar desde el inicio

- Prepara el entono de desarrollode Rust y el entorno de compilación de C++ y Rust.

- Instala [vcpkg](https://github.com/microsoft/vcpkg), y configura la variable de entono `VCPKG_ROOT` correctamente.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
  - Linux/Osx: vcpkg install libvpx libyuv opus

- run `cargo run`

## Como compilar en linux

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
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pulseaudio
```

### Install vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
```

### Soluciona libvpx (For Fedora)

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
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
cargo run
```

### Cambia Wayland a X11 (Xorg)

RustDesk no soporta Wayland. Comprueba [aquí](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) para configurar Xorg en la sesión por defecto de GNOME.

## Como compilar con Docker

Empieza clonando el repositorio y compilando el contenedor de docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Entonces, cada vez que necesites compilar una modificación, ejecuta el siguiente comando:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Ten en cuenta que la primera compilación puede tardar más tiempo antes de que las dependencias se almacenen en la caché, las siguientes compilaciones serán más rápidas. Además, si necesitas especificar diferentes argumentos a la orden de compilación, puede hacerlo al final de la linea de comandos en el apartado`<OPTIONAL-ARGS>`. Por ejemplo, si desea compilar una versión optimizada para publicación, deberá ejecutar el comando anterior seguido de `---release`. El ejecutable resultante estará disponible en la carpeta de destino en su sistema, y puede ser ejecutado con:

```sh
target/debug/rustdesk
```

O si estas ejecutando una versión para su publicación:

```sh
target/release/rustdesk
```

Por favor, asegurate de que estás ejecutando estos comandos desde la raíz del repositorio de RustDesk, de lo contrario la aplicación puede ser incapaz de encontrar los recursos necesarios. También hay que tener en cuenta que otros subcomandos de carga como `install` o `run` no estan actualmente soportados via este metodo y podrían requerir ser instalados dentro del contenedor y no en el host.

## Estructura de archivos

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, configuración, tcp/udp wrapper, protobuf, fs funciones para transferencia de ficheros, y alguna función de utilidad.
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: captura de pantalla
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: control específico por cada plataforma para el teclado/ratón
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: sonido/portapapeles/entrada/servicios de video, y conexiones de red
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: iniciar una conexión "peer to peer"
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Comunicación con [rustdesk-server](https://github.com/rustdesk/rustdesk-server), esperar la conexión remota directa ("TCP hole punching") o conexión indirecta ("relayed")
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: código específico de cada plataforma

## Captura de pantalla

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
