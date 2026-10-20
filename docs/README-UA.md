<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Ваш віддалений робочий стіл"><br>
  <a href="#кроки-для-збірки">Збірка</a> •
  <a href="#як-зібрати-за-допомогою-docker">Docker</a> •
  <a href="#структура-файлів">Структура</a> •
  <a href="#знімки-екрана">Знімки екрана</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>]<br>
  <b>Нам потрібна ваша допомога з перекладом цього README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">інтерфейсу RustDesk</a> та <a href="https://github.com/rustdesk/doc.rustdesk.com">документації RustDesk</a> вашою рідною мовою</b>
</p>

> [!CAUTION]
> **Застереження щодо неналежного використання:** <br>
> Розробники RustDesk не схвалюють і не підтримують неетичне або незаконне використання цього програмного забезпечення. Неналежне використання, зокрема несанкціонований доступ, керування або втручання в приватність, суворо суперечить нашим правилам. Автори не несуть відповідальності за неналежне використання застосунку.

Спілкуйтеся з нами: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

RustDesk — ще одне рішення для віддаленого робочого столу, написане на Rust. Працює одразу після запуску й не потребує налаштування. Ви повністю контролюєте свої дані та їхню безпеку. Можна використовувати наш rendezvous/relay-сервер, [налаштувати власний](https://rustdesk.com/server) або [написати власний rendezvous/relay-сервер](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk вітає внески від усіх охочих. Дивіться [CONTRIBUTING.md](CONTRIBUTING.md), щоб дізнатися, з чого почати.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**ЗАВАНТАЖИТИ ГОТОВУ ЗБІРКУ**](https://github.com/rustdesk/rustdesk/releases)

[**НІЧНІ ЗБІРКИ**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Залежності

Версії для настільних ОС використовують Flutter або Sciter (застарілий) для графічного інтерфейсу. Ця інструкція стосується лише Sciter, оскільки з ним простіше почати. Для збірки версії на Flutter дивіться наш [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml).

Динамічну бібліотеку Sciter потрібно завантажити окремо.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Кроки для збірки

- Підготуйте середовище розробки Rust і середовище для збирання C++.

- Встановіть [vcpkg](https://github.com/microsoft/vcpkg) і правильно задайте змінну середовища `VCPKG_ROOT`.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- Запустіть `cargo run`.

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
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel
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
git clone --recurse-submodules https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## Як зібрати за допомогою Docker

Спочатку клонуйте репозиторій і зберіть Docker-контейнер:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Після цього щоразу, коли потрібно зібрати застосунок, запускайте таку команду:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Перша збірка може тривати довше, доки залежності не буде кешовано. Наступні збірки виконуватимуться швидше. Якщо потрібно передати додаткові аргументи команді збірки, додайте їх після `rustdesk-builder` у кінці команди `docker run`. Наприклад, для оптимізованої release-збірки додайте `--release`. Отриманий виконуваний файл буде доступний у каталозі `target` вашої системи та його можна запустити так:

```sh
target/debug/rustdesk
```

Або, якщо використовується release-збірка:

```sh
target/release/rustdesk
```

Запускайте ці команди з кореневого каталогу репозиторію RustDesk, інакше застосунок може не знайти потрібні ресурси. Також зверніть увагу, що інші підкоманди cargo, наприклад `install` або `run`, наразі не підтримуються цим способом, оскільки вони встановлюватимуть або запускатимуть програму всередині контейнера, а не на хост-системі.

## Структура файлів

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: відеокодек, конфігурація, обгортка tcp/udp та інші допоміжні функції, спільні із сервером
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf, функції файлової системи для передавання файлів, код клавіатури та платформоспецифічний код, що використовуються лише цим застосунком
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: захоплення екрана
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: платформоспецифічне керування клавіатурою та мишею
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: реалізація копіювання та вставлення файлів для Windows, Linux і macOS
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: застарілий інтерфейс Sciter (deprecated)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: служби аудіо, буфера обміну, введення та відео, а також мережеві з'єднання
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: встановлення однорангового з'єднання
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: зв'язок із [rustdesk-server](https://github.com/rustdesk/rustdesk-server), очікування прямого віддаленого з'єднання (TCP hole punching) або з'єднання через ретранслятор
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: платформоспецифічний код
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: код Flutter для настільних і мобільних платформ

## Знімки екрана

![Менеджер підключень](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Підключення до ПК з Windows](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![Передавання файлів](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP-тунелювання](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)

