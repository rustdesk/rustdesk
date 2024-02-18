<p dir="rtl" align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#تصاویر-محیط-نرم افزار">تصاویر محیط نرم‌افزار</a> •
  <a href="#ساختار-پوشه-ها">ساختار</a> •
  <a href="#نحوه-ساخت-با-داکر">داکر</a> •
  <a href="#ساخت">ساخت</a> •
  <a href="#سرورهای-عمومی-رایگان">سرور</a>
</p>
<p align="center" dir="auto">[<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]</p>
<p dir="rtl" align="center"><b>برای ترجمه این سند (README)، <a href="https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/lang" dir="rtl">رابط کاربری RustDesk</a>، <a href="https://github.com/Digi-Desk2/doc.Digi-Desk2.com" dir="rtl">و مستندات آن</a> به زبان مادری شما به کمکتان نیازمندیم. </b></p>

با ما گفتگو کنید:  [Reddit](https://www.reddit.com/r/Digi-Desk2) | [Twitter](https://twitter.com/Digi-Desk2) | [Discord](https://discord.gg/nDceKgxnkV) 


[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

راست‌دسک (RustDesk) نرم‌افزاری برای کارکردن با رایانه‌ی رومیزی از راه دور است و با زبان برنامه‌نویسی Rust نوشته شده است. نیاز به تنظیمات چندانی ندارد و شما را قادر می سازد تا بدون نگرانی از امنیت اطلاعات خود بر آن‌ها کنترل کامل داشته باشید.

می‌توانید از سرور rendezvous/relay ما استفاده کنید، [سرور خودتان را راه‌اندازی کنید](https://Digi-Desk2.com/server) یا
[ سرورrendezvous/relay  خود را بنویسید](https://github.com/Digi-Desk2/Digi-Desk2).

ما از مشارکت همه استقبال می کنیم. برای راهنمایی جهت مشارکت به[`docs/CONTRIBUTING.md`](CONTRIBUTING.md) مراجعه کنید.

[راست‌دسک چطور کار می کند؟](https://github.com/Digi-Desk2/Digi-Desk2/wiki/How-does-RustDesk-work%3F)

[دریافت نرم‌افزار](https://github.com/Digi-Desk2/Digi-Desk2/releases)

## سرورهای عمومی رایگان

شما مي‌توانید از سرورهای زیر به رایگان استفاده کنید. این لیست ممکن است به مرور زمان تغییر می‌کند. اگر به این سرورها نزدیک نیستید، ممکن است اتصال شما کند باشد.
| موقعیت | سرویس دهنده | مشخصات |
| --------- | ------------- | ------------------ |
| آلمان | Hetzner | 2 vCPU / 4GB RAM |

## وابستگی ها

نسخه‌های رومیزی از [sciter](https://sciter.com/) برای رابط کاربری گرافیکی استفاده می‌کنند. خواهشمندیم کتابخانه‌ی پویای sciter را خودتان دانلود کنید از این منابع دریافت کنید.

- [ویندوز](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll)
- [لینوکس](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so)
- [مک](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

نسخه های همراه از Flutter استفاده می کنند. نسخه‌ی رومیزی را هم از Sciter به Flutter منتقل خواهیم کرد.

## نیازمندی‌های ساخت

- محیط توسعه نرم افزار Rust و محیط ساخت ++C خود را آماده کنید

- نرم افزار [vcpkg](https://github.com/microsoft/vcpkg) را نصب کنید و متغیر `VCPKG_ROOT` را به درستی تنظیم کنید.
- بسته‌های vcpkg مورد نیاز را نصب کنید:
  - ویندوز: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
  - مک و لینوکس: `vcpkg install libvpx libyuv opus aom`
- این دستور را اجرا کنید: `cargo run`

## [ساخت](https://Digi-Desk2.com/docs/en/dev/build/)

## نحوه ساخت بر روی لینوکس

### ساخت بر روی (Ubuntu 18 (Debian 10

```sh
sudo apt install -y g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake
```

### ساخت بر روی (Fedora 28 (CentOS 8

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel
```

### ساخت بر روی (Arch (Manjaro

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### نرم افزار vcpkg را نصب کنید

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### رفع ایراد libvpx (برای فدورا)

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

### ساخت

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone https://github.com/Digi-Desk2/Digi-Desk2
cd Digi-Desk2
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## نحوه ساخت با داکر

این مخزن Git را دریافت کنید و کانتینر را به روش زیر بسازید

```sh
git clone https://github.com/Digi-Desk2/Digi-Desk2
cd Digi-Desk2
docker build -t "Digi-Desk2-builder" .
```

سپس، هر بار که نیاز به ساخت نرم‌افزار داشتید، دستور زیر را اجرا کنید:

```sh
docker run --rm -it -v $PWD:/home/user/Digi-Desk2 -v Digi-Desk2-git-cache:/home/user/.cargo/git -v Digi-Desk2-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" Digi-Desk2-builder
```

توجه داشته باشید که نخستین ساخت ممکن است به دلیل محلی نبودن وابستگی‌ها بیشتر طول بکشد. اما دفعات بعدی سریعتر خواهند بود. علاوه بر این، اگر نیاز به تعیین آرگومان های مختلف برای دستور ساخت دارید، می توانید این کار را در انتهای دستور ساخت و از طریق `<OPTIONAL-ARGS>` انجام دهید. به عنوان مثال، اگر می خواهید یک نسخه نهایی بهینه سازی شده ایجاد کنید، دستور بالا را تایپ کنید و در انتها  `release--` را اضافه کنید. فایل اجرایی به دست آمده در پوشه مقصد در سیستم شما در دسترس خواهد بود و می تواند با دستور:

```sh
target/debug/Digi-Desk2
```

یا برای نسخه بهینه سازی شده دستور زیر را اجرا کنید:

```sh
target/release/Digi-Desk2
```

لطفاً اطمینان حاصل کنید که این دستورات را از پوشه مخزن RustDesk اجرا می کنید، در غیر این صورت ممکن است برنامه نتواند منابع مورد نیاز را پیدا کند. همچنین توجه داشته باشید که سایر دستورات فرعی Cargo مانند `install` یا `run` در حال حاضر از طریق این روش پشتیبانی نمی شوند زیرا برنامه به جای سیستم عامل میزبان, در داخل کانتینر نصب و اجرا میشود.

## ساختار پوشه ها 

- **[libs/hbb_common](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions for file transfer, and some other utility functions
- **[libs/scrap](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/libs/enigo)**: platform specific keyboard/mouse control
- **[src/ui](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/server)**: audio/clipboard/input/video services, and network connections
- **[src/client.rs](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/rendezvous_mediator.rs)**: Communicate with [Digi-Desk2-server](https://github.com/Digi-Desk2/Digi-Desk2-server), wait for remote direct (TCP hole punching) or relayed connection
- **[src/platform](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/src/platform)**: platform specific code
- **[flutter](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/flutter)**: Flutter code for mobile
- **[flutter/web/js](https://github.com/Digi-Desk2/Digi-Desk2/tree/master/flutter/web/js)**: Javascript for Flutter web client

## تصاویر محیط نرم‌افزار

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
