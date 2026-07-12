<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - თქვენი დისტანციური სამუშაო მაგიდა"><br>
  <a href="#აწყობის-პირველადი-ნაბიჯები">აწყობის პირველადი ნაბიჯები</a> •
  <a href="#როგორ-ავაწყოთ-Docker-ის-გამოყენებით">როგორ ავაწყოთ Docker-ის გამოყენებით</a> •
  <a href="#ფაილების-სტრუქტურა">ფაილების სტრუქტურა</a> •
  <a href="#ეკრანის-სურათები">ეკრანის სურათები</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>გვჭირდება თქვენი დახმარება ამ README-ის, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk-ის ინტერფეისისა</a>
     და <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk-ის დოკუმენტაციის</a> თქვენს მშობლიურ ენაზე თარგმნაში.</b>
</p>

> [!Caution]
> **პასუხისმგებლობის უარყოფა არასათანადო გამოყენებაზე** <br>
> RustDesk-ის შემქმნელები არ იწონებენ და არ უჭერენ მხარს ამ პროგრამული უზრუნველყოფის რაიმე არაეთიკურ ან უკანონო გამოყენებას. არასათანადო გამოყენება (უნებართვო წვდომა, კონტროლი ან პირად ცხოვრებაში ჩარევა) მკაცრად ეწინააღმდეგება ჩვენს წესებს. ავტორები არ აგებენ პასუხს აპლიკაციის რაიმე არასათანადო გამოყენებაზე.

