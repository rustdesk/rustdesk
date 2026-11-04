# Tefway Remote — Windows x64 build

Produces `TefwayRemoteMaster.exe` and `TefwayRemoteClient.exe` from the same
source tree. The product is compiled in through `TEFWAY_PROFILE`; it cannot be
changed by an option, a preference or a command-line argument afterwards.

## Environment

Pinned by the project's own CI (`.github/workflows/flutter-build.yml`), and what
the build is validated against:

| Tool | Version |
| --- | --- |
| Rust | 1.75 (stable, MSVC toolchain) |
| Flutter | 3.24.5 (stable) |
| Visual Studio | 2022, "Desktop development with C++" |
| vcpkg | `VCPKG_ROOT` set, baseline `9e593bb18ea69cc5095e012465dcd675a822ed0d` from `vcpkg.json` |
| LLVM/libclang | required by `ffigen` |
| Python | 3.11 |
| CMake | 3.x, from the VS installation |

Dependencies from `vcpkg.json` must be installed for `x64-windows-static-md`:

```powershell
vcpkg install --triplet x64-windows-static-md
```

## One-time: generate the bridge

`flutter/lib/generated_bridge.dart` and `src/bridge_generated*.rs` are git
ignored and produced by code generation. A fresh clone cannot build until they
exist. CI generates them with the pinned `flutter_rust_bridge_codegen`; do the
same locally (`flutter_rust_bridge: "1.80.1"` is the version in
`flutter/pubspec.yaml`), then apply the same ffigen workaround `build.py`
applies (`ffi_bindgen_function_refactor`, `build.py:377`).

## Build

From the repository root, in "x64 Native Tools Command Prompt for VS 2022":

```powershell
# Master (technician station: starts sessions, never accepts them)
python res\tefway\package.py --profile master --format windows --build --with-hwcodec

# Client (end-user station: shows its ID, accepts each session by hand)
python res\tefway\package.py --profile client --format windows --build --with-hwcodec
```

`--build` runs, in order: the virtual-display dylib, `cargo build --locked
--features flutter,hwcodec --lib --release`, `flutter build windows --release`,
and copies `dylib_virtual_display.dll` into the bundle. To package an existing
bundle instead, drop `--build`.

## Output

```
build\tefway\master\TefwayRemoteMaster\TefwayRemoteMaster.exe
build\tefway\client\TefwayRemoteClient\TefwayRemoteClient.exe
```

Each directory is the complete portable package: the exe plus `data\`, `*.dll`
and the Rust/Flutter runtime files Flutter links. **A single self-contained exe
is not produced and is not promised**: the virtual-display DLL and the Flutter
runtime assets sit beside the executable. Ship the directory (zip it), not the
exe alone. `build.py --portable` (the upstream single-file packer) works on the
stock `rustdesk.exe` and was not adopted here because it needs a codegen step on
every packaging host.

## Icon verification

1. `flutter\windows\runner\resources\app_icon.ico` and `res\icon.ico` are
   multi-size ICOs (16/24/32/48/64/128/256) generated from `assets/favicon.ico`
   by `python res\tefway\gen_icons.py`.
2. Explorer: the exe shows the Tefway mark at large, medium and small views.
3. Launch it: window icon (title bar), taskbar button, Alt+Tab preview.
4. Right-click the exe → Properties → Details: Product name is
   `Tefway Remote Master` / `Tefway Remote Client`, copyright still credits
   Purslane Tech Pte. Ltd., and the description still says it is built from
   RustDesk.
5. Re-check after signing; a bad signature or a re-linked resource can drop the
   icon.

## Smoke test

1. Start the Master and the Client on two machines (or two Windows users).
2. Client: the ID is shown and copyable. There is no "connect to a peer" input
   at all, because the Client binary is incoming-only in Rust.
3. Master: connect to the Client's ID. The Client must show the accept dialog.
   Accept → session opens. Decline → the Master sees a refusal and no session.
4. Disconnect, connect again: the Client must ask **again**. No stored approval,
   no reusable authorization.
5. Master: ask another machine to connect to the Master's ID. The login must be
   refused (`Tefway Remote Master does not accept incoming connections`) and no
   capture or control may start.
6. Settings → Network: rendezvous and relay are `remote.tefway.com.br`, the key
   is the Tefway key, and the fields cannot be edited to anything else.

## Known issues and limits

- The 256 and 512 pixel icon frames are upscaled from the 128 px source ICO;
  the small sizes are native. A square vector mark from the owner would render
  them sharper; drop it in and re-run `gen_icons.py`.
- Code signing and the MSI installer are out of scope for this gate. Unsigned
  builds raise SmartScreen warnings; the installer path (`res/msi`) still carries
  upstream product metadata and needs a Tefway pass before distribution.
- The Windows service is installed by the app's own elevation flow, not by this
  script; the Master's service refuses every login, and the Client's service
  requires the local click.
- Technician exclusivity is **not** implemented yet: any peer that knows the ID
  can request a session, and the end user decides. See
  `TEFWAY-REMOTE-AUTH-01` in `docs/handoff/TEFWAY-REMOTE-RELEASE-HANDOFF.md`.

## Success criteria

Both exe files build from a clean tree with the versions above, both show the
Tefway icon and product name, the Client cannot start a session by any path,
the Master cannot accept one, and every new Client session requires a click.
