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
- [CATEGORY FREE FASHION GAMES](https://themindzone.pages.dev/category-free-fashion-games.html)
- [MIND GAMES MATH CROSSWORDS](https://learnquester.github.io/mind-games-math-crosswords.html)
- [SAVE THE CROP](https://studyplayings.pages.dev/save-the-crop.html)
- [CONNECT IMAGE](https://learnquester.github.io/connect-image.html)
- [GT CARS CITY RACING](https://studyplayings.pages.dev/gt-cars-city-racing.html)
- [OFFLINE FPS ROYALE](https://studyplayings.pages.dev/offline-fps-royale.html)
- [IDLE FACTORY DOMINATION](https://studyplayings.web.app/idle-factory-domination.html)
- [NUMBER RUSH](https://studyplayings.pages.dev/number-rush.html)
- [INDEX3](https://learnquester.github.io/index3.html)
- [CATEGORY MOUSE](https://studyplayings.pages.dev/category-mouse.html)
- [MAHJONG GARDEN](https://studyplayings.pages.dev/mahjong-garden.html)
- [WOODLAND SLIDE](https://learnquester.github.io/woodland-slide.html)
- [STACK TOWER PRO](https://studyplayings.pages.dev/stack-tower-pro.html)
- [BOMBAMAN 3D](https://learnquester.github.io/bombaman-3d.html)
- [FAST LAP](https://studyplayings.web.app/fast-lap.html)
- [JUMP MAN](https://studyplayings.pages.dev/jump-man.html)
- [SLIPPERY DRIFT RACING](https://studyplayings.web.app/slippery-drift-racing.html)
- [BLANKETS](https://studyplayings.web.app/blankets.html)
- [SPELLMIND](https://studyplayings.pages.dev/spellmind.html)
- [INDEX6](https://learnquester.github.io/index6.html)
- [WORLD SOLITAIRE TRIPEAKS ](https://learnquester.github.io/world-solitaire-tripeaks-.html)
- [TILES MATCHING](https://learnquester.github.io/tiles-matching.html)
- [SHADOWMAN RUNNER](https://studyplayings.pages.dev/shadowman-runner.html)
- [1010 ELIXIR ALCHEMY](https://studyplayings.web.app/1010-elixir-alchemy.html)
- [MERGE COMBO](https://learnquester.github.io/merge-combo.html)
- [YUMMY TALES 3](https://learnquester.github.io/yummy-tales-3.html)
- [HOT COLD WINTER STYLE](https://learnquester.github.io/hot-cold-winter-style.html)
- [GALAXY CARNAGE](https://learnquester.github.io/galaxy-carnage.html)
- [MINE FPS SHOOTER NOOB ARENA](https://studyplayings.web.app/mine-fps-shooter-noob-arena.html)
- [CATEGORY PROXY](https://learnquester.github.io/category-proxy.html)
- [PUZZLE PLAY](https://studyplayings.pages.dev/puzzle-play.html)
- [MR DRIFTER CAR CHASE SIMULATOR](https://studyplayings.pages.dev/mr-drifter-car-chase-simulator.html)
- [STICKMAN MINERS WARS](https://learnquester.github.io/stickman-miners-wars.html)
- [TREASURE CHAMPION CHEST CAPTURE](https://studyplayings.pages.dev/treasure-champion-chest-capture.html)
- [RAGDOLL SHOW THROW BREAK AND DESTROY](https://studyplayings.web.app/ragdoll-show-throw-break-and-destroy.html)
- [CATEGORY WAR137](https://studyplayings.web.app/category-war137.html)
- [FASHION WORLD SIMULATOR](https://studyplayings.web.app/fashion-world-simulator.html)
- [LABUBA MERGE](https://learnquester.github.io/labuba-merge.html)
- [CATEGORY DIFFICULT81](https://studyplayings.pages.dev/category-difficult81.html)
- [POTION MERGE WITCH](https://learnquester.github.io/potion-merge-witch.html)
- [CATEGORY SIMULATION 2](https://learnquester.github.io/category-simulation-2.html)
- [GRANNY GTA VEGAS](https://learnquester.github.io/granny-gta-vegas.html)
- [SHIP CONTROL 3D](https://studyplayings.pages.dev/ship-control-3d.html)
- [EMOJI GUESS](https://studyplayings.pages.dev/emoji-guess.html)
- [ARROW COUNT MASTER](https://studyplayings.pages.dev/arrow-count-master.html)
- [FEET DOCTOR URGENCY CARE](https://studyplayings.web.app/feet-doctor-urgency-care.html)
- [CHILDCARE MASTER ONLINE](https://studyplayings.web.app/childcare-master-online.html)
- [FEET DOCTOR URGENCY CARE](https://learnquester.github.io/feet-doctor-urgency-care.html)
- [CATEGORY GROW99](https://learnquester.github.io/category-grow99.html)
- [BLOCKS AND THATS IT](https://learnquester.github.io/blocks-and-thats-it.html)
- [QUEENS ROYAL SUDOKU PUZZLE](https://studyplayings.web.app/queens-royal-sudoku-puzzle.html)
- [TURBO STARS](https://learnquester.github.io/turbo-stars.html)
- [CATEGORY ADVENTURE](https://learnquester.github.io/category-adventure.html)
- [SCREW SORT 3D SCREW PUZZLE](https://learnquester.github.io/screw-sort-3d-screw-puzzle.html)
- [CATEGORY SHOOTER](https://learnquester.github.io/category-shooter.html)
- [ECHOLOCATION SHOOTER](https://studyplayings.pages.dev/echolocation-shooter.html)
- [PET FALL](https://learnquester.github.io/pet-fall.html)
- [EQ TEST PUZZLE](https://studyplayings.pages.dev/eq-test-puzzle.html)
- [CATEGORY CRASH32](https://studyplayings.pages.dev/category-crash32.html)
- [DRUNK MAN 3D](https://studyplayings.pages.dev/drunk-man-3d.html)
- [CAKE LINK MASTER](https://learnquester.github.io/cake-link-master.html)
- [EXCAVATOR SIMULATOR 3D](https://learnquester.github.io/excavator-simulator-3d.html)
- [FRUIT PARTY](https://studyplayings.pages.dev/fruit-party.html)
- [RIDE SHOOTER](https://studyplayings.web.app/ride-shooter.html)
- [TAP TAP BUILDER](https://learnquester.github.io/tap-tap-builder.html)
- [QUACKVENTURE](https://studyplayings.web.app/quackventure.html)
- [BUNNIES SORT](https://learnquester.github.io/bunnies-sort.html)
- [CATEGORY INTERSTELLARPROXY](https://learnquester.github.io/category-interstellarproxy.html)
- [CATEGORY SANDBOX](https://studyplayings.web.app/category-sandbox.html)
- [CATEGORY BATTLE ROYALE25](https://studyplayings.pages.dev/category-battle-royale25.html)
- [THE BEST WARRIOR](https://learnquester.github.io/the-best-warrior.html)
- [CATEGORY BRAIN](https://learnquester.github.io/category-brain.html)
- [BLIND BOAT SHOOTING MASTER](https://learnquester.github.io/blind-boat-shooting-master.html)
- [CATEGORY BOARDGAMES](https://learnquester.github.io/category-boardgames.html)
- [HIDDEN OBJECTS STORY](https://studyplayings.pages.dev/hidden-objects-story.html)
- [FROG KNIGHT](https://learnquester.github.io/frog-knight.html)
- [SOLAR SMASH](https://studyplayings.pages.dev/solar-smash.html)
- [LITTLE LILY HALLOWEEN PREP](https://studyplayings.web.app/little-lily-halloween-prep.html)
- [YOUR DREAM ROOM](https://studyplayings.pages.dev/your-dream-room.html)
- [LUDO STAR](https://studyplayings.web.app/ludo-star.html)
- [BACKWOODS](https://studyplayings.pages.dev/backwoods.html)
- [SMASH DEFENSE](https://learnquester.github.io/smash-defense.html)
- [CAR PARKING SIMULATOR](https://studyplayings.pages.dev/car-parking-simulator.html)
- [BILLIARDS 3D RUSSIAN PYRAMID](https://studyplayings.web.app/billiards-3d-russian-pyramid.html)
- [BEAM DRIVE CAR CRASH TEST SIMULATOR](https://studyplayings.pages.dev/beam-drive-car-crash-test-simulator.html)
- [TRICKY ARROW 2](https://studyplayings.pages.dev/tricky-arrow-2.html)
- [CATEGORY MERGE224](https://studyplayings.pages.dev/category-merge224.html)
- [PICK BRAINROT 3D BATTLE](https://learnquester.github.io/pick-brainrot-3d-battle.html)
- [ONLINE PORTAL](https://studyplayings.pages.dev/)
- [CATEGORY RACING DRIVING 2](https://studyplayings.web.app/category-racing-driving-2.html)
- [CATEGORY CASUAL 13](https://studyplaying.github.io/category-casual-13.html)
- [HEADLEG DASH PARKOUR](https://studyquests.pages.dev/headleg-dash-parkour.html)
- [OFFICE SPIDER SOLITAIRE](https://studyquesthub.web.app/office-spider-solitaire.html)
- [OFF ROAD OVERDRIVE](https://studyplayings.pages.dev/off-road-overdrive.html)
- [INDEX35](https://studyplaying.github.io/index35.html)
- [ELLIE AND FRIENDS GET READY FOR FIRST DATE](https://learnquester.github.io/ellie-and-friends-get-ready-for-first-date.html)
- [MAHJONG CUTE TILES](https://studyplaying.github.io/mahjong-cute-tiles.html)
- [WORLD WAR 2 SHOOTER](https://studyplaying.github.io/world-war-2-shooter.html)
- [JENNYS MATH PUZZLE](https://studyquesthub.web.app/jennys-math-puzzle.html)
- [ROBYBOX SPACE STATION WAREHOUSE](https://studyplaying.github.io/robybox-space-station-warehouse.html)
- [IDLE MONEY FACTORY](https://learnquester.github.io/idle-money-factory.html)
- [CYBER ARROW](https://learnquester.github.io/cyber-arrow.html)
- [BITGOBLINS RPG SIMULATOR](https://studyplaying.github.io/bitgoblins-rpg-simulator.html)
- [DAILY CHESS PUZZLE](https://studyplayings.web.app/daily-chess-puzzle.html)
- [COZY GARDEN IDLE](https://studyplayings.web.app/cozy-garden-idle.html)
- [CATEGORY AGILITY 2](https://studyquests.pages.dev/category-agility-2.html)
- [CATEGORY PREMIUM PERKS74](https://studyplaying.github.io/category-premium-perks74.html)
- [CATEGORY GROW GAMES](https://learnquester.github.io/category-grow-games.html)
- [COLOR SCREW RESCUE PUZZLE](https://studyplaying.github.io/color-screw-rescue-puzzle.html)
- [CATEGORY BYEPASSHUB](https://studyplayings.pages.dev/category-byepasshub.html)
- [KISS O NECK](https://studyplaying.github.io/kiss-o-neck.html)
- [CLOCKWORK](https://studyplaying.github.io/clockwork.html)
- [BLOOM SORT 2 BEE PUZZLE](https://studyplayings.web.app/bloom-sort-2-bee-puzzle.html)
- [CATEGORY AVOID](https://learnquester.github.io/category-avoid.html)
- [TERMS](https://learnquester.github.io/terms.html)
- [XMAS HEXA SORT](https://studyplaying.github.io/xmas-hexa-sort.html)
- [SUPER SOCCER NOGGINS](https://studyplaying.github.io/super-soccer-noggins.html)
- [CARGO PATH PUZZLE](https://studyplaying.github.io/cargo-path-puzzle.html)
- [KINGS AND QUEENS MATCH 3](https://studyquests.pages.dev/kings-and-queens-match-3.html)
- [WAVE ROAD 3D](https://studyquests.pages.dev/wave-road-3d.html)
- [UNLOCK THE BOLTS](https://learnquester.github.io/unlock-the-bolts.html)
- [CATEGORY EDUCATIONAL](https://studyplayings.pages.dev/category-educational.html)
- [CRAFTSMAN 3D GANGSTER](https://studyplaying.github.io/craftsman-3d-gangster.html)
- [INDIAN SUV OFFROAD SIMULATOR](https://studyquests.pages.dev/indian-suv-offroad-simulator.html)
- [RUNNING IN FOAM](https://studyplaying.github.io/running-in-foam.html)
- [NAIL QUEEN](https://studyplayings.pages.dev/nail-queen.html)
- [GOBATTLEIO](https://studyplaying.github.io/gobattleio.html)
- [HEAD RUNNER DASH](https://studyquesthub.web.app/head-runner-dash.html)
- [HOSPITAL GAME HAPPY CLINIC](https://studyplaying.github.io/hospital-game-happy-clinic.html)
- [NONOGRAM DAILY](https://studyquesthub.web.app/nonogram-daily.html)
