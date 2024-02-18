<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Ваша віддалена стільниця"><br>
  <a href="#безкоштовні-загальнодоступні-сервери">Сервери</a> •
  <a href="#кроки-для-збірки">Збирання</a> •
  <a href="#як-зібрати-за-допомогою-docker">Docker</a> •
  <a href="#структура-файлів">Структура</a> •
  <a href="#знімки">Знімки</a><br>
  [<a href="../README.md">English</a>] | [<a href="docs/README-CS.md">česky</a>] | [<a href="docs/README-ZH.md">中文</a>] | [<a href="docs/README-HU.md">Magyar</a>] | [<a href="docs/README-ES.md">Español</a>] | [<a href="docs/README-FA.md">فارسی</a>] | [<a href="docs/README-FR.md">Français</a>] | [<a href="docs/README-DE.md">Deutsch</a>] | [<a href="docs/README-PL.md">Polski</a>] | [<a href="docs/README-ID.md">Indonesian</a>] | [<a href="docs/README-FI.md">Suomi</a>] | [<a href="docs/README-ML.md">മലയാളം</a>] | [<a href="docs/README-JP.md">日本語</a>] | [<a href="docs/README-NL.md">Nederlands</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Русский</a>] | [<a href="docs/README-PTBR.md">Português (Brasil)</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">한국어</a>] | [<a href="docs/README-AR.md">العربي</a>] | [<a href="docs/README-VN.md">Tiếng Việt</a>] | [<a href="docs/README-DA.md">Dansk</a>] | [<a href="docs/README-GR.md">Ελληνικά</a>] | [<a href="docs/README-TR.md">Türkçe</a>]<br>
  <b>Нам потрібна ваша допомога для перекладу цього README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">інтерфейсу</a> та <a href="https://github.com/rustdesk/doc.rustdesk.com">документації</a> RustDesk на вашу рідну мову</B>
</p>

Спілкування з нами: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

[![Open Bounties](https://img.shields.io/endpoint?url=https%3A%2F%2Fconsole.algora.io%2Fapi%2Fshields%2Frustdesk%2Fbounties%3Fstatus%3Dopen)](https://console.algora.io/org/rustdesk/bounties?status=open)

Ще один застосунок для віддаленого керування стільницею, написаний на Rust. Працює з коробки, не потребує налаштування. Ви повністю контролюєте свої дані, не турбуючись про безпеку. Ви можете використовувати наш сервер ретрансляції, [налаштувати свій власний](https://rustdesk.com/server), або [написати свій власний сервер ретрансляції](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk вітає внесок кожного. Ознайомтеся з [CONTRIBUTING.md](docs/CONTRIBUTING.md), щоб отримати допомогу на початковому етапі.

[**ЧаПи**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**ЗАВАНТАЖЕННЯ ЗАСТОСУНКУ**](https://github.com/rustdesk/rustdesk/releases)

[**НІЧНІ ЗБІРКИ**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Безкоштовні загальнодоступні сервери

Нижче наведені сервери, для безкоштовного використання, вони можуть змінюватися з часом. Якщо ви не перебуваєте поруч з одним із них, ваша мережа може працювати повільно.
| Місцезнаходження | Постачальник | Технічні характеристики |
| --------- | ------------- | ------------------ |
| Німеччина | [Hetzner](https://www.hetzner.com) | 2 vCPU / 4GB RAM |
| Україна (Київ) | [dc.volia](https://dc.volia.com) | 2 vCPU / 4GB RAM |

## Dev Container

[![Open in Dev Containers](https://img.shields.io/static/v1?label=Dev%20Container&message=Open&color=blue&logo=visualstudiocode)](https://vscode.dev/redirect?url=vscode://ms-vscode-remote.remote-containers/cloneInVolume?url=https://github.com/rustdesk/rustdesk)

Якщо у вас уже встановлено VS Code та Docker, ви можете натиснути значок вище, щоб розпочати. Клацання призведе до того, що VS Code автоматично встановить розширення Dev Containers, якщо це необхідно, клонує вихідний код у том контейнера та розгорне контейнер dev для використання.

Дивіться [DEVCONTAINER.md](docs/DEVCONTAINER.md) для додаткової інформації

## Залежності

Стільничні версії використовують Flutter чи Sciter (застаріле) для графічного інтерфейсу. Ця інструкція лише для Sciter, оскільки він є більш простим та дружнім для початківців. Перегляньте [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) для збірки версії на Flutter.

Будь ласка, завантажте динамічну бібліотеку Sciter самостійно.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Кроки для збірки

- Підготуйте середовище розробки Rust і середовище збирання C++.

- Встановіть [vcpkg](https://github.com/microsoft/vcpkg), і правильно встановіть змінну `VCPKG_ROOT`.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- Запустіть `cargo run`

## [Збирання](https://rustdesk.com/docs/en/dev/build/)

## Як зібрати на Linux 

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

### Встановлення vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Виправлення libvpx (для Fedora)

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

### Збирання

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

## Як зібрати за допомогою Docker

Почніть з клонування сховища та створення docker-контейнера:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Надалі щоразу, коли вам буде потрібно зібрати застосунок, запускайте таку команду:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Зверніть увагу, що перша збірка може зайняти більше часу, перш ніж залежності будуть кешовані, але наступні збірки будуть виконуватися швидше. Крім того, якщо вам потрібно вказати інші аргументи для команди збірки, ви можете зробити це в кінці команди у змінній `<OPTIONAL-ARGS>`. Наприклад, якщо ви хочете створити оптимізовану версію, ви маєте запустити наведену вище команду і в кінці рядка додати `--release`. Отриманий виконуваний файл буде доступний у цільовій папці вашої системи і може бути запущений за допомогою:

```sh
target/debug/rustdesk
```

Або, якщо ви використовуєте виконуваний файл релізу:

```sh
target/release/rustdesk
```

Будь ласка, переконайтеся, що ви запускаєте ці команди з кореня сховища RustDesk, інакше додаток не зможе знайти необхідні ресурси. Також зверніть увагу, що інші cargo підкоманди, такі як `install` або `run`, наразі не підтримуються цим методом, оскільки вони будуть встановлювати або запускати програму всередині контейнера, а не на хості.

## Структура файлів

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: відеокодек, конфіг, обгортка tcp/udp, protobuf, функції fs для передавання файлів і деякі інші службові функції
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: захоплення екрана
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: специфічне для платформи керування клавіатурою/мишею
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: реалізація копіювання та вставлення файлів для Windows, Linux, macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: графічний інтерфейс користувача
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: сервіси аудіо/буфера обміну/вводу/відео та мережевих підключень
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: однорангове з'єднання
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: комунікація з [rustdesk-server](https://github.com/rustdesk/rustdesk-server), очікування віддаленого прямого (обхід TCP NAT) або ретрансльованого з'єднання
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: специфічний для платформи код
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: код Flutter для мобільних пристроїв 
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript для Flutter веб клієнту

## Знімки

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