დაგვიკავშირდით: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-%D0%A0%D0%B0%D1%81%D1%88%D0%B8%D1%80%D0%B5%D0%BD%D0%BD%D1%8B%D0%B5%20%D0%92%D0%BE%D0%B7%D0%BC%D0%BE%D0%B6%D0%BD%D0%BE%D1%81%D1%82%D0%B8-blue)](https://rustdesk.com/pricing.html)

კიდევ ერთი დისტანციური სამუშაო მაგიდის პროგრამა, დაწერილი Rust-ზე. მუშაობს გამოყენებისთანავე, კონფიგურაცია არ სჭირდება. თქვენ სრულად აკონტროლებთ თქვენს მონაცემებს უსაფრთხოებაზე ზრუნვის გარეშე. შეგიძლიათ გამოიყენოთ ჩვენი სარელეო სერვერი, [აწყოთ საკუთარი](https://rustdesk.com/server), ან [დაწეროთ საკუთარი](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk მიესალმება ყველას წვლილს. დაწყებამდე გასაცნობად იხილეთ [`docs/CONTRIBUTING-RU.md`](CONTRIBUTING-RU.md).

[**როგორ მუშაობს RustDesk?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F) (დოკუმენტაცია ინგლისურ ენაზე)

[**ხშირად დასმული კითხვები**](https://github.com/rustdesk/rustdesk/wiki/FAQ) (გვერდი ინგლისურ ენაზე)

[**აპლიკაციის ჩამოტვირთვა**](https://github.com/rustdesk/rustdesk/releases)

[**ღამის ბილდები (აქტუალური)**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## დამოკიდებულებები

PC-ვერსიისთვის გრაფიკული ინტერფეისისთვის გამოიყენება Flutter ან Sciter (მოძველებული) ბიბლიოთეკები. ეს სახელმძღვანელო გულისხმობს მუშაობას Sciter-თან, რადგან ის უფრო მარტივია გამოსაყენებლად და მასთან უფრო ადვილია მუშაობის დაწყება. ასევე შეგიძლიათ იხილოთ ჩვენი [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) მექანიზმი Flutter-ის ბილდებისთვის.

ჩამოტვირთეთ Flutter-ის დინამიკური ბიბლიოთეკა დამოუკიდებლად.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## აწყობის პირველადი ნაბიჯები

- მოამზადეთ Rust-ის შემუშავების გარემო და C++ აწყობის გარემო.

- დააინსტალირეთ [vcpkg](https://github.com/microsoft/vcpkg) და სწორად დააყენეთ ცვლადი `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- შეასრულეთ ბრძანება `cargo run`

## [აწყობა](https://rustdesk.com/docs/ru/dev/build/)

## როგორ ავაწყოთ Linux-ზე

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

## როგორ ავაწყოთ Docker-ის გამოყენებით

დაიწყეთ რეპოზიტორიის კლონირებით და docker-კონტეინერის შექმნით:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

შემდეგ აპლიკაციის ყოველი აწყობისას შეასრულეთ შემდეგი ბრძანება:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

გაითვალისწინეთ, რომ პირველ აწყობას შესაძლოა მეტი დრო დასჭირდეს, სანამ დამოკიდებულებები დაქეშდება, მაგრამ შემდგომი აწყობები უფრო სწრაფად შესრულდება. ასევე, თუ გჭირდებათ სხვა არგუმენტების მითითება აწყობის ბრძანებისთვის, ამის გაკეთება შეგიძლიათ ბრძანების ბოლოს `<OPTIONAL-ARGS>` ცვლადში. მაგალითად, თუ გსურთ ოპტიმიზებული ვერსიის შექმნა, უნდა შეასრულოთ ზემოთ მოცემული ბრძანება და სტრიქონის ბოლოს დაამატოთ `--release`. მიღებული შესრულებადი ფაილი ხელმისაწვდომი იქნება თქვენი სისტემის სამიზნე საქაღალდეში და შეიძლება გაეშვას შემდეგი ბრძანებით:

```sh
target/debug/rustdesk
```

ან, თუ იყენებთ რელიზის შესრულებად ფაილს:

```sh
target/release/rustdesk
```

გთხოვთ, დარწმუნდეთ, რომ ამ ბრძანებებს ასრულებთ RustDesk-ის რეპოზიტორიის ძირიდან, წინააღმდეგ შემთხვევაში აპლიკაცია ვერ იპოვის საჭირო რესურსებს. ასევე გაითვალისწინეთ, რომ Cargo-ს სხვა ქვებრძანებები, როგორიცაა `install` ან `run`, ამჟამად ამ მეთოდით არ არის მხარდაჭერილი, რადგან ისინი პროგრამას დააინსტალირებენ ან გაუშვებენ კონტეინერის შიგნით და არა ჰოსტზე.

## ფაილების სტრუქტურა

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ვიდეოკოდეკი, კონფიგურაცია, TCP/UDP-ის ვრაპერი, protobuf, ფაილური სისტემის ფუნქციები ფაილების გადაცემისთვის და ზოგიერთი სხვა სამსახურებრივი ფუნქცია
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: ეკრანის აღება
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: პლატფორმა-სპეციფიკური კლავიატურის/მაუსის მართვა
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: ფაილების ბუფერის ფუნქციონალი Windows-ის, Linux-ისა და macOS-ისთვის
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: გრაფიკული მომხმარებლის ინტერფეისი Sciter-ზე (მოძველებული)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: აუდიოს, ბუფერის, შეყვანის, ვიდეოსა და ქსელური კავშირების სერვისები
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: peer-to-peer კავშირი
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: კავშირი [RustDesk-ის სერვერთან](https://github.com/rustdesk/rustdesk-server), ელოდება დისტანციურ პირდაპირ (TCP hole punching-ის მეშვეობით) ან სარელეო კავშირს
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: პლატფორმა-სპეციფიკური კოდი
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter-ის კოდი PC-ვერსიისა და მობილური მოწყობილობებისთვის
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/v1/js)**: JavaScript Flutter-ის Web-კლიენტისთვის

## ეკრანის სურათები

![კავშირების მენეჯერი](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Windows-ზე დისტანციურ სამუშაო მაგიდასთან დაკავშირება](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![ფაილების გადაცემა](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP-ტუნელირება](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
