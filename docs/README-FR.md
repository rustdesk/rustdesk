<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Votre bureau à distance"><br>
  <a href="#étapes-de-compilation-de-base">Compilation</a> •
  <a href="#comment-compiler-avec-docker">Docker</a> •
  <a href="#structure-du-projet">Structure</a> •
  <a href="#captures-décran">Captures d'écran</a><br>
  [<a href="../README.md">English</a>] | [<a href="README-UA.md">Українська</a>] | [<a href="README-CS.md">česky</a>] | [<a href="README-ZH.md">中文</a>] | [<a href="README-HU.md">Magyar</a>] | [<a href="README-ES.md">Español</a>] | [<a href="README-FA.md">فارسی</a>] | [<a href="README-DE.md">Deutsch</a>] | [<a href="README-PL.md">Polski</a>] | [<a href="README-ID.md">Indonesian</a>] | [<a href="README-FI.md">Suomi</a>] | [<a href="README-ML.md">മലയാളം</a>] | [<a href="README-JP.md">日本語</a>] | [<a href="README-NL.md">Nederlands</a>] | [<a href="README-IT.md">Italiano</a>] | [<a href="README-RU.md">Русский</a>] | [<a href="README-PTPT.md">Português (Portugal)</a>] | [<a href="README-PTBR.md">Português (Brasil)</a>] | [<a href="README-EO.md">Esperanto</a>] | [<a href="README-KR.md">한국어</a>] | [<a href="README-AR.md">العربي</a>] | [<a href="README-VN.md">Tiếng Việt</a>] | [<a href="README-DA.md">Dansk</a>] | [<a href="README-GR.md">Ελληνικά</a>] | [<a href="README-TR.md">Türkçe</a>] | [<a href="README-NO.md">Norsk</a>] | [<a href="README-RO.md">Română</a>] | [<a href="README-KA.md">ქართული</a>]<br>
  <b>Nous avons besoin de votre aide pour traduire ce README, l'<a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">interface de RustDesk</a> et la <a href="https://github.com/rustdesk/doc.rustdesk.com">documentation de RustDesk</a> dans votre langue maternelle</b>
</p>

> [!Caution]
> **Avertissement contre l'utilisation abusive :** <br>
> Les développeurs de RustDesk ne cautionnent ni ne soutiennent aucune utilisation contraire à l'éthique ou illégale de ce logiciel. Toute utilisation abusive, telle que l'accès ou le contrôle non autorisé, ou l'atteinte à la vie privée, est strictement contraire à nos règles. Les auteurs ne sont pas responsables de l'utilisation abusive de l'application.


Discutez avec nous : [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Fonctionnalit%C3%A9s%20avanc%C3%A9es-blue)](https://rustdesk.com/pricing.html)

Encore une solution de bureau à distance, écrite en Rust. Elle fonctionne immédiatement, sans aucune configuration. Vous gardez le contrôle total de vos données, sans souci de sécurité. Vous pouvez utiliser notre serveur de rendez-vous/relais, [installer le vôtre](https://rustdesk.com/server) ou [écrire votre propre serveur de rendez-vous/relais](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk accueille les contributions de tout le monde. Consultez [CONTRIBUTING-FR.md](CONTRIBUTING-FR.md) pour bien démarrer.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**TÉLÉCHARGER LES BINAIRES**](https://github.com/rustdesk/rustdesk/releases)

[**VERSION NIGHTLY**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on-fr.png"
    alt="Disponible sur F-Droid"
    height="80">](https://f-droid.org/fr/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=fr"
    alt="Disponible sur Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Dépendances

Les versions de bureau utilisent Flutter ou Sciter (obsolète) pour l'interface graphique. Ce tutoriel ne concerne que Sciter, plus simple et plus accessible pour débuter. Consultez notre [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) pour compiler la version Flutter.

Veuillez télécharger vous-même la bibliothèque dynamique Sciter.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Étapes de compilation de base

- Préparez votre environnement de développement Rust et votre environnement de compilation C++

- Installez [vcpkg](https://github.com/microsoft/vcpkg) et définissez correctement la variable d'environnement `VCPKG_ROOT`

  - Windows : vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS : vcpkg install libvpx libyuv opus aom

- Lancez `cargo run`

## [Compilation](https://rustdesk.com/docs/fr/dev/build/)

## Comment compiler sous Linux

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

### Installer vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Corriger libvpx (pour Fedora)

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

### Compiler

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

## Comment compiler avec Docker

Commencez par cloner le dépôt et construire le conteneur Docker :

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Ensuite, chaque fois que vous devez compiler l'application, exécutez la commande suivante :

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Notez que la première compilation peut être plus longue, le temps de mettre les dépendances en cache ; les compilations suivantes seront plus rapides. Si vous devez passer d'autres arguments à la commande de compilation, ajoutez-les à la fin de la commande, à la position `<OPTIONAL-ARGS>`. Par exemple, pour compiler une version release optimisée, exécutez la commande ci-dessus suivie de `--release`. L'exécutable produit se trouve dans le dossier `target` de votre système et peut être lancé avec :

```sh
target/debug/rustdesk
```

Ou, s'il s'agit d'un exécutable release :

```sh
target/release/rustdesk
```

Veillez à exécuter ces commandes depuis la racine du dépôt RustDesk, sinon l'application risque de ne pas trouver les ressources nécessaires. Notez également que les autres sous-commandes de cargo, comme `install` ou `run`, ne sont pas prises en charge par cette méthode : elles installeraient ou lanceraient le programme dans le conteneur et non sur l'hôte.

## Structure du projet

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)** : codec vidéo, configuration, surcouche TCP/UDP et autres fonctions utilitaires partagées avec le serveur
- **[libs/base](https://github.com/rustdesk/rustdesk/tree/master/libs/base)** : protobuf, fonctions de système de fichiers pour le transfert de fichiers, code clavier et code de plateforme utilisés uniquement par cette application
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)** : capture d'écran
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)** : contrôle du clavier et de la souris, spécifique à chaque plateforme
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)** : copier-coller de fichiers pour Windows, Linux et macOS
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)** : ancienne interface Sciter (obsolète)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)** : services audio, presse-papiers, entrées et vidéo, et connexions réseau
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)** : établir une connexion avec un pair
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)** : communiquer avec [rustdesk-server](https://github.com/rustdesk/rustdesk-server), attendre une connexion distante directe (TCP hole punching) ou relayée
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)** : code spécifique à chaque plateforme
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)** : code Flutter pour les versions de bureau et mobile

## Captures d'écran

![Gestionnaire de connexions](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connecté à un PC Windows](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![Transfert de fichiers](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![Tunnel TCP](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
