# Tefway Remote — portable Linux AppImage (Podman builder)

Produces a single-file, per-user AppImage:

```
build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
build/tefway/master/TefwayRemoteMaster-Linux-x86_64.AppImage
```

The builder is self-contained in `docker/tefway-linux/`:

| File | Purpose |
| --- | --- |
| `Dockerfile` | Debian bookworm builder image with pinned Rust, Flutter 3.44.8, the pinned `flutter_rust_bridge_codegen` binary, `appimagetool` and `linuxdeploy`. |
| `build.sh` | Host driver (rootless Podman, one build at a time) and in-container build/package steps. |

## Why a direct AppDir

The previous AppImage path built a `.deb` first and unpacked it with
`appimage-builder`. That flow hid two problems:

1. The Rust core is installed by Flutter's CMake at
   `<bundle>/lib/librustdesk.so`. The packager checked for
   `<bundle>/librustdesk.so`, so a correct Flutter bundle was rejected while a
   hand-assembled one could ship with the library in a place the Dart loader
   (`Platform.resolvedExecutable` + `/lib/librustdesk.so`) does not search.
2. Repacking a distro package made the AppImage depend on the package's
   `/usr/share/<pkg>` layout and on `appimage-builder`'s focal `apt` bundle.

This builder assembles the AppDir directly from the Flutter release bundle. The
layout is the one the runtime expects:

```
AppDir/
  AppRun
  tefway-remote-client.desktop
  tefway-remote-client.png
  usr/lib/tefway-remote-client/
    TefwayRemoteClient
    data/flutter_assets/
    lib/librustdesk.so
    lib/libflutter_linux_gtk.so
    lib/<bundled runtime libs>
  usr/share/icons/hicolor/{16,32,48,64,128,256,512}x.../apps/tefway-remote-client.png
```

`TefwayRemoteClient` keeps Flutter's `$ORIGIN/lib` RUNPATH, so both
`lib/librustdesk.so` and `libflutter_linux_gtk.so` resolve next to the
executable. The Flutter binary is renamed only after the bundle is validated.

## Requirements

* Fedora Linux x86_64 with rootless Podman.
* Roughly 15 GB free for the image, Cargo target and both AppDirs.
* Network access for the image build (Debian packages, Flutter SDK, pinned
  AppImage tools) and for Cargo/pub during the first build.

The builder does **not**:

* run `podman --privileged`;
* install a systemd unit on the host;
* change global SELinux policy (the container is started with
  `--security-opt label=disable` instead of relabelling the checkout);
* change Wayland compositor settings;
* build both profiles concurrently.

## Build

```bash
cd /home/lucas/Desenvolvimento/tefway-remote-linux

# 1. Client first.
docker/tefway-linux/build.sh --profile client

# 2. After the Client is validated, build the Master separately.
docker/tefway-linux/build.sh --profile master
```

Useful options:

```bash
--skip-image       reuse an existing builder image
--force-bridge     regenerate the gitignored flutter_rust_bridge files
--features LIST    extra cargo features (comma separated)
--image NAME:TAG   use another builder image tag
```

The host locks `build/tefway/.build.lock`, so a second invocation fails instead
of starting a competing Cargo/Flutter build.

## Resource limits

The defaults are deliberately conservative for the low-memory host:

| Setting | Value |
| --- | --- |
| `CARGO_BUILD_JOBS` | `2` |
| `CMAKE_BUILD_PARALLEL_LEVEL` | `2` |
| `MAKEFLAGS` | `-j2` |
| Rust codegen | `linux-pkg-config` feature; codec libraries come from Debian packages instead of building vcpkg ports inside the container |

If the build is killed for memory, diagnose the host consumers first (the
builder does not raise parallelism automatically). The builder never increases
these limits or removes optimizations on its own.

## Runtime libraries

`librustdesk.so` and the Flutter engine stay in `lib/`. The builder also copies
the codec libraries it links (`libopus`, `libvpx`, `libaom`, `libyuv`) and the
tray/indicator libraries, plus any dependency that the build host's `ldconfig`
does not provide. The GTK/GLib/Pango/GStreamer desktop stack is intentionally
taken from the host: bundling a second GTK without its schemas, pixbuf loaders
and immodules is the usual cause of "starts but every image fails" AppImages.
Fedora Workstation ships that stack.

## Verification performed by the builder

After packaging, `build.sh` extracts the finished AppImage and fails unless:

* `AppRun` and the root `.desktop` file exist;
* `usr/lib/<pkg>/<exe>` is executable;
* `usr/lib/<pkg>/lib/librustdesk.so` exists;
* `usr/lib/<pkg>/lib/libflutter_linux_gtk.so` exists;
* `usr/lib/<pkg>/data/flutter_assets` contains an `AssetManifest.*`;
* `ldd librustdesk.so` reports no `not found`;
* the executable RUNPATH still contains `$ORIGIN/lib`.

Each artifact gets a sibling
`<artifact>.manifest.json` with profile, product, commit, builder image,
Rust/Flutter versions, cargo features, SHA-256 and size.

## Runtime smoke test (Fedora, normal user)

```bash
chmod +x build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage --get-id
./build/tefway/client/TefwayRemoteClient-Linux-x86_64.AppImage
```

The last command opens the Client window. Confirm:

1. the title and badge read `Tefway Remote Client`;
2. it shows its ID and has no outbound "connect to peer" affordance;
3. the log shows `remote.tefway.com.br` as the rendezvous server;
4. a connection offered by the Master triggers the local approval dialog.

`--get-id` starts the core in CLI mode only; it is a quick evidence check that
the Rust library loaded, not a substitute for the GUI flow.

## Known limits

* The AppImage is attended-use only: it installs no service, so login-screen
  sessions and unattended access do not apply to the Client product by design.
* Wayland capture remains subject to the host's PipeWire portal and the upstream
  RustDesk Wayland limitations; X11 remains the reliable path.
* The AppImage relies on the host GTK3 desktop stack (see above).
* Technician exclusivity (`TEFWAY-REMOTE-AUTH-01`) is still not implemented.
