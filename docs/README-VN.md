<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Phần mềm điểu khiển máy tính từ xa dành cho bạn"><br>
  <a href="#free-public-servers">Máy chủ</a> •
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Cấu trúc tệp tin</a> •
  <a href="#snapshot">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-GR.md">Ελληνικά</a>]<br>
  <b>Chúng tôi cần sự gíup đỡ của bạn để dịch trang README này, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> và <a href="https://github.com/rustdesk/doc.rustdesk.com">tài liệu</a> sang ngôn ngữ bản địa của bạn</b>
</p>

Chat với chúng tôi qua: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Một phần mềm điểu khiển máy tính từ xa, đuợc lập trình bằng ngôn ngữ Rust. Hoạt động tức thì, không cần phải cài đặt. Bạn có toàn quyền điểu khiển với dữ liệu của bạn mà không cần phải lo lắng về sự bảo mật. Bạn có thể sử dụng máy chủ rendezvous/relay của chúng tôi, [tự cài đặt máy chủ](https://rustdesk.com/server), hay thậm chí [tự tạo máy chủ rendezvous/relay](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

Mọi người đều đuợc chào đón để đóng góp vào RustDesk. Để bắt đầu, hãy đọc [`docs/CONTRIBUTING.md`](CONTRIBUTING.md).

[**RustDesk hoạt động như thế nào?**](https://github.com/rustdesk/rustdesk/wiki/How-does-RustDesk-work%3F)

[**CÁC BẢN PHÂN PHÁT MÃ NHỊ PHÂN**](https://github.com/rustdesk/rustdesk/releases)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Các Máy Chủ Công Khai Miễn Phí

Dưới đây là những máy chủ mà bạn có thể sử dụng mà không mất phí, chú ý là máy chủ có thể thay đổi theo thời gian. Nếu địa điểm của bạn không gần một trong số những máy chủ này, thì kết nói có thể chậm.

| Địa điểm | Nhà cung cấp | Cấu hình |
| --------- | ------------- | ------------------ |
| Seoul | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Germany | Hetzner | 2 vCPU / 4GB RAM |
| Germany | Codext | 4 vCPU / 8GB RAM |
| Finland (Helsinki) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| USA (Ashburn) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |

## Dependencies

Phiên bản cho máy tính sử dụng [sciter](https://sciter.com/) cho giao diện của phần mềm, vậy nên bạn cần tự tải về thư viện sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

Phiên bản cho điện thoại sử dụng Flutter. Chúng tôi sẽ chuyển sang sử dụng Flutter thay cho Sciter cho phiên bản máy tính.

## Cách để build

- Chuẩn bị môi trường phát triển Rust và môi trường build C++

- Tải và cài [vcpkg](https://github.com/microsoft/vcpkg), và đặt biến môi trường `VCPKG_ROOT` sao cho đúng.

  - Đối với Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
  - Đối với Linux/MacOS: vcpkg install libvpx libyuv opus

- Chạy lệnh `cargo run`

## [Build](https://rustdesk.com/docs/en/dev/build/)

## Cách để build cho Linux

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

### Cách cài vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
```

### Cách sửa lỗi libvpx (Dành cho hệ điều hành Fedora)

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

### Cách build

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

### Chuyển từ Wayland sang X11 (Xorg)

RustDesk hiện không hỗ trợ Wayland. Hãy xem [đường linh ở đây](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) cách để cài đặt Xorg làm session mặc định của GNOME.

## Cách để build sử dụng Docker

Bắt đầu bằng cách sao chép repo này về máy tính và build cái Docker cointainer:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Rồi mỗi khi bạn chạy ứng dụng, thì hãy chạy lệnh này:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Chú ý: Lần build đầu tiên có thể sẽ mất lâu hơn truớc khi các dependecies đuợc lưu lại, những lần build sau sẽ nhanh hơn. Hơn nũa, nếu bạn cần cung cấp các cài đặt lệnh khác cho lệnh build, bạn có thể đặt những cài đặt lệnh này vào cuối lệnh ở phần `<OPTIONAL-ARGS>`. Ví dụ nếu bạn cần build phiên bản đuợc tối ưu hóa, bạn sẽ chạy lệnh trên cùng với cài đặt lệnh ‘--release’. Kết quả build sẽ được lưu trong thư mục target trên máy tính của bạn, và có thể chạy với lệnh:

```sh
target/debug/rustdesk
```

Nếu bạn đang chạy bản build đuợc tối ưu hóa, thì bạn có thể chạy với lệnh:

```sh
target/release/rustdesk
```

Hãy đảm bảo là bạn đang chạy những lệnh này từ thu mục rễ của repo RustDesk, vì nếu không thì ứng dụng có thể sẽ không tìm đuợc những tệp tài nguyên cần thiết. Cũng như nhớ rằng những lệnh con của cargo như `install` hoặc `run` hiện chưa được hỗ trợ bởi phương pháp này vì chúng sẽ cài đặt hoặc chạy ứng dụng trong container thay vì trên máy tính của bạn.

## Cấu trúc tệp tin

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, cấu hình, tcp/udp wrapper, protobuf, fs functions để truyền file, và một số hàm tiện ích khác
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: để ghi lại màn hình
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: để điều khiển máy tính/con chuột trên những nền tảng khác nhau
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: giao diện người dùng
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: các dịch vụ âm thanh, clipboard, đầu vào, video và các kết nối mạng
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: để bắt đầu kết nối với một peer
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Để liên lạc với [rustdesk-server](https://github.com/rustdesk/rustdesk-server), đợi cho kết nối trực tiếp (TCP hole punching) hoặc kết nối được relayed.
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: mã nguồn riêng cho mỗi nền tảng
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Mã Flutter dành cho điện thoại
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Mã JavaScript dành cho giao diện trên web bằng Flutter

## Snapshot

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
