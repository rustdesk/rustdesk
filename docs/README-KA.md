<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - თქვენი დისტანციური სამუშაო მაგიდა"><br>
  <a href="#აწყობის-ძირითადი-ნაბიჯები">აწყობა</a> •
  <a href="#როგორ-ავაწყოთ-docker-ით">Docker</a> •
  <a href="#ფაილების-სტრუქტურა">სტრუქტურა</a> •
  <a href="#ეკრანის-სურათები">ეკრანის სურათები</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>]<br>
  <b>გვჭირდება თქვენი დახმარება ამ README-ის, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk-ის ინტერფეისისა</a> და <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk-ის დოკუმენტაციის</a> თქვენს მშობლიურ ენაზე სათარგმნად</b>
</p>

> [!Caution]
> **პასუხისმგებლობის უარყოფა არასათანადო გამოყენებაზე:** <br>
> RustDesk-ის შემქმნელები არ იწონებენ და არ უჭერენ მხარს ამ პროგრამის რაიმე არაეთიკურ ან უკანონო გამოყენებას. არასათანადო გამოყენება, როგორიცაა უნებართვო წვდომა, კონტროლი ან პირად ცხოვრებაში ჩარევა, მკაცრად ეწინააღმდეგება ჩვენს წესებს. ავტორები არ აგებენ პასუხს აპლიკაციის რაიმე არასათანადო გამოყენებაზე.


მოგვწერეთ: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

კიდევ ერთი დისტანციური სამუშაო მაგიდის პროგრამა, დაწერილი Rust-ზე. მუშაობს დაუყოვნებლივ, კონფიგურაციის გარეშე. თქვენ სრულად აკონტროლებთ თქვენს მონაცემებს და უსაფრთხოებაზე ფიქრი არ გიწევთ. შეგიძლიათ გამოიყენოთ ჩვენი rendezvous/relay სერვერი, [გამართოთ საკუთარი](https://rustdesk.com/server) ან [დაწეროთ საკუთარი rendezvous/relay სერვერი](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk მიესალმება ყველას წვლილს. დასაწყებად იხილეთ [CONTRIBUTING.md](CONTRIBUTING.md).

[**ხშირად დასმული კითხვები**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**პროგრამის ჩამოტვირთვა**](https://github.com/rustdesk/rustdesk/releases)

[**ყოველღამეული ანაწყობი**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## დამოკიდებულებები

დესკტოპ ვერსიები გრაფიკული ინტერფეისისთვის იყენებს Flutter-ს ან Sciter-ს (მოძველებული). ეს სახელმძღვანელო მხოლოდ Sciter-ს ეხება, რადგან მასთან მუშაობის დაწყება უფრო მარტივი და მოსახერხებელია. Flutter ვერსიის ასაწყობად იხილეთ ჩვენი [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml).

გთხოვთ, Sciter-ის დინამიკური ბიბლიოთეკა თავად ჩამოტვირთოთ.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## აწყობის ძირითადი ნაბიჯები

- მოამზადეთ Rust-ის დეველოპმენტის გარემო და C++-ის აწყობის გარემო

- დააინსტალირეთ [vcpkg](https://github.com/microsoft/vcpkg) და სწორად მიუთითეთ გარემოს ცვლადი `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- გაუშვით `cargo run`

## [აწყობა](https://rustdesk.com/docs/en/dev/build/)

## როგორ ავაწყოთ Linux-ზე

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

### vcpkg-ის ინსტალაცია

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx-ის გასწორება (Fedora-სთვის)

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

### აწყობა

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

## როგორ ავაწყოთ Docker-ით

დაიწყეთ რეპოზიტორიის კლონირებით და Docker-კონტეინერის აწყობით:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

შემდეგ, ყოველ ჯერზე, როცა აპლიკაციის აწყობა დაგჭირდებათ, გაუშვით ეს ბრძანება:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

გაითვალისწინეთ, რომ პირველ აწყობას შეიძლება მეტი დრო დასჭირდეს, სანამ დამოკიდებულებები დაქეშდება; შემდგომი აწყობები უფრო სწრაფი იქნება. გარდა ამისა, თუ აწყობის ბრძანებისთვის სხვა არგუმენტების მითითება გჭირდებათ, ამის გაკეთება შეგიძლიათ ბრძანების ბოლოს, `<OPTIONAL-ARGS>` პოზიციაზე. მაგალითად, თუ ოპტიმიზებული რელიზ-ვერსიის აწყობა გსურთ, გაუშვით ზემოთ მოცემული ბრძანება და ბოლოში მიუწერეთ `--release`. მიღებული შესრულებადი ფაილი თქვენს სისტემაში target საქაღალდეში იქნება ხელმისაწვდომი და მისი გაშვება ასე შეიძლება:

```sh
target/debug/rustdesk
```

ან, თუ რელიზის შესრულებად ფაილს უშვებთ:

```sh
target/release/rustdesk
```

გთხოვთ, დარწმუნდეთ, რომ ამ ბრძანებებს RustDesk-ის რეპოზიტორიის ძირეული საქაღალდიდან უშვებთ, წინააღმდეგ შემთხვევაში აპლიკაციამ შეიძლება ვერ იპოვოს საჭირო რესურსები. ასევე გაითვალისწინეთ, რომ cargo-ს სხვა ქვებრძანებები, როგორიცაა `install` ან `run`, ამ მეთოდით ამჟამად არ არის მხარდაჭერილი, რადგან ისინი პროგრამას დააინსტალირებდნენ ან გაუშვებდნენ კონტეინერის შიგნით და არა ჰოსტზე.

## ფაილების სტრუქტურა

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ვიდეოკოდეკი, კონფიგურაცია, tcp/udp-ის ვრაპერი და სერვერთან გაზიარებული ზოგიერთი სხვა დამხმარე ფუნქცია
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf, ფაილური სისტემის ფუნქციები ფაილების გადასაცემად, კლავიატურისა და პლატფორმის კოდი, რომელსაც მხოლოდ ეს აპლიკაცია იყენებს
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: ეკრანის გადაღება
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: კლავიატურის/მაუსის მართვა კონკრეტული პლატფორმისთვის
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: ფაილების კოპირებისა და ჩასმის რეალიზაცია Windows-ის, Linux-ისა და macOS-ისთვის.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: ძველი Sciter UI (მოძველებული)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: აუდიოს/გაცვლის ბუფერის/შეყვანის/ვიდეოს სერვისები და ქსელური კავშირები
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: peer-კავშირის დაწყება
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: კავშირი [rustdesk-server](https://github.com/rustdesk/rustdesk-server)-თან, ელოდება დისტანციურ პირდაპირ (TCP hole punching) ან რელეით გადაცემულ კავშირს
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: პლატფორმაზე დამოკიდებული კოდი
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter-ის კოდი დესკტოპისა და მობილურისთვის

## ეკრანის სურათები

![კავშირების მენეჯერი](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![დაკავშირებულია Windows კომპიუტერთან](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![ფაილების გადაცემა](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP ტუნელირება](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)

