<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Ваш удаленый рабочий стол"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Нам нужна ваша помощь для перевода этого README <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a>
     и документацию RustDesk на ваш родной язык. <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a></b>
</p>

Общение с нами: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Еще одно программное обеспечение для удаленного рабочего стола, написанное на Rust. Работает из коробки, не требует настройки. Вы полностью контролируете свои данные, не беспокоясь о безопасности. Вы можете использовать наш сервер ретрансляции, [настроить свой собственный](https://rustdesk.com/server), или [написать свой](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk приветствует вклад каждого. Ознакомьтесь с [`docs/CONTRIBUTING-RU.md`](CONTRIBUTING-RU.md) в начале работы для понимания.

[**Как работает RustDesk?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[**СКАЧАТЬ ПРИЛОЖЕНИЕ**](https://github.com/rustdesk/rustdesk/releases)

[**ночные сборки (актуальные)**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png" alt="Get it on F-Droid" height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Зависимости

Настольные версии используют [sciter](https://sciter.com/) для графического интерфейса, загрузите динамическую библиотеку sciter самостоятельно.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

Мобильные версии используют Flutter. В будущем мы перенесем настольную версию со Sciter на Flutter.

## Первичные шаги для сборки

- Подготовьте среду разработки Rust и среду сборки C++.

- Установите [vcpkg](https://github.com/microsoft/vcpkg), и правильно установите переменную `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- Запустите `cargo run`

## Как собрать на Linux 

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

### Установка vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Исправление libvpx (для Fedora)

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

### Сборка

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

## Как собрать с помощью Docker

Начните с клонирования репозитория и создания docker-контейнера:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Затем каждый раз, когда вам нужно собрать приложение, запускайте следующую команду:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Обратите внимание, что первая сборка может занять больше времени, прежде чем зависимости будут кэшированы, но последующие сборки будут выполняться быстрее. Кроме того, если вам нужно указать другие аргументы для команды сборки, вы можете сделать это в конце команды в переменной `<OPTIONAL-ARGS>`. Например, если вы хотите создать оптимизированную версию, вы должны запустить приведенную выше команду и в конце строки добавить `--release`. Полученный исполняемый файл будет доступен в целевой папке вашей системы и может быть запущен с помощью:

```sh
target/debug/rustdesk
```

Или, если вы используете исполняемый файл релиза:

```sh
target/release/rustdesk
```

Пожалуйста, убедитесь, что вы запускаете эти команды из корня репозитория RustDesk, иначе приложение не сможет найти необходимые ресурсы. Также обратите внимание, что другие cargo подкоманды, такие как `install` или `run`, в настоящее время не поддерживаются этим методом, поскольку они будут устанавливать или запускать программу внутри контейнера, а не на хосте.

## Структура файлов

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: видеокодек, конфиг, обертка tcp/udp, protobuf, функции fs для передачи файлов и некоторые другие служебные функции
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: захват экрана
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: специфичное для платформы управление клавиатурой/мышью
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: графический пользовательский интерфейс
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: сервисы аудио/буфера обмена/ввода/видео и сетевых подключений
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: одноранговое соединение
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: свяжитесь с [rustdesk-server](https://github.com/rustdesk/rustdesk-server), дождитесь удаленного прямого (обход TCP NAT) или ретранслируемого соединения
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: специфичный для платформы код

> [!Осторожно]
> **Отказ от ответственности за неправомерное использование:** <br>
> Разработчики RustDesk не одобряют и не поддерживают какое-либо неэтичное или незаконное использование данного программного обеспечения. Неправомерное использование, такое как несанкционированный доступ, контроль или вторжение в частную жизнь, строго противоречит нашим правилам. Авторы не несут ответственности за любое неправомерное использование приложения.

## Скриншоты

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
