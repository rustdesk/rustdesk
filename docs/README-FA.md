<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a dir="rtl" href="#اسکرین-شات-ها">اسنپ شات</a> •
  <a dir="rtl" href="#ساختار-پوشه-ها">ساختار</a> •
  <a dir="rtl" href="#نحوه-ساخت-با-داکر">داکر</a> •
  <a dir="rtl" href="#ساخت">ساخت</a> •
  <a dir="rtl" href="#سرورهای-عمومی-رایگان">سرور</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>]<br>
  &#x202b;<b>برای ترجمه این  <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang"> RustDesk UI</a>  ،README  و <a href="https://github.com/rustdesk/doc.rustdesk.com">Doc</a> به زبان مادری شما به کمکتون نیاز داریم
</p>

با ما گپ بزنید:  [Reddit](https://www.reddit.com/r/rustdesk) | [Twitter](https://twitter.com/rustdesk) | [Discord](https://discord.gg/nDceKgxnkV) 


[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

یک نرم افزار دیگر کنترل دسکتاپ از راه دور، که با Rust نوشته شده است. راه اندازی سریع وبدون نیاز به تنظیمات. شما کنترل کاملی بر داده های خود دارید، بدون هیچ گونه نگرانی امنیتی.
می‌توانید از سرور rendezvous/relay ما استفاده کنید، [سرور خودتان را راه‌اندازی کنید](https://rustdesk.com/server) یا
[ سرورrendezvous/relay  خود را بنویسید](https://github.com/rustdesk/rustdesk).

&#x202b;راست دسک (RustDesk) از مشارکت همه استقبال می کند. برای راهنمایی جهت مشارکت به [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) مراجعه کنید.

[راست دسک چطور کار می کند؟](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[دانلود باینری](https://github.com/rustdesk/rustdesk/releases)

## سرورهای عمومی رایگان

سرورهایی  زیر را  به صورت رایگان میتوانید استفاده می کنید. این لیست ممکن است در طول زمان تغییر کند. اگر به این سرورها نزدیک نیستید، ممکن است سرویس شما کند شود.
| موقعیت | سرویس دهنده | مشخصات |
| --------- | ------------- | ------------------ |
| Seoul | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Germany | Hetzner | 2 vCPU / 4GB RAM |
| Germany | Codext | 4 vCPU / 8GB RAM |
| Finland (Helsinki) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| USA (Ashburn) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |

## وابستگی ها

نسخه‌های دسکتاپ از [sciter](https://sciter.com/) برای رابط کاربری گرافیکی استفاده می‌کنند، لطفا کتابخانه پویا sciter را خودتان دانلود کنید.

[ویندوز](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[لینوکس](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[مک](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

نسخه های موبایل از Flutter استفاده می کنند. بعداً نسخه دسکتاپ را از Sciter به Flutter منتقل خواهیم کرد.

## مراحل بنیادین برای ساخت

&#x202b;- محیط توسعه نرم افزار Rust و محیط ساخت ++C خود را آماده کنید

&#x202b;- نرم افزار [vcpkg](https://github.com/microsoft/vcpkg) را نصب کنید و متغیر `VCPKG_ROOT` را به درستی تنظیم کنید:

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static`
  - Linux/MacOS: `vcpkg install libvpx libyuv opus`

- run `cargo run`

## [ساخت](https://rustdesk.com/docs/en/dev/build/)

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
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
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
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

### تغییر Wayland به (X11 (Xorg

راست دسک از Wayland پشتیبانی نمی کند. برای جایگزنی Xorg به عنوان پیش‌فرض GNOM، [اینجا](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) را کلیک کنید.

## نحوه ساخت با داکر

این مخزن گیت را کلون کنید و کانتینر را به روش زیر بسازید

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

سپس، هر بار که نیاز به ساخت اپلیکیشن داشتید، دستور زیر را اجرا کنید:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

توجه داشته باشید که ساخت اول ممکن است قبل از کش شدن وابستگی ها بیشتر طول بکشد، دفعات بعدی سریعتر خواهند بود. علاوه بر این، اگر نیاز به تعیین آرگومان های مختلف برای دستور ساخت دارید، می توانید این کار را در انتهای دستور ساخت و از طریق `<OPTIONAL-ARGS>` انجام دهید. به عنوان مثال، اگر می خواهید یک نسخه نهایی بهینه سازی شده ایجاد کنید، دستور بالا را تایپ کنید و در انتها  `release--` را اضافه کنید. فایل اجرایی به دست آمده در پوشه مقصد در سیستم شما در دسترس خواهد بود و می تواند با دستور:

```sh
target/debug/rustdesk
```

یا برای نسخه بهینه سازی شده دستور زیر را اجرا کنید:

```sh
target/release/rustdesk
```

لطفاً اطمینان حاصل کنید که این دستورات را از پوشه مخزن RustDesk اجرا می کنید، در غیر این صورت ممکن است برنامه نتواند منابع مورد نیاز را پیدا کند. همچنین توجه داشته باشید که سایر دستورات فرعی Cargo مانند `install` یا `run` در حال حاضر از طریق این روش پشتیبانی نمی شوند زیرا برنامه به جای سیستم عامل میزبان, در داخل کانتینر نصب و اجرا میشود.

## ساختار پوشه ها 

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions for file transfer, and some other utility functions
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platform specific keyboard/mouse control
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: audio/clipboard/input/video services, and network connections
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Communicate with [rustdesk-server](https://github.com/rustdesk/rustdesk-server), wait for remote direct (TCP hole punching) or relayed connection
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platform specific code
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter code for mobile
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Javascript for Flutter web client

## اسکرین شات ها

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
