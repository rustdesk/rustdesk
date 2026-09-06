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
- [EMERLAND SOLITAIRE](https://iskillquest.pages.dev/emerland-solitaire.html)
- [MERGE BLOCKS 2048](https://quizverses.github.io/merge-blocks-2048.html)
- [NOOB JAILBREAK 2](https://studyplaying.github.io/noob-jailbreak-2.html)
- [CATEGORY BATTLE](https://studyplaying.github.io/category-battle.html)
- [URBAN ASSAULT FORCE](https://quizverses-9d2f2.web.app/urban-assault-force.html)
- [ULTIMATE DESTRUCTION SIMULATOR](https://studyplaying.github.io/ultimate-destruction-simulator.html)
- [PARKOUR WORLD 2](https://quizverses.github.io/parkour-world-2.html)
- [HEX PLANET IDLE](https://quizverses.github.io/hex-planet-idle.html)
- [WORM OUT BRAIN TEASER GAMES](https://quizverses.github.io/worm-out-brain-teaser-games.html)
- [BATTLE ZONE 2D](https://studyquests.pages.dev/battle-zone-2d.html)
- [BUBBLE AROUND](https://quizverses-9d2f2.web.app/bubble-around.html)
- [LEGEND OF FIREBALL](https://quizverses.github.io/legend-of-fireball.html)
- [FINGER HEART MONSTER REFILL](https://quizverses.github.io/finger-heart-monster-refill.html)
- [CATEGORY MANAGEMENT](https://quizverses.pages.dev/category-management.html)
- [HOTEL MANAGER](https://studyplaying.github.io/hotel-manager.html)
- [BRAINROTS LAVA SURVIVE ONLINE](https://studyquests.pages.dev/brainrots-lava-survive-online.html)
- [CATEGORY SIDE SCROLLING184](https://studyplayings.web.app/category-side-scrolling184.html)
- [CATEGORY PIXEL313](https://quizverses.pages.dev/category-pixel313.html)
- [KIDS COLORING](https://quizverses.github.io/kids-coloring.html)
- [MR DRIFTER CAR CHASE SIMULATOR](https://studyplayings.pages.dev/mr-drifter-car-chase-simulator.html)
- [LINK FLOW](https://learnquester.github.io/link-flow.html)
- [PRINCESS RUN 3D](https://studyplayings.pages.dev/princess-run-3d.html)
- [HAWAII MATCH 6](https://quizverses-9d2f2.web.app/hawaii-match-6.html)
- [CATEGORY CASUAL971](https://studyplayings.web.app/category-casual971.html)
- [NATURAL DISASTER SURVIVAL OBBY](https://learnquester.github.io/natural-disaster-survival-obby.html)
- [GRAND CLASH ARENA](https://studyplaying.github.io/grand-clash-arena.html)
- [INDEX13](https://studyplaying.github.io/index13.html)
- [ELLIE AND FRIENDS GET READY FOR FIRST DATE](https://learnquester.github.io/ellie-and-friends-get-ready-for-first-date.html)
- [CATEGORY MOUSE1 697](https://quizverses.github.io/category-mouse1-697.html)
- [CATEGORY CAN T STOP PLAYING215](https://studyplaying.github.io/category-can-t-stop-playing215.html)
- [POLICE CHASE DRIFTER](https://studyplaying.github.io/police-chase-drifter.html)
- [HEAD JUMP](https://quizverses.github.io/head-jump.html)
- [2 CARS RUN](https://quizverses.github.io/2-cars-run.html)
- [CUTE CRAFT LAB](https://studyplaying.github.io/cute-craft-lab.html)
- [LODE RETRO ADVENTURE](https://quizverses.github.io/lode-retro-adventure.html)
- [CONSOLE IDLE](https://quizverses-9d2f2.web.app/console-idle.html)
- [SUPERMARKET SORT GROCERY GAME](https://quizverses.github.io/supermarket-sort-grocery-game.html)
- [PEOPLE PLAYGROUND RAGDOLL ARENA](https://studyplayings.pages.dev/people-playground-ragdoll-arena.html)
- [CATEGORY AGILITY](https://quizverses.github.io/category-agility.html)
- [CATEGORY COLOR](https://studyplaying.github.io/category-color.html)
- [CATEGORY PLATFORM](https://quizverses.pages.dev/category-platform.html)
- [CATEGORY ARENA255](https://quizverses.github.io/category-arena255.html)
- [HAPPY FARM THE CROP](https://studyplaying.github.io/happy-farm-the-crop.html)
- [MATH CROSSWORD PUZZLE GENIUS EDITION](https://quizverses.github.io/math-crossword-puzzle-genius-edition.html)
- [CUT N FILL](https://studyquests.github.io/cut-n-fill.html)
- [BLOCK PIXELS](https://studyquests.pages.dev/block-pixels.html)
- [SISYPHUS SIMULATOR](https://studyplayings.pages.dev/sisyphus-simulator.html)
- [REAL GT RACING SIMULATOR](https://studyquests.github.io/real-gt-racing-simulator.html)
- [CATEGORY SECURLY BYPASS](https://quizverses.pages.dev/category-securly-bypass.html)
- [CATEGORY CASUAL 11](https://studyplaying.github.io/category-casual-11.html)
- [CATEGORY BLOCK94](https://studyplaying.github.io/category-block94.html)
- [CATEGORY AGILITY 3](https://studyplaying.github.io/category-agility-3.html)
- [BUILD A GO KART](https://quizverses.github.io/build-a-go-kart.html)
- [CATEGORY UNBLOCK](https://studyplaying.github.io/category-unblock.html)
- [MINI GAMES CASUAL COLLECTION](https://studyquests.pages.dev/mini-games-casual-collection.html)
- [CATEGORY RPG](https://quizverses.pages.dev/category-rpg.html)
- [CATEGORY ARENA255](https://learnquester.github.io/category-arena255.html)
- [EXCAVATOR SIMULATOR 3D](https://studyplaying.github.io/excavator-simulator-3d.html)
- [CATEGORY MAKEUP](https://quizverses.pages.dev/category-makeup.html)
- [BANG BANG MAHJONG](https://studyquests.github.io/bang-bang-mahjong.html)
- [ARMY FIGHT 3D](https://studyquests.github.io/army-fight-3d.html)
- [LADDER MASTER COLOR RUN](https://learnquester.github.io/ladder-master-color-run.html)
- [NUMBER RUSH](https://studyplayings.pages.dev/number-rush.html)
- [INDEX2](https://learnquester.github.io/index2.html)
- [BRAIN FIND CAN YOU FIND IT](https://studyplayings.pages.dev/brain-find-can-you-find-it.html)
- [CATEGORY PUZZLE 2](https://quizverses.pages.dev/category-puzzle-2.html)
- [MAGIC TOWERS SOLITAIRE](https://studyquests.github.io/magic-towers-solitaire.html)
- [ARCHERY MASTER](https://studyquests.pages.dev/archery-master.html)
- [LABUBU MERGE CLICKER](https://quizverses-9d2f2.web.app/labubu-merge-clicker.html)
- [SORTSTORE](https://studyquests.github.io/sortstore.html)
- [HEXA GO](https://quizverses.github.io/hexa-go.html)
- [TANK STARS](https://studyplayings.pages.dev/tank-stars.html)
- [CATEGORY BATTLE GAMES](https://studyplaying.github.io/category-battle-games.html)
- [PRINCESSES AT HORROR SCHOOL](https://quizverses.github.io/princesses-at-horror-school.html)
- [MATCH 3D PUZZLE SAGA](https://studyplaying.github.io/match-3d-puzzle-saga.html)
- [DELICIOUS EMILYS NEW BEGINNING VALENTINES EDITION](https://studyquests.github.io/delicious-emilys-new-beginning-valentines-edition.html)
- [KICK LUCKY BOXES ONLINE](https://quizverses.github.io/kick-lucky-boxes-online.html)
- [INDEX19](https://learnquester.github.io/index19.html)
- [INDEX17](https://studyplaying.github.io/index17.html)
- [FIRE TRUCK DRIVING SIMULATOR](https://learnquester.github.io/fire-truck-driving-simulator.html)
- [GOD OF LIGHT](https://quizverses-9d2f2.web.app/god-of-light.html)
- [CATEGORY PLATFORM260](https://studyquesthub.web.app/category-platform260.html)
- [FASHION VALKYRIES SAGA OF STYLE](https://studyplaying.github.io/fashion-valkyries-saga-of-style.html)
- [BRAWL BROS SQUAD](https://studyplayings.pages.dev/brawl-bros-squad.html)
- [CARNAGE BATTLE ARENA](https://studyplayings.pages.dev/carnage-battle-arena.html)
- [CATEGORY OBBY56](https://quizverses.pages.dev/category-obby56.html)
- [CATEGORY ANIMAL216](https://quizverses-9d2f2.web.app/category-animal216.html)
- [MERMAIDCORE MAKEUP](https://quizverses.github.io/mermaidcore-makeup.html)
- [FEED ME MONSTERS IDLE BATTLE](https://studyplayings.pages.dev/feed-me-monsters-idle-battle.html)
- [PET ME MAZE](https://quizverses.github.io/pet-me-maze.html)
- [CATEGORY STRATEGY](https://studyquesthub.web.app/category-strategy.html)
- [CATEGORY QUIZ40](https://studyplayings.pages.dev/category-quiz40.html)
- [CATEGORY BATTLE CATEGORY](https://studyplaying.github.io/category-battle-category.html)
- [REACH 2048](https://studyquests.github.io/reach-2048.html)
- [CATEGORY BUILDING182](https://studyplayings.pages.dev/category-building182.html)
- [GUN RUSH](https://learnquester.github.io/gun-rush.html)
- [CATEGORY BIKE63](https://studyplayings.pages.dev/category-bike63.html)
- [RACE IT CAR RACING](https://studyquests.github.io/race-it-car-racing.html)
- [PERFECT CAKE MAKER](https://studyquests.github.io/perfect-cake-maker.html)
- [AMERICAN BLOCK SNIPER ONLINE](https://studyquests.github.io/american-block-sniper-online.html)
- [CATEGORY FPS 2](https://studyplaying.github.io/category-fps-2.html)
- [BEST CLASSIC FREECELL SOLITAIRE](https://studyquests.github.io/best-classic-freecell-solitaire.html)
- [BLOX FRUITS](https://studyquests.pages.dev/blox-fruits.html)
- [CRAFT DRILL](https://studyquests.github.io/craft-drill.html)
- [MERGE SESAME](https://studyplayings.pages.dev/merge-sesame.html)
- [SMASH THE CAR TO PIECES](https://studyplayings.pages.dev/smash-the-car-to-pieces.html)
- [CATEGORY JIGSAW](https://quizverses-9d2f2.web.app/category-jigsaw.html)
- [CATEGORY CONTROLLER](https://studyplayings.pages.dev/category-controller.html)
- [PUZZLE BLOCKS](https://studyplaying.github.io/puzzle-blocks.html)
- [ARMY DEFENCE DINO SHOOT](https://quizverses.github.io/army-defence-dino-shoot.html)
- [INDEX16](https://studyplayings.pages.dev/index16.html)
- [FRUIT JAM MERGE PUZZLE GAME](https://learnquester.github.io/fruit-jam-merge-puzzle-game.html)
- [CATEGORY BATTLE524](https://studyplayings.pages.dev/category-battle524.html)
- [BLOCK UP](https://studyquests.github.io/block-up.html)
- [POGO MASTERS](https://quizverses-9d2f2.web.app/pogo-masters.html)
- [MATCH FIGHTER](https://studyquests.pages.dev/match-fighter.html)
- [CATEGORY DOG18](https://quizverses-9d2f2.web.app/category-dog18.html)
- [THE COUNTERFEIT BANK](https://studyquests.github.io/the-counterfeit-bank.html)
- [CATEGORY TOP DOWN251](https://studyquesthub.web.app/category-top-down251.html)
- [ANIMATION COLORING ALPHABET LORE](https://quizverses.github.io/animation-coloring-alphabet-lore.html)
- [CATEGORY SPORTS](https://studyquesthub.web.app/category-sports.html)
- [CAR DEALER IDLE](https://studyquests.github.io/car-dealer-idle.html)
- [TRI PEAKS EMERLAND SOLITAIRE](https://studyquests.github.io/tri-peaks-emerland-solitaire.html)
- [OFFICE SOLITAIRE](https://studyquests.github.io/office-solitaire.html)
- [CATEGORY CRAFTING45](https://studyplaying.github.io/category-crafting45.html)
- [PLANE CHASE](https://quizverses.github.io/plane-chase.html)
- [CATEGORY MANAGEMENT209](https://quizverses.pages.dev/category-management209.html)
- [FROG BYTE](https://studyquests.github.io/frog-byte.html)
- [TAYLOR DRESS STUDIO PREPPY WILD WEST GLAM](https://studyquests.github.io/taylor-dress-studio-preppy-wild-west-glam.html)
- [BATTLE ARENA](https://quizverses.pages.dev/battle-arena.html)
