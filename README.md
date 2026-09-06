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
- [PERFECT TIDY](https://studyquesthub.web.app/perfect-tidy.html)
- [ANTISTRESS RELAXATION BOX](https://theskillquest.pages.dev/antistress-relaxation-box.html)
- [OLE BUNNY](https://studyquests.pages.dev/ole-bunny.html)
- [GOMU GOMAN](https://quizverses-9d2f2.web.app/gomu-goman.html)
- [CATEGORY IDLE448](https://quizverses.github.io/category-idle448.html)
- [CATEGORY QUIZ](https://quizverses.github.io/category-quiz.html)
- [KEY QUEST](https://quizverses.github.io/key-quest.html)
- [CATEGORY EDUCATIONAL](https://studyquests.pages.dev/category-educational.html)
- [CATEGORY CASUAL 10](https://studyplaying.github.io/category-casual-10.html)
- [CATEGORY SHOOTER](https://quizverses-9d2f2.web.app/category-shooter.html)
- [CATEGORY STICKMAN175](https://quizverses-9d2f2.web.app/category-stickman175.html)
- [PUZZLE PLAY](https://studyplayings.pages.dev/puzzle-play.html)
- [ROBYBOX SPACE STATION WAREHOUSE](https://quizverses.github.io/robybox-space-station-warehouse.html)
- [SPIDER EVOLUTION](https://studyplayings.pages.dev/spider-evolution.html)
- [CATEGORY SPACE57](https://quizverses.github.io/category-space57.html)
- [TAP BEAD](https://studyquesthub.web.app/tap-bead.html)
- [GOD OF LIGHT](https://quizverses-9d2f2.web.app/god-of-light.html)
- [DOT BY DOT](https://studyplayings.pages.dev/dot-by-dot.html)
- [TUNG SAHUR COLORING](https://quizverses.github.io/tung-sahur-coloring.html)
- [BLOCK EATING SIMULATOR](https://studyquests.pages.dev/block-eating-simulator.html)
- [CATEGORY SOLITAIRE27](https://quizverses.github.io/category-solitaire27.html)
- [CATEGORY STRATEGY](https://quizverses.github.io/category-strategy.html)
- [CATEGORY SIMULATION 2](https://quizverses.pages.dev/category-simulation-2.html)
- [DOWNHILL CAR RIDE CRASH TEST](https://learnquester.github.io/downhill-car-ride-crash-test.html)
- [SMASH THE BOTTLE](https://quizverses.github.io/smash-the-bottle.html)
- [CATEGORY BUSINESS135](https://studyquests.pages.dev/category-business135.html)
- [CATEGORY RUNNING107](https://quizverses.github.io/category-running107.html)
- [ASMR BEAUTY HOMELESS](https://quizverses.github.io/asmr-beauty-homeless.html)
- [BACKROOMS SKIBIDI TERRORS](https://quizverses.github.io/backrooms-skibidi-terrors.html)
- [CATEGORY STICKMAN](https://quizverses.github.io/category-stickman.html)
- [CATEGORY ZOMBIE](https://quizverses-9d2f2.web.app/category-zombie.html)
- [HEADLEG DASH PARKOUR](https://quizverses.github.io/headleg-dash-parkour.html)
- [CATEGORY BIKE](https://quizverses.pages.dev/category-bike.html)
- [MY PURRFECT CAT HOTEL](https://studyplaying.github.io/my-purrfect-cat-hotel.html)
- [JELLY BELLY MAKE THE ELEPHANT](https://quizverses.github.io/jelly-belly-make-the-elephant.html)
- [SOLITAIRE STORY TRIPEAKS 6](https://quizverses.github.io/solitaire-story-tripeaks-6.html)
- [FLAG PUZZLE JAM COLLECT FLAGS](https://quizverses-9d2f2.web.app/flag-puzzle-jam-collect-flags.html)
- [CLICKER HERO](https://studyplaying.github.io/clicker-hero.html)
- [BOLTS AND NUTS SORTING](https://studyplaying.github.io/bolts-and-nuts-sorting.html)
- [ZENITH RUSH](https://learnquester.github.io/zenith-rush.html)
- [CATEGORY 2D1 060](https://quizverses.pages.dev/category-2d1-060.html)
- [CANDY SMASH](https://quizverses-9d2f2.web.app/candy-smash.html)
- [CATEGORY SPORTS](https://quizverses-9d2f2.web.app/category-sports.html)
- [MAKEUP FRUITS](https://studyplayings.pages.dev/makeup-fruits.html)
- [CATEGORY CASUAL 5](https://quizverses.github.io/category-casual-5.html)
- [CATEGORY GROW99](https://quizverses.pages.dev/category-grow99.html)
- [CATEGORY CANNON22](https://quizverses.github.io/category-cannon22.html)
- [CATEGORY FLASH](https://quizverses.pages.dev/category-flash.html)
- [PLANE CRASH RAGDOLL SIMULATOR](https://studyplaying.github.io/plane-crash-ragdoll-simulator.html)
- [MATH RUNNER](https://quizverses.github.io/math-runner.html)
- [SOCCER EURO CUP 2025](https://quizverses.github.io/soccer-euro-cup-2025.html)
- [CATEGORY DESTROY256](https://thelearnquester.web.app/category-destroy256.html)
- [CATEGORY FPS175](https://quizverses-9d2f2.web.app/category-fps175.html)
- [FISH OUT OF WATER](https://studyplayings.pages.dev/fish-out-of-water.html)
- [CATEGORY PUZZLE 2](https://quizverses.github.io/category-puzzle-2.html)
- [CHROMA TREK](https://studyquests.pages.dev/chroma-trek.html)
- [CATEGORY FLASH 3](https://quizverses.github.io/category-flash-3.html)
- [CANDY COLOR SORT PUZZLE](https://quizverses.github.io/candy-color-sort-puzzle.html)
- [JIGSAW FANTASY](https://learnquester.github.io/jigsaw-fantasy.html)
- [CATEGORY BUBBLE SHOOTER](https://quizverses.pages.dev/category-bubble-shooter.html)
- [COSMIC AVIATOR](https://studyplaying.github.io/cosmic-aviator.html)
- [2 PLAYER GAMES KIDS KITCHEN](https://studyplayings.pages.dev/2-player-games-kids-kitchen.html)
- [BANK ROBBERY ESCAPE](https://quizverses.github.io/bank-robbery-escape.html)
- [COUNTRYSIDE DRIVING QUEST](https://studyquests.pages.dev/countryside-driving-quest.html)
- [CATEGORY SURVIVAL366](https://studyplaying.github.io/category-survival366.html)
- [SERIOUS HEAD 2](https://studyquests.pages.dev/serious-head-2.html)
- [PIN DETECTIVE](https://studyplaying.github.io/pin-detective.html)
- [CATEGORY DOG18](https://quizverses.github.io/category-dog18.html)
- [POPSORTICA](https://learnquester.github.io/popsortica.html)
- [ROYAL REBELLION PUNK MAGIC](https://quizverses-9d2f2.web.app/royal-rebellion-punk-magic.html)
- [SMART DOTS RELOADED](https://studyplaying.github.io/smart-dots-reloaded.html)
- [1945 AIR FORCE AIRPLANE](https://studyquests.pages.dev/1945-air-force-airplane.html)
- [QUIZMANIA TRIVIA GAME](https://studyplaying.github.io/quizmania-trivia-game.html)
- [CATEGORY PUZZLE 2](https://studyquests.pages.dev/category-puzzle-2.html)
- [COLOR SCREW RESCUE PUZZLE](https://studyquests.pages.dev/color-screw-rescue-puzzle.html)
- [UGC MATH RACE](https://quizverses-9d2f2.web.app/ugc-math-race.html)
- [ONLINE CAR DESTRUCTION SIMULATOR 3D](https://quizverses.github.io/online-car-destruction-simulator-3d.html)
- [DOG MERGE MANIA](https://studyplaying.github.io/dog-merge-mania.html)
- [ONET MAHJONG CONNECT](https://quizverses.github.io/onet-mahjong-connect.html)
- [LAZY DOG](https://quizverses.github.io/lazy-dog.html)
- [MONSTER SCHOOL 2](https://quizverses-9d2f2.web.app/monster-school-2.html)
- [FLIGHT PILOT AIRPLANE GAMES 24](https://studyplayings.pages.dev/flight-pilot-airplane-games-24.html)
- [SKYDOM REFORGED](https://studyplayings.pages.dev/skydom-reforged.html)
- [HERO WIZARD SAVE YOUR GIRLFRIEND](https://studyplaying.github.io/hero-wizard-save-your-girlfriend.html)
- [TREASURE HUNT PUZZLE](https://studyplayings.pages.dev/treasure-hunt-puzzle.html)
- [LABUBU MERGE](https://learnquester.github.io/labubu-merge.html)
- [ISLAND BATTLE 3D](https://studyquesthub.web.app/island-battle-3d.html)
- [SLAP AND RUN](https://quizverses-9d2f2.web.app/slap-and-run.html)
- [METAXIS](https://studyplayings.pages.dev/metaxis.html)
- [NUMBER TRICKY PUZZLES](https://studyplaying.github.io/number-tricky-puzzles.html)
- [KNOCKOUT DUDES](https://quizverses.github.io/knockout-dudes.html)
- [MOJO MATCH 3D](https://studyquests.pages.dev/mojo-match-3d.html)
- [PERFECT ASMR CLEANING](https://studyplaying.github.io/perfect-asmr-cleaning.html)
- [PING PONG AIR](https://studyplaying.github.io/ping-pong-air.html)
- [CATEGORY WATER39](https://quizverses.github.io/category-water39.html)
- [AXE THROW](https://quizverses.github.io/axe-throw.html)
- [CATEGORY LOVE12](https://quizverses.github.io/category-love12.html)
- [MERGE FRUIT](https://studyplaying.github.io/merge-fruit.html)
- [CATEGORY WAR](https://quizverses-9d2f2.web.app/category-war.html)
- [INDEX6](https://quizverses.pages.dev/index6.html)
- [BLOCK MASTER SUPER PUZZLE](https://studyplayings.pages.dev/block-master-super-puzzle.html)
- [SUMMER CONNECT](https://quizverses.github.io/summer-connect.html)
- [DRIVE RACE CRASH](https://quizverses.github.io/drive-race-crash.html)
- [MY CAKE SHOP BAKE SERVE](https://quizverses.github.io/my-cake-shop-bake-serve.html)
- [HAPPY MONSTERS 2](https://studyplayings.pages.dev/happy-monsters-2.html)
- [CATEGORY BUILDING182](https://quizverses.github.io/category-building182.html)
- [OBBY CARDS THE LEGEND HUNT](https://quizverses-9d2f2.web.app/obby-cards-the-legend-hunt.html)
- [JOIN CLASH COLOR BUTTON](https://quizverses.pages.dev/join-clash-color-button.html)
- [CATEGORY SIDE SCROLLING184](https://thelearnquester.web.app/category-side-scrolling184.html)
- [WARPING BAT](https://quizverses.pages.dev/warping-bat.html)
- [CAR CARE REPAIR DUDU MECHANIC](https://quizverses.pages.dev/car-care-repair-dudu-mechanic.html)
- [MATCH 3 DREAM ROOM](https://studyquests.pages.dev/match-3-dream-room.html)
- [REAL CAR PARKING AND STUNT](https://studyplaying.github.io/real-car-parking-and-stunt.html)
- [UNSCREW WOOD PUZZLE](https://studyplaying.github.io/unscrew-wood-puzzle.html)
- [INDEX4](https://thelearnquester.web.app/index4.html)
- [ROYAL KITCHEN THE LOST KING](https://quizverses.github.io/royal-kitchen-the-lost-king.html)
- [TOCA AVATAR MY HOSPITAL](https://studyquesthub.web.app/toca-avatar-my-hospital.html)
- [MAKE TWO](https://studyplayings.pages.dev/make-two.html)
- [MAZE CRAZE](https://learnquester.github.io/maze-craze.html)
- [MERMAIDS SPOT THE DIFFERENCES](https://studyquests.pages.dev/mermaids-spot-the-differences.html)
- [MAGIC BOTTLES](https://quizverses-9d2f2.web.app/magic-bottles.html)
- [TEARDOWN DESTRUCTION SANDBOX](https://studyplayings.pages.dev/teardown-destruction-sandbox.html)
- [CATEGORY MAHJONG CONNECT](https://studyquests.github.io/category-mahjong-connect.html)
- [APOCALYPSE SHELTER](https://quizverses.pages.dev/apocalypse-shelter.html)
- [CATEGORY BRAIN](https://quizverses.pages.dev/category-brain.html)
- [FLOAT FOR BRAINROTS](https://studyplayings.pages.dev/float-for-brainrots.html)
- [CRASH THE ROBOT](https://studyquests.github.io/crash-the-robot.html)
- [CATEGORY COLLECT565](https://quizverses.pages.dev/category-collect565.html)
- [CATEGORY BRAIN261](https://quizverses.pages.dev/category-brain261.html)
- [CATEGORY SIMULATION 3](https://studyquests.github.io/category-simulation-3.html)
