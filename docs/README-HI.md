<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - आपका रिमोट डेस्कटॉप"><br>
  <a href="#बिल्ड-करने-के-मूल-चरण">बिल्ड</a> •
  <a href="#docker-के-साथ-कैसे-बिल्ड-करें">Docker</a> •
  <a href="#फ़ाइल-संरचना">संरचना</a> •
  <a href="#स्क्रीनशॉट">स्क्रीनशॉट</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>] | [<a href="README-BN.md">বাংলা</a>] | [<a href="README-UR.md">اردو</a>] | [<a href="README-TH.md">ไทย</a>] | [<a href="README-ZHTW.md">繁體中文</a>]<br>
  <b>इस README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> और <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk दस्तावेज़</a> का आपकी मातृभाषा में अनुवाद करने के लिए हमें आपकी मदद चाहिए</b>
</p>

> [!Caution]
> **दुरुपयोग अस्वीकरण:** <br>
> RustDesk के डेवलपर इस सॉफ़्टवेयर के किसी भी अनैतिक या अवैध उपयोग का समर्थन नहीं करते। अनधिकृत पहुँच, नियंत्रण या निजता का उल्लंघन जैसा दुरुपयोग हमारे दिशानिर्देशों के सख़्त ख़िलाफ़ है। एप्लिकेशन के किसी भी दुरुपयोग के लिए लेखक ज़िम्मेदार नहीं हैं।


हमसे बात करें: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Rust में लिखा गया एक और रिमोट डेस्कटॉप समाधान। यह बिना किसी कॉन्फ़िगरेशन के तुरंत काम करता है। आपके डेटा पर आपका पूरा नियंत्रण रहता है, सुरक्षा की कोई चिंता नहीं। आप हमारे rendezvous/relay सर्वर का उपयोग कर सकते हैं, [अपना सर्वर सेट अप कर सकते हैं](https://rustdesk.com/server), या [अपना rendezvous/relay सर्वर लिख सकते हैं](https://github.com/rustdesk/rustdesk-server-demo)।

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk हर किसी के योगदान का स्वागत करता है। शुरुआत करने के लिए [CONTRIBUTING.md](CONTRIBUTING.md) देखें।

[**अक्सर पूछे जाने वाले प्रश्न**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**बाइनरी डाउनलोड**](https://github.com/rustdesk/rustdesk/releases)

[**नाइटली बिल्ड**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## निर्भरताएँ

डेस्कटॉप संस्करण GUI के लिए Flutter या Sciter (अप्रचलित) का उपयोग करते हैं। यह ट्यूटोरियल केवल Sciter के लिए है, क्योंकि इससे शुरुआत करना आसान और सुविधाजनक है। Flutter संस्करण बिल्ड करने के लिए हमारा [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) देखें।

कृपया Sciter डायनामिक लाइब्रेरी स्वयं डाउनलोड करें।

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## बिल्ड करने के मूल चरण

- अपना Rust डेवलपमेंट एनवायरनमेंट और C++ बिल्ड एनवायरनमेंट तैयार करें

- [vcpkg](https://github.com/microsoft/vcpkg) इंस्टॉल करें और `VCPKG_ROOT` एनवायरनमेंट वेरिएबल सही ढंग से सेट करें

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- `cargo run` चलाएँ

## [बिल्ड](https://rustdesk.com/docs/en/dev/build/)

## Linux पर कैसे बिल्ड करें

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

### vcpkg इंस्टॉल करें

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### libvpx ठीक करें (Fedora के लिए)

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

### बिल्ड

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

## Docker के साथ कैसे बिल्ड करें

सबसे पहले रिपॉज़िटरी क्लोन करें और Docker कंटेनर बिल्ड करें:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

इसके बाद, हर बार जब आपको एप्लिकेशन बिल्ड करना हो, यह कमांड चलाएँ:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

ध्यान दें कि पहले बिल्ड में अधिक समय लग सकता है, क्योंकि तब तक निर्भरताएँ कैश नहीं हुई होतीं; बाद के बिल्ड तेज़ होंगे। इसके अलावा, यदि आपको बिल्ड कमांड में अलग आर्ग्युमेंट देने हों, तो आप उन्हें कमांड के अंत में `<OPTIONAL-ARGS>` की जगह दे सकते हैं। उदाहरण के लिए, यदि आप ऑप्टिमाइज़ किया गया रिलीज़ संस्करण बिल्ड करना चाहते हैं, तो ऊपर दी गई कमांड के बाद `--release` जोड़कर चलाएँ। तैयार एक्ज़ीक्यूटेबल आपके सिस्टम पर target फ़ोल्डर में मिलेगा और इसे इस तरह चलाया जा सकता है:

```sh
target/debug/rustdesk
```

या, यदि आप रिलीज़ एक्ज़ीक्यूटेबल चला रहे हैं:

```sh
target/release/rustdesk
```

कृपया सुनिश्चित करें कि आप ये कमांड RustDesk रिपॉज़िटरी के रूट से चला रहे हैं, अन्यथा एप्लिकेशन को ज़रूरी संसाधन नहीं मिल सकते। यह भी ध्यान दें कि cargo के अन्य सबकमांड, जैसे `install` या `run`, इस तरीके से अभी समर्थित नहीं हैं, क्योंकि वे प्रोग्राम को होस्ट के बजाय कंटेनर के अंदर इंस्टॉल करेंगे या चलाएँगे।

## फ़ाइल संरचना

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: वीडियो कोडेक, कॉन्फ़िग, tcp/udp रैपर और सर्वर के साथ साझा किए जाने वाले कुछ अन्य यूटिलिटी फ़ंक्शन
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)**: protobuf, फ़ाइल ट्रांसफ़र के लिए fs फ़ंक्शन, कीबोर्ड और प्लेटफ़ॉर्म कोड, जिनका उपयोग केवल यही ऐप करता है
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: स्क्रीन कैप्चर
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: प्लेटफ़ॉर्म-विशिष्ट कीबोर्ड/माउस नियंत्रण
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: Windows, Linux, macOS के लिए फ़ाइल कॉपी और पेस्ट का कार्यान्वयन।
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: पुराना Sciter UI (अप्रचलित)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: ऑडियो/क्लिपबोर्ड/इनपुट/वीडियो सेवाएँ और नेटवर्क कनेक्शन
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: पीयर कनेक्शन शुरू करना
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: [rustdesk-server](https://github.com/rustdesk/rustdesk-server) से संवाद करता है, रिमोट डायरेक्ट (TCP hole punching) या रिले कनेक्शन की प्रतीक्षा करता है
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: प्लेटफ़ॉर्म-विशिष्ट कोड
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: डेस्कटॉप और मोबाइल के लिए Flutter कोड

## स्क्रीनशॉट

![कनेक्शन मैनेजर](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Windows PC से कनेक्टेड](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![फ़ाइल ट्रांसफ़र](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP टनलिंग](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
