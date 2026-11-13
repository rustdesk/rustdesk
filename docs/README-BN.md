<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - আপনার রিমোট ডেস্কটপ"><br>
  <a href="#বিল্ড-করার-প্রাথমিক-ধাপ">বিল্ড</a> •
  <a href="#docker-দিয়ে-কীভাবে-বিল্ড-করবেন">Docker</a> •
  <a href="#ফাইলের-কাঠামো">কাঠামো</a> •
  <a href="#স্ক্রিনশট">স্ক্রিনশট</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>] | [<a href="README-HI.md">हिन्दी</a>] | [<a href="README-UR.md">اردو</a>] | [<a href="README-TH.md">ไทย</a>] | [<a href="README-ZHTW.md">繁體中文</a>]<br>
  <b>এই README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> এবং <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk ডকুমেন্টেশন</a> আপনার মাতৃভাষায় অনুবাদ করতে আমাদের আপনার সাহায্য দরকার</b>
</p>

> [!Caution]
> **অপব্যবহার সংক্রান্ত দাবিত্যাগ:** <br>
> RustDesk-এর ডেভেলপাররা এই সফটওয়্যারের কোনো অনৈতিক বা বেআইনি ব্যবহার সমর্থন করেন না। অননুমোদিত প্রবেশ, নিয়ন্ত্রণ বা গোপনীয়তা লঙ্ঘনের মতো অপব্যবহার আমাদের নির্দেশিকার সম্পূর্ণ পরিপন্থী। অ্যাপ্লিকেশনের কোনো অপব্যবহারের জন্য লেখকরা দায়ী নন।


আমাদের সঙ্গে কথা বলুন: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Rust-এ লেখা আরও একটি রিমোট ডেস্কটপ সমাধান। কোনো কনফিগারেশন ছাড়াই সঙ্গে সঙ্গে কাজ করে। আপনার ডেটার ওপর আপনার পূর্ণ নিয়ন্ত্রণ থাকে, নিরাপত্তা নিয়ে চিন্তার কিছু নেই। আপনি আমাদের rendezvous/relay সার্ভার ব্যবহার করতে পারেন, [নিজের সার্ভার সেট আপ করতে পারেন](https://rustdesk.com/server), অথবা [নিজের rendezvous/relay সার্ভার লিখতে পারেন](https://github.com/rustdesk/rustdesk-server-demo)।

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk সবার অবদানকে স্বাগত জানায়। শুরু করতে [CONTRIBUTING.md](CONTRIBUTING.md) দেখুন।

[**সচরাচর জিজ্ঞাসিত প্রশ্ন**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**বাইনারি ডাউনলোড**](https://github.com/rustdesk/rustdesk/releases)

[**নাইটলি বিল্ড**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## নির্ভরতা

ডেস্কটপ সংস্করণগুলো GUI-এর জন্য Flutter অথবা Sciter (অপ্রচলিত) ব্যবহার করে। এই টিউটোরিয়ালটি শুধু Sciter-এর জন্য, কারণ এটি দিয়ে শুরু করা সহজ ও সুবিধাজনক। Flutter সংস্করণ বিল্ড করতে আমাদের [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) দেখুন।

অনুগ্রহ করে Sciter ডায়নামিক লাইব্রেরি নিজে ডাউনলোড করুন।

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## বিল্ড করার প্রাথমিক ধাপ

- আপনার Rust ডেভেলপমেন্ট এনভায়রনমেন্ট এবং C++ বিল্ড এনভায়রনমেন্ট প্রস্তুত করুন

- [vcpkg](https://github.com/microsoft/vcpkg) ইনস্টল করুন এবং `VCPKG_ROOT` এনভায়রনমেন্ট ভেরিয়েবল সঠিকভাবে সেট করুন

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- `cargo run` চালান

## [বিল্ড](https://rustdesk.com/docs/en/dev/build/)

## Linux-এ কীভাবে বিল্ড করবেন

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

### vcpkg ইনস্টল করুন

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx ঠিক করুন (Fedora-র জন্য)

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

### বিল্ড

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

## Docker দিয়ে কীভাবে বিল্ড করবেন

প্রথমে রিপোজিটরি ক্লোন করুন এবং Docker কন্টেইনার বিল্ড করুন:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

এরপর, যখনই অ্যাপ্লিকেশন বিল্ড করার দরকার হবে, এই কমান্ডটি চালান:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

মনে রাখবেন, প্রথম বিল্ডে বেশি সময় লাগতে পারে, কারণ তখনও নির্ভরতাগুলো ক্যাশ হয়নি; পরের বিল্ডগুলো দ্রুত হবে। এছাড়া বিল্ড কমান্ডে ভিন্ন আর্গুমেন্ট দিতে চাইলে কমান্ডের শেষে `<OPTIONAL-ARGS>` অবস্থানে দিতে পারেন। উদাহরণস্বরূপ, অপ্টিমাইজ করা রিলিজ সংস্করণ বিল্ড করতে চাইলে ওপরের কমান্ডের শেষে `--release` যোগ করে চালান। তৈরি হওয়া এক্সিকিউটেবল আপনার সিস্টেমের target ফোল্ডারে পাওয়া যাবে এবং এভাবে চালানো যাবে:

```sh
target/debug/rustdesk
```

অথবা, যদি রিলিজ এক্সিকিউটেবল চালান:

```sh
target/release/rustdesk
```

অনুগ্রহ করে নিশ্চিত করুন যে এই কমান্ডগুলো RustDesk রিপোজিটরির রুট থেকে চালাচ্ছেন, নইলে অ্যাপ্লিকেশন প্রয়োজনীয় রিসোর্স খুঁজে নাও পেতে পারে। আরও মনে রাখবেন, cargo-র অন্যান্য সাবকমান্ড, যেমন `install` বা `run`, এই পদ্ধতিতে বর্তমানে সমর্থিত নয়, কারণ সেগুলো প্রোগ্রামটি হোস্টের বদলে কন্টেইনারের ভেতরে ইনস্টল বা চালু করবে।

## ফাইলের কাঠামো

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ভিডিও কোডেক, কনফিগ, tcp/udp wrapper এবং সার্ভারের সঙ্গে ভাগ করা আরও কিছু ইউটিলিটি ফাংশন
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf, ফাইল স্থানান্তরের জন্য fs ফাংশন, কিবোর্ড ও প্ল্যাটফর্ম কোড, যা শুধু এই অ্যাপই ব্যবহার করে
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: স্ক্রিন ক্যাপচার
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: প্ল্যাটফর্ম-নির্দিষ্ট কিবোর্ড/মাউস নিয়ন্ত্রণ
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: Windows, Linux, macOS-এর জন্য ফাইল কপি ও পেস্টের বাস্তবায়ন।
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: পুরোনো Sciter UI (অপ্রচলিত)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: অডিও/ক্লিপবোর্ড/ইনপুট/ভিডিও সার্ভিস এবং নেটওয়ার্ক সংযোগ
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: পিয়ার সংযোগ শুরু করা
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server)-এর সঙ্গে যোগাযোগ করে, রিমোট সরাসরি (TCP hole punching) অথবা রিলে সংযোগের জন্য অপেক্ষা করে
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: প্ল্যাটফর্ম-নির্দিষ্ট কোড
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: ডেস্কটপ ও মোবাইলের জন্য Flutter কোড

## স্ক্রিনশট

![সংযোগ ব্যবস্থাপক](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![একটি Windows পিসির সঙ্গে সংযুক্ত](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![ফাইল স্থানান্তর](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP টানেলিং](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
