<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#free-public-servers">Servers</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Kami membutuhkan bantuan Anda untuk menerjemahkan README ini dan <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> ke bahasa asli anda</b>
</p>

Birbincang bersama kami: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Perangkat lunak desktop jarak jauh lainnya, ditulis dengan Rust. Bekerja begitu saja, tidak memerlukan konfigurasi. Anda memiliki kendali penuh atas data Anda, tanpa khawatir tentang keamanan. Anda dapat menggunakan server rendezvous/relay kami, [konfigurasi server sendiri](https://rustdesk.com/server), or [tulis rendezvous/relay server anda sendiri](https://github.com/rustdesk/rustdesk-server-demo).

RustDesk menyambut baik kontribusi dari semua orang. Lihat [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) untuk membantu sebelum memulai.

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

## Publik Server Gratis

Di bawah ini adalah server yang bisa Anda gunakan secara gratis, dapat berubah seiring waktu. Jika Anda tidak dekat dengan salah satu dari ini, jaringan Anda mungkin lambat.
| Lokasi | Vendor | Spesifikasi |
| --------- | ------------- | ------------------ |
| Seoul | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Germany | Hetzner | 2 vCPU / 4GB RAM |
| Germany | Codext | 4 vCPU / 8GB RAM |
| Finland (Helsinki) | [Netlock](https://netlockendpoint.com) | 4 vCPU / 8GB RAM |
| USA (Ashburn) | [Netlock](https://netlockendpoint.com) | 4 vCPU / 8GB RAM |
| Ukraine (Kyiv) | [dc.volia](https://dc.volia.com) | 2 vCPU / 4GB RAM |

## Dependencies

Versi desktop menggunakan [sciter](https://sciter.com/) untuk GUI, silahkan download sendiri sciter dynamic library.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Langkah untuk RAW Build

- Siapkan env pengembangan Rust dan C++ build env

- Install [vcpkg](https://github.com/microsoft/vcpkg), dan arahkan `VCPKG_ROOT` env variable dengan benar

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus aom

- jalankan `cargo run`

## Bagaimana Build di Linux

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

### Install vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Perbaiki libvpx (Untuk Fedora)

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

### Build

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

### Ubah Wayland menjadi X11 (Xorg)

RustDesk tidak mendukung Wayland. Cek [ini](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) untuk mengonfigurasi Xorg sebagai sesi GNOME default.

## Bagaimana build dengan Docker

Mulailah dengan mengkloning repositori dan build dengan docker container:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Kemudian, setiap kali Anda perlu build aplikasi, jalankan perintah berikut:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Perhatikan bahwa build pertama mungkin memerlukan waktu lebih lama sebelum dependensi di-cache, build berikutnya akan lebih cepat. Selain itu, jika Anda perlu menentukan argumen yang berbeda untuk perintah build, Anda dapat melakukannya di akhir perintah di posisi `<OPTIONAL-ARGS>`. Misalnya, jika Anda ingin membangun versi rilis yang dioptimalkan, Anda akan menjalankan perintah di atas diikuti oleh `--release`. Hasil eksekusi akan tersedia pada target folder di sistem anda, dan dapat dijalankan dengan:

```sh
target/debug/rustdesk
```

Atau, jika Anda menjalankan rilis yang dapat dieksekusi:

```sh
target/release/rustdesk
```

Harap pastikan bahwa Anda menjalankan perintah ini dari root repositori RustDesk, jika tidak, aplikasi mungkin tidak dapat menemukan sumber daya yang diperlukan. Perhatikan juga perintah cargo seperti `install` atau `run` saat ini tidak didukung melalui metode ini karena mereka akan menginstal atau menjalankan program di dalam container bukan pada host.

## Struktur File

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions untuk transfer file, dan beberapa fungsi utilitas lainnya
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: spesifikasi platform keyboard/mouse control
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: audio/clipboard/input/video services, dan network connections
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Komunikasi dengan [rustdesk-server](https://github.com/rustdesk/rustdesk-server), menunggu untuk remote direct (TCP hole punching) atau relayed connection
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: kode khusus platform

## Snapshots

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
