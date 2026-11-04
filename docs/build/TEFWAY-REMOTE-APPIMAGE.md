# Tefway Remote — AppImage x86_64

Produces `TefwayRemoteMaster-x86_64.AppImage` and
`TefwayRemoteClient-x86_64.AppImage`.

AppImage packaging reuses the project's existing `appimage-builder` recipe
(`appimage/AppImageBuilder-x86_64.yml`) as a template:
`res/tefway/pkg/AppImageBuilder.yml.in` is the same recipe with the package
paths, app id, icon and executable rendered per profile.

## Environment

Build on Debian/Ubuntu x86_64 — `appimage-builder` pulls Ubuntu focal packages
into the AppDir, and the project's recipe is written for that.

| Tool | Version |
| --- | --- |
| Rust | 1.75 (stable) |
| Flutter | 3.24.5 (stable) |
| appimage-builder | 1.1.x (`pip install appimage-builder`) |
| dpkg-dev, bsdtar, gzip | distro packages |
| Python | 3.10+ |

```bash
sudo apt install -y build-essential dpkg-dev libarchive-tools gzip python3-pip
pip3 install appimage-builder
```

Everything else (Rust, Flutter 3.24.5, vcpkg) is the Debian document's setup.

## Build

The recipe is generated from a .deb, so the flow is: build → deb → AppImage.

```bash
python3 res/tefway/package.py --profile master --format appimage --build
python3 res/tefway/package.py --profile client --format appimage --build
```

`package.py` builds the bundle, packages the .deb, renders
`build/tefway/<profile>/appimage/AppImageBuilder.yml` and runs
`appimage-builder --recipe`, then renames the result.

## Output

```
build/tefway/master/TefwayRemoteMaster-x86_64.AppImage
build/tefway/client/TefwayRemoteClient-x86_64.AppImage
```

## Icon verification

```bash
./TefwayRemoteClient-x86_64.AppImage --appimage-extract
ls squashfs-root/usr/share/icons/hicolor/*/apps/tefway-remote-client.png
```
Then run the AppImage and check the dock/taskbar icon and the window icon.

## Smoke test

```bash
chmod +x TefwayRemoteClient-x86_64.AppImage
./TefwayRemoteClient-x86_64.AppImage
```

1. The Client shows its ID and offers no way to connect out.
2. The Master AppImage connects to that ID; the Client asks for approval.
3. Reconnect asks again; decline grants nothing.
4. The Master refuses incoming logins.
5. Network settings show `remote.tefway.com.br` and the Tefway key.

## Known issues and limits

- FUSE: on a host without FUSE, run with `--appimage-extract-and-run`.
- AppImage has no systemd unit, so the service path is not installed. Sessions
  from the login screen and elevated capture therefore do not work from the
  AppImage; that is why the .deb/.rpm packages exist. The AppImage is for
  attended use.
- The AppImage does not install desktop entries; the recipe drops
  `usr/share/applications` on purpose. Integration (menu entry, icon) is the
  desktop environment's AppImage launcher's job.
- 256/512 px icons are upscaled from the 128 px source.
- Technician exclusivity is not implemented; see `TEFWAY-REMOTE-AUTH-01`.
