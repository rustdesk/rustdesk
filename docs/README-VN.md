

<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#free-public-servers">Server</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Chúng tôi rất hoan nghênh sự hỗ trợ của bạn trong việc dịch trang README, trang giao diện người dùng của RustDesk - <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> và trang tài liệu của RustDesk - <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a> sang Tiếng Việt</b>
</p>

Hãy trao đổi với chúng tôi qua: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

RustDesk là một phần mềm điểu khiển máy tính từ xa mã nguồn mở, được viết bằng Rust. Nó hoạt động ngay sau khi cài đặt, không yêu cầu cấu hình phức tạp. Bạn có toàn quyền kiểm soát với dữ liệu của mình mà không cần phải lo lắng về vấn đề bảo mật. Bạn có thể sử dụng máy chủ rendezvous/relay của chúng tôi hoặc [tự cài đặt máy chủ của riêng mình](https://rustdesk.com/server) hay thậm chí [tự tạo máy chủ rendezvous/relay cho riêng bạn](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

**RustDesk** luôn hoan nghênh mọi đóng góp từ mọi người. Hãy xem tệp [`docs/CONTRIBUTING.md`](CONTRIBUTING.md) để bắt đầu. 

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)
[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)
[**NIGHTLY BUILD**](https://github.com/rustdesk/rustdesk/FAQreleases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Dependencies

Phiên bản máy tính sử dụng __Flutter__ hoặc __Sciter__ (đã lỗi thời) cho giao diện người dùng (GUI). Hướng dẫn này chỉ áp dụng cho phiên bản Sciter, vì nó thân thiện và dễ bắt đầu hơn. Hãy kiểm tra [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) của chúng tôi để xây dựng phiên bản Flutter.

Vui lòng tự tải thư viện `Sciter` về máy theo hướng dẫn cho từng hệ điều hành.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) | [Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) | [MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Các bước build cơ bản

- Chuẩn bị môi trường phát triển Rust và môi trường biên dịch C++

- Tải và cài đặt [`vcpkg`](https://github.com/microsoft/vcpkg), và thiết lập biến môi trường `VCPKG_ROOT`.

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
  - Linux/MacOS: `vcpkg install libvpx libyuv opus aom`
- Chạy lệnh `cargo run`

## [Build](https://rustdesk.com/docs/en/dev/build/)

## Cách build cho Linux

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

### Cách cài đặt `vcpkg`

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Cách sửa lỗi `libvpx` (Dành cho hệ điều hành Fedora)

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

## Cách build bằng Docker

Bắt đầu bằng cách sao chép repo này về máy tính của bạn và tạo Docker container:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Sau đó, mỗi khi bạn chạy ứng dụng, thì hãy chạy dòng lệnh sau:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Lưu ý rằng **lần build đầu tiên có thể mất thời gian hơn trước khi các dependencies được lưu vào bộ nhớ cache**, nhưng các lần build sau sẽ nhanh hơn. Ngoài ra, nếu bạn cần chỉ định các đối số khác cho lệnh build, bạn có thể thêm chúng vào cuối lệnh ở phần `<OPTIONAL-ARGS>`. Ví dụ, nếu bạn muốn build phiên bản tối ưu hóa, bạn sẽ chạy lệnh trên với tùy chọn `--release`. Kết quả biên dịch sẽ được lưu trong thư mục target trên máy tính của bạn, và có thể chạy với lệnh:

```sh
target/debug/rustdesk
```

Nếu bạn đang chạy bản build được tối ưu hóa, thì bạn có thể chạy với lệnh:

```sh
target/release/rustdesk
```

Hãy đảm bảo rằng bạn đang chạy các lệnh này từ gốc của thư mục **RustDesk**, nếu không, ứng dụng có thể không thể tìm thấy các tệp tài nguyên cần thiết. Hãy lưu ý rằng các câu lệnh con khác của **cargo** như **install** hoặc **run** hiện không được hỗ trợ qua phương pháp này, vì chúng sẽ cài đặt hoặc chạy chương trình bên trong **container** thay vì trên máy tính của bạn.

## Cấu trúc tệp tin

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, cấu hình, tcp/udp wrapper, protobuf, fs functions để truyền file, và một số hàm tiện ích khác
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: ghi lại màn hình
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: điều khiển máy tính/chuột trên các nền tảng khác nhau
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: giao diện người dùng
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: các dịch vụ âm thanh, clipboard, đầu vào, video và các kết nối mạng
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: bắt đầu kết nối với một peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: giao tiếp với [rustdesk-server](https://github.com/rustdesk/rustdesk-server), đợi kết nối trực tiếp (TCP hole punching) hoặc kết nối được chuyển tiếp.
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: mã nguồn riêng cho mỗi nền tảng
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Mã Flutter dành máy tính và điện thoại
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Mã JavaScript dành cho giao diện trên web bằng Flutter

## Snapshot

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
