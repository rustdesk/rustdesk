<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - รีโมตเดสก์ท็อปของคุณ"><br>
  <a href="#ขั้นตอนการบิลด์เบื้องต้น">บิลด์</a> •
  <a href="#วิธีบิลด์ด้วย-docker">Docker</a> •
  <a href="#โครงสร้างไฟล์">โครงสร้าง</a> •
  <a href="#ภาพหน้าจอ">ภาพหน้าจอ</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>] | [<a href="README-HI.md">हिन्दी</a>] | [<a href="README-BN.md">বাংলা</a>] | [<a href="README-UR.md">اردو</a>] | [<a href="README-ZHTW.md">繁體中文</a>]<br>
  <b>เราต้องการความช่วยเหลือจากคุณในการแปล README นี้ <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> และ <a href="https://github.com/rustdesk/doc.rustdesk.com">เอกสาร RustDesk</a> เป็นภาษาแม่ของคุณ</b>
</p>

> [!Caution]
> **คำปฏิเสธความรับผิดชอบต่อการใช้งานในทางที่ผิด:** <br>
> ผู้พัฒนา RustDesk ไม่สนับสนุนการใช้ซอฟต์แวร์นี้ในทางที่ผิดจริยธรรมหรือผิดกฎหมายใด ๆ การใช้งานในทางที่ผิด เช่น การเข้าถึงหรือควบคุมโดยไม่ได้รับอนุญาต หรือการละเมิดความเป็นส่วนตัว ขัดต่อแนวทางของเราอย่างร้ายแรง ผู้เขียนไม่รับผิดชอบต่อการใช้งานแอปพลิเคชันในทางที่ผิดใด ๆ


พูดคุยกับเรา: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

โซลูชันรีโมตเดสก์ท็อปอีกตัวหนึ่งที่เขียนด้วย Rust ใช้งานได้ทันทีโดยไม่ต้องตั้งค่าใด ๆ คุณควบคุมข้อมูลของคุณได้อย่างเต็มที่ ไม่ต้องกังวลเรื่องความปลอดภัย คุณสามารถใช้เซิร์ฟเวอร์ rendezvous/relay ของเรา [ตั้งเซิร์ฟเวอร์ของคุณเอง](https://rustdesk.com/server) หรือ [เขียนเซิร์ฟเวอร์ rendezvous/relay ของคุณเอง](https://github.com/rustdesk/rustdesk-server-demo)

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk ยินดีต้อนรับการมีส่วนร่วมจากทุกคน ดู [CONTRIBUTING.md](CONTRIBUTING.md) เพื่อเริ่มต้น

[**คำถามที่พบบ่อย**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**ดาวน์โหลดไฟล์ไบนารี**](https://github.com/rustdesk/rustdesk/releases)

[**บิลด์รายคืน (Nightly)**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## ไลบรารีที่ต้องใช้

เวอร์ชันเดสก์ท็อปใช้ Flutter หรือ Sciter (เลิกใช้แล้ว) สำหรับ GUI บทแนะนำนี้ครอบคลุมเฉพาะ Sciter เพราะเริ่มต้นได้ง่ายและสะดวกกว่า สำหรับการบิลด์เวอร์ชัน Flutter ดู [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) ของเรา

โปรดดาวน์โหลดไลบรารีไดนามิกของ Sciter ด้วยตนเอง

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## ขั้นตอนการบิลด์เบื้องต้น

- เตรียมสภาพแวดล้อมการพัฒนา Rust และสภาพแวดล้อมการบิลด์ C++

- ติดตั้ง [vcpkg](https://github.com/microsoft/vcpkg) และตั้งค่าตัวแปรสภาพแวดล้อม `VCPKG_ROOT` ให้ถูกต้อง

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- รัน `cargo run`

## [บิลด์](https://rustdesk.com/docs/en/dev/build/)

## วิธีบิลด์บน Linux

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

### ติดตั้ง vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### แก้ไข libvpx (สำหรับ Fedora)

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

### บิลด์

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

## วิธีบิลด์ด้วย Docker

เริ่มจากโคลนรีโพซิทอรีและบิลด์คอนเทนเนอร์ Docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

จากนั้น ทุกครั้งที่ต้องการบิลด์แอปพลิเคชัน ให้รันคำสั่งต่อไปนี้:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

โปรดทราบว่าการบิลด์ครั้งแรกอาจใช้เวลานานกว่าเนื่องจากยังไม่มีการแคชไลบรารีที่ต้องใช้ การบิลด์ครั้งต่อ ๆ ไปจะเร็วขึ้น นอกจากนี้ หากต้องการระบุอาร์กิวเมนต์อื่นให้กับคำสั่งบิลด์ ก็สามารถใส่ไว้ท้ายคำสั่งในตำแหน่ง `<OPTIONAL-ARGS>` ได้ ตัวอย่างเช่น หากต้องการบิลด์เวอร์ชันรีลีสที่ปรับแต่งประสิทธิภาพแล้ว ให้รันคำสั่งข้างต้นตามด้วย `--release` ไฟล์ปฏิบัติการที่ได้จะอยู่ในโฟลเดอร์ target บนระบบของคุณ และรันได้ด้วยคำสั่ง:

```sh
target/debug/rustdesk
```

หรือ หากคุณรันไฟล์ปฏิบัติการเวอร์ชันรีลีส:

```sh
target/release/rustdesk
```

โปรดตรวจสอบให้แน่ใจว่าคุณรันคำสั่งเหล่านี้จากโฟลเดอร์รากของรีโพซิทอรี RustDesk มิฉะนั้นแอปพลิเคชันอาจหาทรัพยากรที่จำเป็นไม่พบ และโปรดทราบว่าคำสั่งย่อยอื่น ๆ ของ cargo เช่น `install` หรือ `run` ยังไม่รองรับในวิธีนี้ เพราะคำสั่งเหล่านั้นจะติดตั้งหรือรันโปรแกรมภายในคอนเทนเนอร์แทนที่จะเป็นบนเครื่องโฮสต์

## โครงสร้างไฟล์

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: ตัวแปลงสัญญาณวิดีโอ, การตั้งค่า, ตัวห่อ tcp/udp และฟังก์ชันอรรถประโยชน์อื่น ๆ ที่ใช้ร่วมกับเซิร์ฟเวอร์
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf, ฟังก์ชัน fs สำหรับการถ่ายโอนไฟล์, โค้ดคีย์บอร์ดและโค้ดเฉพาะแพลตฟอร์มที่ใช้เฉพาะในแอปนี้
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: การจับภาพหน้าจอ
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: การควบคุมคีย์บอร์ด/เมาส์เฉพาะแพลตฟอร์ม
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: การคัดลอกและวางไฟล์สำหรับ Windows, Linux, macOS
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: Sciter UI รุ่นเก่า (เลิกใช้แล้ว)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: บริการเสียง/คลิปบอร์ด/อินพุต/วิดีโอ และการเชื่อมต่อเครือข่าย
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: เริ่มการเชื่อมต่อแบบ peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: สื่อสารกับ [rustdesk-server](https://github.com/rustdesk/rustdesk-server) รอการเชื่อมต่อระยะไกลแบบตรง (TCP hole punching) หรือผ่าน relay
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: โค้ดเฉพาะแพลตฟอร์ม
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: โค้ด Flutter สำหรับเดสก์ท็อปและมือถือ

## ภาพหน้าจอ

![ตัวจัดการการเชื่อมต่อ](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![เชื่อมต่อกับพีซี Windows](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![การถ่ายโอนไฟล์](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
