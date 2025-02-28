
<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Uzak masaüstü uygulamanız"><br>
  <a href="#free-public-servers">Sunucular</a> •
  <a href="#raw-steps-to-build">Derleme</a> •
  <a href="#how-to-build-with-docker">Docker ile Derleme</a> •
  <a href="#file-structure">Dosya Yapısı</a> •
  <a href="#snapshot">Ekran Görüntüleri</a><br>
  [<a href="docs/README-UA.md">Українська</a>] | [<a href="docs/README-CS.md">česky</a>] | [<a href="docs/README-ZH.md">中文</a>] | [<a href="docs/README-HU.md">Magyar</a>] | [<a href="docs/README-ES.md">Español</a>] | [<a href="docs/README-FA.md">فارسی</a>] | [<a href="docs/README-FR.md">Français</a>] | [<a href="docs/README-DE.md">Deutsch</a>] | [<a href="docs/README-PL.md">Polski</a>] | [<a href="docs/README-ID.md">Indonesian</a>] | [<a href="docs/README-FI.md">Suomi</a>] | [<a href="docs/README-ML.md">മലയാളം</a>] | [<a href="docs/README-JP.md">日本語</a>] | [<a href="docs/README-NL.md">Nederlands</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Русский</a>] | [<a href="docs/README-PTBR.md">Português (Brasil)</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">한국어</a>] | [<a href="docs/README-AR.md">العربي</a>] | [<a href="docs/README-VN.md">Tiếng Việt</a>] | [<a href="docs/README-DA.md">Dansk</a>] | [<a href="docs/README-GR.md">Ελληνικά</a>]<br>
  <b>README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> ve <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Belge</a>'sini ana dilinize çevirmemiz için yardımınıza ihtiyacımız var</b>
</p>

Bizimle sohbet edin: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Başka bir uzak masaüstü yazılımı daha, Rust dilinde yazılmış. Hemen kullanıma hazır, hiçbir yapılandırma gerektirmez. Verilerinizin tam kontrolünü elinizde tutarsınız ve güvenlikle ilgili endişeleriniz olmaz. Kendi buluş/iletme sunucumuzu kullanabilirsiniz, [kendi sunucunuzu kurabilirsiniz](https://rustdesk.com/server) veya [kendi buluş/iletme sunucunuzu yazabilirsiniz](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk, herkesten katkıyı kabul eder. Başlamak için [CONTRIBUTING.md](docs/CONTRIBUTING-TR.md) belgesine göz atın.

[**SSS**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**BİNARİ İNDİR**](https://github.com/rustdesk/rustdesk/releases)

[**NİGHTLY DERLEME**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="F-Droid'de Alın"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Bağımlılıklar

Masaüstü sürümleri GUI için

 [Sciter](https://sciter.com/) veya Flutter kullanır, bu kılavuz sadece Sciter içindir.

Lütfen Sciter dinamik kütüphanesini kendiniz indirin.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Temel Derleme Adımları

- Rust geliştirme ortamınızı ve C++ derleme ortamınızı hazırlayın.

- [vcpkg](https://github.com/microsoft/vcpkg) yükleyin ve `VCPKG_ROOT` çevresel değişkenini doğru bir şekilde ayarlayın.

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- `cargo run` komutunu çalıştırın.

## [Derleme](https://rustdesk.com/docs/en/dev/build/)

## Linux Üzerinde Derleme Nasıl Yapılır

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
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel
```

### Arch (Manjaro)

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### vcpkg'yi Yükleyin

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx'i Düzeltin (Fedora için)

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

### Derleme

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

## Docker ile Derleme Nasıl Yapılır

Öncelikle deposunu klonlayın ve Docker konteynerini oluşturun:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Ardından, uygulamayı derlemek için her seferinde aşağıdaki komutu çalıştırın:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

İlk derleme, bağımlılıklar önbelleğe alınmadan önce daha uzun sürebilir, sonraki derlemeler daha hızlı olacaktır. Ayrıca, derleme komutuna isteğe bağlı argümanlar belirtmeniz gerekiyorsa, bunu

 komutun sonunda `<İSTEĞE BAĞLI-ARGÜMANLAR>` pozisyonunda yapabilirsiniz. Örneğin, optimize edilmiş bir sürümü derlemek isterseniz, yukarıdaki komutu çalıştırdıktan sonra `--release` ekleyebilirsiniz. Oluşan yürütülebilir dosya sisteminizdeki hedef klasöründe bulunacak ve şu komutla çalıştırılabilir:

```sh
target/debug/rustdesk
```

Veya, yayın yürütülebilir dosyası çalıştırılıyorsa:

```sh
target/release/rustdesk
```

Lütfen bu komutları RustDesk deposunun kökünden çalıştırdığınızdan emin olun, aksi takdirde uygulama gereken kaynakları bulamayabilir. Ayrıca, `install` veya `run` gibi diğer cargo altkomutları şu anda bu yöntem aracılığıyla desteklenmemektedir, çünkü bunlar programı konteyner içinde kurar veya çalıştırır ve ana makinede değil.
 
## Dosya Yapısı

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video kodlayıcı, yapılandırma, tcp/udp sarmalayıcı, protobuf, dosya transferi için fs işlevleri ve diğer bazı yardımcı işlevler
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: ekran yakalama
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platforma özgü klavye/fare kontrolü
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: ses/pasta/klavye/video hizmetleri ve ağ bağlantıları
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: bir eş bağlantısı başlatır
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server) ile iletişim kurar, uzak doğrudan (TCP delik vurma) veya iletme bağlantısını bekler
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platforma özgü kod
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: mobil için Flutter kodu
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: Flutter web istemcisi için JavaScript

> [!Dikkat]
> **Yanlış Kullanım Uyarısı:** <br>
> RustDesk geliştiricileri, bu yazılımın etik olmayan veya yasa dışı kullanımını onaylamaz veya desteklemez. Yetkisiz erişim, kontrol veya gizlilik ihlali gibi kötüye kullanımlar kesinlikle yönergelerimize aykırıdır. Yazarlar, uygulamanın herhangi bir yanlış kullanımından sorumlu değildir.

## Ekran Görüntüleri

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
```
