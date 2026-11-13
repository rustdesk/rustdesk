<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - آپ کا ریموٹ ڈیسک ٹاپ"><br>
  <a href="#بلڈ-کرنے-کے-بنیادی-مراحل">بلڈ</a> •
  <a href="#docker-کے-ساتھ-بلڈ-کیسے-کریں">Docker</a> •
  <a href="#فائلوں-کی-ساخت">ساخت</a> •
  <a href="#اسکرین-شاٹس">اسکرین شاٹس</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>] | [<a href="README-HI.md">हिन्दी</a>] | [<a href="README-BN.md">বাংলা</a>] | [<a href="README-TH.md">ไทย</a>] | [<a href="README-ZHTW.md">繁體中文</a>]<br>
  <b>اس README، <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> اور <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk دستاویزات</a> کا آپ کی مادری زبان میں ترجمہ کرنے کے لیے ہمیں آپ کی مدد درکار ہے</b>
</p>

> [!Caution]
> **غلط استعمال سے متعلق دستبرداری:** <br>
> RustDesk کے ڈویلپرز اس سافٹ ویئر کے کسی بھی غیر اخلاقی یا غیر قانونی استعمال کی حمایت نہیں کرتے۔ غیر مجاز رسائی، کنٹرول یا رازداری میں مداخلت جیسا غلط استعمال ہماری ہدایات کے سخت خلاف ہے۔ ایپلیکیشن کے کسی بھی غلط استعمال کی ذمہ داری مصنفین پر نہیں ہے۔


ہم سے بات کریں: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Rust میں لکھا گیا ایک اور ریموٹ ڈیسک ٹاپ حل۔ یہ کسی کنفیگریشن کے بغیر فوراً کام کرتا ہے۔ آپ کے ڈیٹا پر آپ کا مکمل کنٹرول رہتا ہے اور سیکیورٹی کی کوئی فکر نہیں۔ آپ ہمارا rendezvous/relay سرور استعمال کر سکتے ہیں، [اپنا سرور سیٹ اپ کر سکتے ہیں](https://rustdesk.com/server)، یا [اپنا rendezvous/relay سرور لکھ سکتے ہیں](https://github.com/rustdesk/rustdesk-server-demo)۔

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk ہر کسی کی شراکت کا خیرمقدم کرتا ہے۔ آغاز کے لیے [CONTRIBUTING.md](CONTRIBUTING.md) دیکھیں۔

[**اکثر پوچھے جانے والے سوالات**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**بائنری ڈاؤن لوڈ**](https://github.com/rustdesk/rustdesk/releases)

[**نائٹلی بلڈ**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## انحصارات

ڈیسک ٹاپ ورژن GUI کے لیے Flutter یا Sciter (متروک) استعمال کرتے ہیں۔ یہ ٹیوٹوریل صرف Sciter کے لیے ہے، کیونکہ اس سے آغاز کرنا آسان اور سہل ہے۔ Flutter ورژن بلڈ کرنے کے لیے ہمارا [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) دیکھیں۔

براہ کرم Sciter کی ڈائنامک لائبریری خود ڈاؤن لوڈ کریں۔

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## بلڈ کرنے کے بنیادی مراحل

- اپنا Rust ڈویلپمنٹ ماحول اور C++ بلڈ ماحول تیار کریں

- [vcpkg](https://github.com/microsoft/vcpkg) انسٹال کریں اور `VCPKG_ROOT` ماحولیاتی متغیر درست طریقے سے سیٹ کریں

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- `cargo run` چلائیں

## [بلڈ](https://rustdesk.com/docs/en/dev/build/)

## Linux پر بلڈ کیسے کریں

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

### vcpkg انسٹال کریں

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx درست کریں (Fedora کے لیے)

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

### بلڈ

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

## Docker کے ساتھ بلڈ کیسے کریں

سب سے پہلے ریپوزٹری کلون کریں اور Docker کنٹینر بلڈ کریں:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

اس کے بعد، جب بھی آپ کو ایپلیکیشن بلڈ کرنی ہو، یہ کمانڈ چلائیں:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

یاد رکھیں کہ پہلی بلڈ میں زیادہ وقت لگ سکتا ہے کیونکہ اس وقت تک انحصارات کیش نہیں ہوئے ہوتے؛ بعد کی بلڈز تیز ہوں گی۔ اس کے علاوہ، اگر آپ کو بلڈ کمانڈ میں مختلف آرگیومنٹس دینے ہوں تو آپ انہیں کمانڈ کے آخر میں `<OPTIONAL-ARGS>` کی جگہ دے سکتے ہیں۔ مثال کے طور پر، اگر آپ آپٹمائزڈ ریلیز ورژن بلڈ کرنا چاہتے ہیں تو اوپر دی گئی کمانڈ کے آخر میں `--release` لگا کر چلائیں۔ تیار شدہ ایگزیکیوٹیبل آپ کے سسٹم پر target فولڈر میں دستیاب ہوگا اور اسے یوں چلایا جا سکتا ہے:

```sh
target/debug/rustdesk
```

یا، اگر آپ ریلیز ایگزیکیوٹیبل چلا رہے ہیں:

```sh
target/release/rustdesk
```

براہ کرم یقینی بنائیں کہ آپ یہ کمانڈز RustDesk ریپوزٹری کے روٹ سے چلا رہے ہیں، ورنہ ایپلیکیشن کو ضروری وسائل نہیں مل سکیں گے۔ یہ بھی نوٹ کریں کہ cargo کی دیگر سب کمانڈز، جیسے `install` یا `run`، اس طریقے سے فی الحال معاون نہیں ہیں، کیونکہ وہ پروگرام کو ہوسٹ کے بجائے کنٹینر کے اندر انسٹال کریں گی یا چلائیں گی۔

## فائلوں کی ساخت

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ویڈیو کوڈیک، کنفیگ، tcp/udp ریپر اور سرور کے ساتھ مشترک کچھ دیگر یوٹیلیٹی فنکشنز
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf، فائل ٹرانسفر کے لیے fs فنکشنز، کی بورڈ اور پلیٹ فارم کوڈ جو صرف یہی ایپ استعمال کرتی ہے
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: اسکرین کیپچر
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: پلیٹ فارم کے لحاظ سے کی بورڈ/ماؤس کنٹرول
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: Windows، Linux اور macOS کے لیے فائل کاپی اور پیسٹ کا نفاذ۔
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: پرانا Sciter UI (متروک)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: آڈیو/کلپ بورڈ/ان پٹ/ویڈیو سروسز اور نیٹ ورک کنکشنز
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: پیئر کنکشن شروع کرنا
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server) سے رابطہ کرتا ہے اور ریموٹ براہِ راست (TCP hole punching) یا ریلے کنکشن کا انتظار کرتا ہے
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: پلیٹ فارم کے لحاظ سے مخصوص کوڈ
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: ڈیسک ٹاپ اور موبائل کے لیے Flutter کوڈ

## اسکرین شاٹس

![کنکشن مینیجر](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Windows پی سی سے منسلک](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![فائل ٹرانسفر](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP ٹنلنگ](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
