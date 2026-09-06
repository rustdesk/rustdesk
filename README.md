<p align="center">
  <img src="res/logo-header.svg" alt="RustDesk - Your remote desktop"><br>
  <a href="#raw-steps-to-build">Build</a> •
  <a href="#how-to-build-with-docker">Docker</a> •
  <a href="#file-structure">Structure</a> •
  <a href="#screenshots">Screenshots</a><br>
  [<a href="docs/README-UA.md">Українська</a>] | [<a href="docs/README-CS.md">česky</a>] | [<a href="docs/README-ZH.md">中文</a>] | [<a href="docs/README-HU.md">Magyar</a>] | [<a href="docs/README-ES.md">Español</a>] | [<a href="docs/README-FA.md">فارسی</a>] | [<a href="docs/README-FR.md">Français</a>] | [<a href="docs/README-DE.md">Deutsch</a>] | [<a href="docs/README-PL.md">Polski</a>] | [<a href="docs/README-ID.md">Indonesian</a>] | [<a href="docs/README-FI.md">Suomi</a>] | [<a href="docs/README-ML.md">മലയാളം</a>] | [<a href="docs/README-JP.md">日本語</a>] | [<a href="docs/README-NL.md">Nederlands</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Русский</a>] | [<a href="docs/README-PTBR.md">Português (Brasil)</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">한국어</a>] | [<a href="docs/README-AR.md">العربي</a>] | [<a href="docs/README-VN.md">Tiếng Việt</a>] | [<a href="docs/README-DA.md">Dansk</a>] | [<a href="docs/README-GR.md">Ελληνικά</a>] | [<a href="docs/README-TR.md">Türkçe</a>] | [<a href="docs/README-NO.md">Norsk</a>] | [<a href="docs/README-RO.md">Română</a>]<br>
  <b>We need your help to translate this README, <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">RustDesk UI</a> and <a href="https://github.com/rustdesk/doc.rustdesk.com">RustDesk Doc</a> to your native language</b>
</p>

> [!Caution]
> **Misuse Disclaimer:** <br>
> The developers of RustDesk do not condone or support any unethical or illegal use of this software. Misuse, such as unauthorized access, control or invasion of privacy, is strictly against our guidelines. The authors are not responsible for any misuse of the application.


Chat with us: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Yet another remote desktop solution, written in Rust. Works out of the box with no configuration required. You have full control of your data, with no concerns about security. You can use our rendezvous/relay server, [set up your own](https://rustdesk.com/server), or [write your own rendezvous/relay server](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

RustDesk welcomes contribution from everyone. See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for help getting started.

[**FAQ**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**BINARY DOWNLOAD**](https://github.com/rustdesk/rustdesk/releases)

[**NIGHTLY BUILD**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Get it on F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Get it on Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Dependencies

Desktop versions use Flutter or Sciter (deprecated) for GUI. This tutorial is for Sciter only, since it is easier and more friendly to start. Check out our [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) for building the Flutter version.

Please download Sciter dynamic library yourself.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Raw Steps to build

- Prepare your Rust development env and C++ build env

- Install [vcpkg](https://github.com/microsoft/vcpkg), and set `VCPKG_ROOT` env variable correctly

  - Windows: vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static
  - Linux/macOS: vcpkg install libvpx libyuv opus aom

- run `cargo run`

## [Build](https://rustdesk.com/docs/en/dev/build/)

## How to Build on Linux

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

### Fix libvpx (For Fedora)

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
git clone --recurse-submodules https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## How to build with Docker

Begin by cloning the repository and building the Docker container:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Then, each time you need to build the application, run the following command:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Note that the first build may take longer before dependencies are cached, subsequent builds will be faster. Additionally, if you need to specify different arguments to the build command, you may do so at the end of the command in the `<OPTIONAL-ARGS>` position. For instance, if you wanted to build an optimized release version, you would run the command above followed by `--release`. The resulting executable will be available in the target folder on your system, and can be run with:

```sh
target/debug/rustdesk
```

Or, if you're running a release executable:

```sh
target/release/rustdesk
```

Please ensure that you run these commands from the root of the RustDesk repository, or the application may not find the required resources. Also note that other cargo subcommands such as `install` or `run` are not currently supported via this method as they would install or run the program inside the container instead of the host.

## File Structure

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: video codec, config, tcp/udp wrapper, protobuf, fs functions for file transfer, and some other utility functions
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: screen capture
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: platform specific keyboard/mouse control
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: file copy and paste implementation for Windows, Linux, macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: obsolete Sciter UI (deprecated)
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: audio/clipboard/input/video services, and network connections
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: start a peer connection
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Communicate with [rustdesk-server](https://github.com/rustdesk/rustdesk-server), wait for remote direct (TCP hole punching) or relayed connection
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: platform specific code
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: Flutter code for desktop and mobile

## Screenshots

![Connection Manager](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Connected to a Windows PC](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![File Transfer](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![TCP Tunneling](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)



## 🌐 Web Resources & Interactive Index
- [TANK ATTACK 5](https://iskillquest.pages.dev/tank-attack-5.html)
- [BLOCK UP](https://studyquests.github.io/block-up.html)
- [COLLECT BRAINROT ARENA](https://studyplayings.web.app/collect-brainrot-arena.html)
- [DINO SHOOTER PRO](https://learnquester.github.io/dino-shooter-pro.html)
- [ARCHERY RAGDOLL](https://learnquester.github.io/archery-ragdoll.html)
- [OMG WORD SUSHI](https://studyplayings.web.app/omg-word-sushi.html)
- [STICK WAR SAGA](https://learnquester.github.io/stick-war-saga.html)
- [LIVE 100 DAYS](https://studyplayings.web.app/live-100-days.html)
- [CATEGORY BOOKMARKLET](https://thelearnquester.web.app/category-bookmarklet.html)
- [LOGIC STORM ANIMALS PUZZLE](https://studyplayings.web.app/logic-storm-animals-puzzle.html)
- [LAST PLAY RAGDOLL SANDBOX KQB](https://studyplayings.web.app/last-play-ragdoll-sandbox-kqb.html)
- [BARBIECORE AESTHETICS](https://learnquester.github.io/barbiecore-aesthetics.html)
- [AIRPORT MASTER PLANE TYCOON](https://studyplayings.web.app/airport-master-plane-tycoon.html)
- [PUZZLE BLOCKS FILL IT COMPLETELY](https://studyplayings.web.app/puzzle-blocks-fill-it-completely.html)
- [POPCAT CLICKER](https://learnquester.github.io/popcat-clicker.html)
- [FARM OF WORDS](https://studyplayings.web.app/farm-of-words.html)
- [PALKOVIL THE WAY HOME](https://learnquester.github.io/palkovil-the-way-home.html)
- [SQUISHY TABA PAW ASMR](https://studyplayings.web.app/squishy-taba-paw-asmr.html)
- [MEMORY MATCH MAGIC](https://learnquester.github.io/memory-match-magic.html)
- [OBBY GYM SIMULATOR ESCAPE](https://learnquester.github.io/obby-gym-simulator-escape.html)
- [STICKMAN RAGDOLL PLAYGROUND](https://learnquester.github.io/stickman-ragdoll-playground.html)
- [HERO WIZARD SAVE YOUR GIRLFRIEND](https://studyplayings.web.app/hero-wizard-save-your-girlfriend.html)
- [FASHION VALKYRIES SAGA OF STYLE](https://studyplayings.pages.dev/fashion-valkyries-saga-of-style.html)
- [BASKET SWAP](https://learnquester.github.io/basket-swap.html)
- [BIMKA DRIVE SMASH CARS INTO SPLINTERS](https://learnquester.github.io/bimka-drive-smash-cars-into-splinters.html)
- [MARBLE BLAST](https://studyplayings.web.app/marble-blast.html)
- [PALM ISLAND SOLITAIRE](https://studyplayings.web.app/palm-island-solitaire.html)
- [STICKMAN ARCHERO FIGHT STICK SHADOW FIGHT WAR](https://learnquester.github.io/stickman-archero-fight-stick-shadow-fight-war.html)
- [CLEANING SIMULATOR](https://learnquester.github.io/cleaning-simulator.html)
- [HOT COLD WINTER STYLE](https://learnquester.github.io/hot-cold-winter-style.html)
- [LUCKY BRAINROT BLOCKS ONLINE](https://learnquester.github.io/lucky-brainrot-blocks-online.html)
- [STICKMAN FIGHT PRO](https://learnquester.github.io/stickman-fight-pro.html)
- [LITTLE CANDY BAKERY](https://learnquester.github.io/little-candy-bakery.html)
- [CATEGORY BRAIN260](https://studyplayings.pages.dev/category-brain260.html)
- [NINJA BAMBOO ASSASSIN](https://studyplayings.web.app/ninja-bamboo-assassin.html)
- [LIGHT ACADEMIA FASHION](https://studyplayings.web.app/light-academia-fashion.html)
- [SNIPER 3D ZOMBIE](https://studyplayings.web.app/sniper-3d-zombie.html)
- [ZOMBIES WEAPON MERGE 4](https://studyplayings.pages.dev/zombies-weapon-merge-4.html)
- [OBBY VS ZOMBIES](https://learnquester.github.io/obby-vs-zombies.html)
- [DIAMOND MOSAIC](https://learnquester.github.io/diamond-mosaic.html)
- [ERASE THE EXTRA ELEMENT](https://learnquester.github.io/erase-the-extra-element.html)
- [2248 BLAST](https://learnquester.github.io/2248-blast.html)
- [SEA MATCH](https://studyplayings.pages.dev/sea-match.html)
- [TOILET ROLL](https://learnquester.github.io/toilet-roll.html)
- [STICK TACTICS DESTRUCTION](https://learnquester.github.io/stick-tactics-destruction.html)
- [CATEGORY RPG80](https://studyplayings.pages.dev/category-rpg80.html)
- [FAST LAP](https://studyplayings.web.app/fast-lap.html)
- [BLOXDHOP IO](https://studyplayings.web.app/bloxdhop-io.html)
- [ARROW COUNT MASTER](https://learnquester.github.io/arrow-count-master.html)
- [IDLE LANDMARK BUILDER](https://learnquester.github.io/idle-landmark-builder.html)
- [OBBY POGO PARKOUR](https://learnquester.github.io/obby-pogo-parkour.html)
- [MATCH 3 DREAM ROOM](https://studyplayings.web.app/match-3-dream-room.html)
- [FRUIT CAFE MATCH 3](https://learnquester.github.io/fruit-cafe-match-3.html)
- [MAKE AMERICA GREAT AGAIN](https://learnquester.github.io/make-america-great-again.html)
- [FOREST MATCH 4](https://studyplayings.web.app/forest-match-4.html)
- [PEOPLE PLAYGROUND RAGDOLL BATTLE](https://learnquester.github.io/people-playground-ragdoll-battle.html)
- [IDLE FARM](https://studyplayings.web.app/idle-farm.html)
- [CATEGORY AGILITY 2](https://studyplayings.pages.dev/category-agility-2.html)
- [KNEE CASE SIMULATOR](https://studyplayings.pages.dev/knee-case-simulator.html)
- [REAL IMPOSSIBLE SKY TRACKS CAR DRIVING](https://studyplayings.web.app/real-impossible-sky-tracks-car-driving.html)
- [FRUIT BLOCK TETRA PUZZLE](https://studyplayings.pages.dev/fruit-block-tetra-puzzle.html)
- [SHIP CONTROL 3D](https://studyplayings.web.app/ship-control-3d.html)
- [POP PARTY SUIKA WATERMELON](https://studyplayings.web.app/pop-party-suika-watermelon.html)
- [FIDGET TOYS POP IT](https://learnquester.github.io/fidget-toys-pop-it.html)
- [BRAINROT CLICKER](https://learnquester.github.io/brainrot-clicker.html)
- [SOLITAIRE EMPEROR SECRETS OF FATE](https://learnquester.github.io/solitaire-emperor-secrets-of-fate.html)
- [UNBLOCK BALL SLIDE PUZZLE](https://learnquester.github.io/unblock-ball-slide-puzzle.html)
- [URBAN ASSAULT FORCE](https://learnquester.github.io/urban-assault-force.html)
- [UNPUZZLE MASTER](https://learnquester.github.io/unpuzzle-master.html)
- [CATEGORY PUZZLE 3](https://studyplayings.pages.dev/category-puzzle-3.html)
- [COZY GARDEN IDLE](https://learnquester.github.io/cozy-garden-idle.html)
- [EXTREME CAR DRIVING SIMULATOR](https://studyplayings.pages.dev/extreme-car-driving-simulator.html)
- [HIDDEN OBJECT CLUES AND MYSTERIES](https://studyplayings.web.app/hidden-object-clues-and-mysteries.html)
- [CARD MASTER](https://learnquester.github.io/card-master.html)
- [ASMR PET TREATMENT](https://learnquester.github.io/asmr-pet-treatment.html)
- [HALLOWEEN CHALLENGE](https://learnquester.github.io/halloween-challenge.html)
- [DOODLE DINO RUN](https://studyplayings.web.app/doodle-dino-run.html)
- [LUDO STAR](https://studyplayings.web.app/ludo-star.html)
- [DINOSAUR CARDS](https://studyplayings.web.app/dinosaur-cards.html)
- [WORMS](https://learnquester.github.io/worms.html)
- [GEOMETRY ARROW 2](https://learnquester.github.io/geometry-arrow-2.html)
- [NUTS STACK SORT NUTS BOLTS](https://studyplayings.web.app/nuts-stack-sort-nuts-bolts.html)
- [WORD RIVERS](https://studyplayings.web.app/word-rivers.html)
- [CUBE CONNECT](https://studyplayings.web.app/cube-connect.html)
- [CATEGORY CAR](https://thelearnquester.web.app/category-car.html)
- [REAL FREEKICK 3D](https://learnquester.github.io/real-freekick-3d.html)
- [CATEGORY BATTLE524](https://thelearnquester.web.app/category-battle524.html)
- [PYRAMIDZ](https://studyplayings.web.app/pyramidz.html)
- [MERMAIDCORE AESTHETICS](https://learnquester.github.io/mermaidcore-aesthetics.html)
- [SLIME RUSH](https://studyplayings.pages.dev/slime-rush.html)
- [GUN RUSH](https://learnquester.github.io/gun-rush.html)
- [OBBY 1 PET EVERY SECONDS](https://learnquester.github.io/obby-1-pet-every-seconds.html)
- [SEAT JAM 3D](https://studyplayings.pages.dev/seat-jam-3d.html)
- [RED ESCAPE](https://studyplayings.web.app/red-escape.html)
- [CHICKEN SHOOTER IO](https://studyplayings.web.app/chicken-shooter-io.html)
- [TERMS](https://studyplayings.pages.dev/terms.html)
- [BELL MADNESS](https://studyplayings.web.app/bell-madness.html)
- [CATEGORY COLOR197](https://thelearnquester.web.app/category-color197.html)
- [THE STONE MINER](https://learnquester.github.io/the-stone-miner.html)
- [BYEPASSHUB](https://studyplayings.pages.dev/byepasshub.html)
- [CATEGORY ARENA255](https://studyplayings.pages.dev/category-arena255.html)
- [CAKE LINK MASTER](https://studyplayings.web.app/cake-link-master.html)
- [MINICRAFT WINTERBLOCK](https://studyplayings.pages.dev/minicraft-winterblock.html)
- [AUTUMN GLAM GALA](https://studyplayings.pages.dev/autumn-glam-gala.html)
- [TOY CARS 3D RACING](https://learnquester.github.io/toy-cars-3d-racing.html)
- [CIRCLE RUN ENDLESS](https://studyplayings.pages.dev/circle-run-endless.html)
- [SKY BLOCK BOUNCE](https://learnquester.github.io/sky-block-bounce.html)
- [ONLINE PORTAL](https://studyplayings.pages.dev/)
- [EUROPE AT WAR](https://studyplayings.web.app/europe-at-war.html)
- [CATEGORY BATTLE523](https://learnquester.github.io/category-battle523.html)
- [CATEGORY CAN T STOP PLAYING212](https://learnquester.github.io/category-can-t-stop-playing212.html)
- [CATEGORY CASUAL](https://thelearnquester.web.app/category-casual.html)
- [CHRISTMAS FIND THE DIFFERENCES](https://studyplayings.pages.dev/christmas-find-the-differences.html)
- [KITTEN NEVER DIES](https://studyplayings.web.app/kitten-never-dies.html)
- [CATEGORY PUZZLE 2](https://studyplayings.pages.dev/category-puzzle-2.html)
- [COMBINE PICKAXES](https://studyplayings.pages.dev/combine-pickaxes.html)
- [BRAIN PUZZLE TRICKY CHOICES](https://learnquester.github.io/brain-puzzle-tricky-choices.html)
- [WORM ESCAPE](https://studyplayings.web.app/worm-escape.html)
- [CATEGORY 3D1 371](https://thelearnquester.web.app/category-3d1-371.html)
- [FARM VS ZOMBIES](https://studyplayings.web.app/farm-vs-zombies.html)
- [PRIVACY](https://thelearnquester.web.app/privacy.html)
- [DART HERO](https://studyplayings.web.app/dart-hero.html)
- [MAIDO](https://studyplayings.pages.dev/maido.html)
- [ITALIAN BRAINROT CHALLENGE](https://learnquester.github.io/italian-brainrot-challenge.html)
- [WINTER WOLF](https://studyplayings.web.app/winter-wolf.html)
- [SHAPE TRANSFORMING SHIFTING RUN](https://learnquester.github.io/shape-transforming-shifting-run.html)
- [INDEX6](https://studyplayings.pages.dev/index6.html)
- [HERO TRANSFORM RUN](https://learnquester.github.io/hero-transform-run.html)
- [FORMULA RACING GAMES CAR GAME](https://studyplayings.web.app/formula-racing-games-car-game.html)
- [CATEGORY BATTLE](https://studyplayings.pages.dev/category-battle.html)
