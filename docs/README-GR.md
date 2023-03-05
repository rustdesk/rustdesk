<p align="center">
  <img src="res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#Δωρεάν-δημόσιοι-διακομιστές">Διακομιστές</a> •
  <a href="#Γενικά-βήματα-ώστε-να-κάνετε-build">Build</a> •
  <a href="#Πως-να-κάνετε-build-στο-Docker">Docker</a> •
  <a href="#Δομή-φακέλων">Δομή</a> •
  <a href="#Στιγμιότυπα">Snapshot</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-FR.md">Français</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>]<br>
  <b>Χρειαζόμαστε τη βοήθειά σας για να μεταφράσουμε αυτό το αρχείο README, το <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> και το <a href="https://github.com/rustdesk/doc.rustdesk.com">Doc</a> στη μητρική σας γλώσσα</b>
</p>

Επικοινωνήστε μαζί μας μέσω: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk)

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/I2I04VU09)

Ένα λογισμικό απομακρυσμένης επιφάνειας εργασίας, γραμμένο σε γλώσσα Rust. Δεν χρειάζεται κάποια παραμετροποίηση, λειτουργεί αμέσως μετά την εγκατάσταση. Έχετε τον πλήρη έλεγχο των δεδομένων σας, χωρίς να ανησυχείτε για την ασφάλειά τους. Μπορείτε να χρησιμοποιήσετε τους προκαθορισμένους διακομιστές rendezvous/αναμετάδοσης, [να εγκαταστήσετε τον δικό σας διακομιστή](https://rustdesk.com/server), ή [να αναπτύξετε ένα δικό σας διακομιστή rendezvous/αναμετάδοσης](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

Το RustDesk ενθαρρύνει τη συνεισφορά όλων. Διαβάστε το [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) για βοήθεια στο πως να ξεκινήσετε.

[**Συχνές ερωτήσεις**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**Κατεβάστε τα αρχεία**](https://github.com/rustdesk/rustdesk/releases)

[**NIGHTLY BUILD**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)

## Δωρεάν δημόσιοι διακομιστές

Παρακάτω είναι οι διακομιστές που χρησιμοποιούνται δωρεάν, ενδέχεται να αλλάξουν με την πάροδο του χρόνου. Εάν δεν είστε κοντά σε ένα από αυτούς, το δίκτυό σας ίσως να είναι αργό.
| Περιοχή | Πάροχος | Προδιαγραφές |
| --------- | ------------- | ------------------ |
| Σεούλ | AWS lightsail | 1 vCPU / 0.5GB RAM |
| Γερμανία | Hetzner | 2 vCPU / 4GB RAM |
| Γερμανία | Codext | 4 vCPU / 8GB RAM |
| Φινλανδία (Ελσίνκι) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| ΗΠΑ (Άσμπερν) | 0x101 Cyber Security | 4 vCPU / 8GB RAM |
| Ουκρανία (Κίεβο) | dc.volia (2VM) | 2 vCPU / 4GB RAM |

## Dev Container

[![Open in Dev Containers](https://img.shields.io/static/v1?label=Dev%20Container&message=Open&color=blue&logo=visualstudiocode)](https://vscode.dev/redirect?url=vscode://ms-vscode-remote.remote-containers/cloneInVolume?url=https://github.com/rustdesk/rustdesk)

Αν έχετε εγκατεστημένα το VS Code και το Docker, μπορείτε να ξεκινήσετε κάνοντας κλικ στην παραπάνω εικόνα. Αυτό θα έχει ως αποτέλεσμα, το VS Code να εγκαταστήσει αυτόματα την επέκταση Dev Containers, εάν χρειάζεται, θα κλωνοποιήσει τον πηγαίο κώδικα σε έναν νέο container και θα εκκινήσει ένα Dev Container για χρήση προγραμματισμού.

Για περισσότερες πληροφορίες μεταβείτε στο [DEVCONTAINER.md](docs/DEVCONTAINER.md).

## Προαπαιτούμενα για build  

Στις παραθυρικές εκδόσεις χρησιμοποιείται είτε το [sciter](https://sciter.com/) είτε το Flutter, τα παρακάτω βήματα είναι μόνο για το Sciter.

Παρακαλώ κατεβάστε μόνοι σας την δυναμική βιβλιοθήκη sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[MacOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Γενικά βήματα ώστε να κάνετε build

- Προετοιμάστε τα περιβάλλοντα προγραμματισμού Rust και C++

- Εγκαταστήσετε το [vcpkg](https://github.com/microsoft/vcpkg), και ρυθμίστε σωστά την παράμετρο συστήματος `VCPKG_ROOT`

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static
  - Linux/MacOS: vcpkg install libvpx libyuv opus

- Εκτελέστε `cargo run`

## [Build](https://rustdesk.com/docs/en/dev/build/)

## Πως να το κάνετε build στο Linux

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

### Εγκατάσταση vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2021.12.01
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus
```

### Διόρθωση libvpx (για Fedora)

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

### Αλλαγή του Wayland σε X11 (Xorg)

Το RustDesk δεν υποστιρίζει το πρωτόκολλο Wayland. Διαβάστε [εδώ](https://docs.fedoraproject.org/en-US/quick-docs/configuring-xorg-as-default-gnome-session/) ώστε να ορίσετε το Xorg ως το προκαθορισμένο GNOME περιβάλλον.

## Υποστήριξη Wayland

Το Wayland προς το παρόν δεν διαθέτει κάποιο API το οποίο να στέλνει τα πατήματα πλήκτρων στα υπόλοιπα παράθυρα. Για τον λόγο αυτό, το Rustdesk χρησιμοποιεί ένα API από κατώτερο επίπεδο, όπως το `/dev/uinput` (Linux kernel level).

Σε περίπτωση που το Wayland είναι η ελεγχόμενη πλευρά, θα πρέπει να ξεκινήσετε με τον παρακάτω τρόπο:
```bash
# Start uinput service
$ sudo rustdesk --service
$ rustdesk
```
**Σημείωση**: Η εγγραφή οθόνης του Wayland χρησιμοποιεί διαφορετικές διεπαφές. Το RustDesk προς το παρόν υποστηρίζει μόνο org.freedesktop.portal.ScreenCast.
```bash
$ dbus-send --session --print-reply       \
  --dest=org.freedesktop.portal.Desktop \
  /org/freedesktop/portal/desktop       \
  org.freedesktop.DBus.Properties.Get   \
  string:org.freedesktop.portal.ScreenCast string:version
# Not support
Error org.freedesktop.DBus.Error.InvalidArgs: No such interface “org.freedesktop.portal.ScreenCast”
# Support
method return time=1662544486.931020 sender=:1.54 -> destination=:1.139 serial=257 reply_serial=2
   variant       uint32 4
```

## Πως να κάνετε build στο Docker

Ξεκινήστε κλωνοποιώντας το αποθετήριο και κάνοντας build το docker container:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
docker build -t "rustdesk-builder" .
```

Στη συνέχεια, κάθε φορά που επιθυμείτε να κάνετε build την εφαρμογή, εκτελέστε την ακόλουθη εντολή:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Σημειώστε ότι το πρώτο build μπορεί να διαρκέσει περισσότερο, ώστε να αποθηκευτούν στην προσωρινή μνήμη οι εξαρτήσεις, τα επόμενα build θα είναι ταχύτερα. Επιπλέον, εάν πρέπει να καθορίσετε διαφορετικές παραμέτρους στην εντολή build, μπορείτε να το κάνετε στο τέλος της εντολής με την χρήση `<OPTIONAL-ARGS>`. Για παράδειγμα, εάν επιθυμείτε να δημιουργήσετε μια βελτιστοποιημένη έκδοση της εφαρμογής, θα εκτελέσετε την παραπάνω εντολή ακολουθούμενη από το `--release`. Το εκτελέσιμο αρχείο θα είναι διαθέσιμο στον προκαθορισμένο φάκελο στο σύστημά σας και μπορεί να εκτελεστεί με:

```sh
target/debug/rustdesk
```

Ή στην περίπτωση μιας βελτιστοποιημένης έκδοσης της εφαρμογής εκτελέστε:

```sh
target/release/rustdesk
```

Βεβαιωθείτε ότι εκτελείτε αυτές τις εντολές από την αρχική διαδρομή του αποθετηρίου του Rustdesk, διαφορετικά η εφαρμογή ενδέχεται να μην είναι σε θέση να βρει τους απαιτούμενους πόρους. Σημειώστε επίσης ότι άλλες υποεντολές, όπως το `install` ή το `run` δεν υποστηρίζονται επί του παρόντος μέσω αυτής της μεθόδου καθώς θα εγκαταστήσουν ή θα εκτελέσουν το πρόγραμμα εντός του container αντί του κεντρικού υπολογιστή.

## Δομή φακέλων

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions for file transfer, and some other utility functions
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platform specific keyboard/mouse control
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: GUI
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: audio/clipboard/input/video services, and network connections
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Communicate with [rustdesk-server](https://github.com/rustdesk/rustdesk-server), wait for remote direct (TCP hole punching) or relayed connection
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platform specific code
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter code for mobile
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/js)**: JavaScript for Flutter web client

## Στιγμιότυπα

![image](https://user-images.githubusercontent.com/71636191/113112362-ae4deb80-923b-11eb-957d-ff88daad4f06.png)

![image](https://user-images.githubusercontent.com/71636191/113112619-f705a480-923b-11eb-911d-97e984ef52b6.png)

![image](https://user-images.githubusercontent.com/71636191/113112857-3fbd5d80-923c-11eb-9836-768325faf906.png)

![image](https://user-images.githubusercontent.com/71636191/135385039-38fdbd72-379a-422d-b97f-33df71fb1cec.png)
