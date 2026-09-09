{
  description = "RustDesk Remote Desktop — open source TeamViewer / Citrix alternative";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    nixpkgs-darwin-legacy.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    flake-utils.url = "github:numtide/flake-utils";
    hbb_common = {
      url = "github:rustdesk/hbb_common/55395c6fcbcb8dd4bc8d4e7ab4d7d7c8d1789b43";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, nixpkgs-darwin-legacy, flake-utils, hbb_common, ... }:
    {
      overlays.default = final: prev: {
        rustdesk = (self.packages.${final.system} or self.packages.${final.stdenv.system}).default or null;
      };
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs =
          if system == "x86_64-darwin"
          then import nixpkgs-darwin-legacy { inherit system; config.allowUnfree = true; }
          else import nixpkgs { inherit system; config.allowUnfree = true; };

        lib = pkgs.lib;

        # Static libopus — nixpkgs only ships a shared build; magnum-opus's
        # build.rs links `static=opus` via the vcpkg shim below.
        libopusStatic = pkgs.libopus.overrideAttrs (old: {
          mesonFlags = (old.mesonFlags or [ ]) ++ [
            "-Ddefault_library=static"
          ];
          postInstall = (old.postInstall or "") + ''
            # meson puts the .a in $out/lib when default_library=static
          '';
        });

        # libaom without vmaf — the static .a references vmaf symbols that
        # the scrap build.rs doesn't link; disabling vmaf avoids the
        # unresolved symbols at link time.
        libaomNoVmaf = pkgs.libaom.override { enableVmaf = false; };

        # Sciter runtime for macOS — nixpkgs only ships libsciter for Linux.
        # The same sciter-sdk GitHub commit used by nixpkgs has bin.osx/libsciter.dylib.
        libsciter-darwin = pkgs.stdenv.mkDerivation {
          pname = "libsciter-darwin";
          version = "4.4.8.23-bis";
          src = pkgs.fetchurl {
            url = "https://github.com/c-smile/sciter-sdk/raw/524a90ef7eab16575df9496f7e4c374bbd5fb1fe/bin.osx/libsciter.dylib";
            hash = "sha256-qNhEQ5tDy3DwKoYQxzX/3nikG6oeVRBYuGkURjAkCg0=";
          };
          dontUnpack = true;
          installPhase = ''
            runHook preInstall
            install -m755 -D $src $out/lib/libsciter.dylib
            runHook postInstall
          '';
          meta = {
            homepage = "https://sciter.com";
            description = "Embeddable HTML/CSS/JavaScript engine for modern UI development";
            platforms = lib.platforms.darwin;
            sourceProvenance = with lib.sourceTypes; [ lib.sourceTypes.binaryNativeCode ];
            license = lib.licenses.unfree;
          };
        };

        # Assemble a vcpkg-like installed tree from nixpkgs static libs so
        # libs/scrap/build.rs and magnum-opus/build.rs (which require
        # VCPKG_ROOT on macOS x86_64 and only fall back to homebrew on
        # aarch64) can find libyuv/libvpx/aom/opus.
        # Layout: $out/installed/<target>/lib/*.a, .../include/*
        vcpkgRoot = pkgs.runCommand "rustdesk-vcpkg-root" { } (
          let
            target =
              if pkgs.stdenv.hostPlatform.isAarch64 then "arm64-osx"
              else if pkgs.stdenv.hostPlatform.isx86_64 then "x64-osx"
              else throw "unsupported darwin arch for vcpkg shim";
            mkLinks = name: pkg: ''
              ln -s ${lib.getLib pkg}/lib/lib${name}.a $out/installed/${target}/lib/lib${name}.a
              for h in ${lib.getDev pkg}/include/*; do
                ln -s "$h" "$out/installed/${target}/include/$(basename "$h")"
              done
            '';
          in
          ''
            mkdir -p $out/installed/${target}/lib $out/installed/${target}/include
            ${mkLinks "yuv" pkgs.libyuv}
            ${mkLinks "vpx" pkgs.libvpx}
            ${mkLinks "aom" libaomNoVmaf.static}
            ${mkLinks "opus" libopusStatic}
          ''
        );

        rustdesk = pkgs.rustPlatform.buildRustPackage {
          pname = "rustdesk";
          version = "1.5.0";
          src = ./.;
          cargoHash = "sha256-GTFAfVwcWtEz5ViPWRbInTvZbOM8jS/aD9cAxVhKsfw=";

          patches = [ ./nix/make-build-reproducible.patch ];

          desktopItems = lib.optionals pkgs.stdenv.isLinux [
            (pkgs.makeDesktopItem {
              name = "rustdesk";
              desktopName = "RustDesk";
              genericName = "Remote Desktop";
              comment = "Remote Desktop";
              exec = "rustdesk %u";
              icon = "rustdesk";
              terminal = false;
              type = "Application";
              startupNotify = true;
              categories = [ "Network" "RemoteAccess" "GTK" ];
              keywords = [ "internet" "linux" "dart" "rust" "remote-control" "p2p" "teamviewer" "rust-lang" "rdp" "remote-desktop" "vnc" ];
              startupWMClass = "rustdesk";
              actions.new-window = {
                name = "Open a New Window";
                exec = "rustdesk %u";
              };
            })
            (pkgs.makeDesktopItem {
              name = "rustdesk-link";
              desktopName = "RustDesk";
              noDisplay = true;
              mimeTypes = [ "x-scheme-handler/rustdesk" ];
              tryExec = "rustdesk";
              exec = "rustdesk %u";
              icon = "rustdesk";
              terminal = false;
              type = "Application";
              startupNotify = false;
              startupWMClass = "rustdesk";
            })
          ];

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.perl
            pkgs.makeWrapper
            pkgs.rustPlatform.bindgenHook
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.copyDesktopItems
            pkgs.wrapGAppsHook3
          ];

          buildFeatures = lib.optionals pkgs.stdenv.isLinux [ "linux-pkg-config" ];

          doCheck = false;

          buildInputs = [
            pkgs.bzip2
            pkgs.libgit2
            pkgs.libsodium
            pkgs.libvpx
            pkgs.libyuv
            pkgs.libopus
            pkgs.libaom
            pkgs.openssl
            pkgs.zlib
            pkgs.zstd
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.atk
            pkgs.cairo
            pkgs.dbus
            pkgs.gdk-pixbuf
            pkgs.glib
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gstreamer
            pkgs.gtk3
            pkgs.libpulseaudio
            pkgs.libxtst
            pkgs.libxkbcommon
            pkgs.pam
            pkgs.pango
            pkgs.alsa-lib
            pkgs.xdotool
            pkgs.libsciter
            pkgs.libayatana-appindicator
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.apple-sdk
            vcpkgRoot
            libsciter-darwin
          ];

          # Replace the hbb_common submodule with the flake input pin.
          # The pin in flake.lock must be updated when the submodule gitlink
          # changes; the flake input is the source of truth for reproducibility.
          prePatch = ''
            rm -rf libs/hbb_common
            cp -r ${hbb_common} libs/hbb_common
            chmod -R u+w libs/hbb_common
          '';

          postPatch = ''
            sed -e '1i #include <cstdint>' -i $cargoDepsCopy/*/webm-1.1.0/src/sys/libwebm/mkvparser/mkvparser.cc
            sed -e '1i #include <cstdint>' -i $cargoDepsCopy/*/webm-sys-1.0.4/libwebm/mkvparser/mkvparser.cc
          '';

          postInstall = ''
            mkdir -p $out/lib/rustdesk $out/share
            mv $out/bin/rustdesk $out/lib/rustdesk
            makeWrapper $out/lib/rustdesk/rustdesk $out/bin/rustdesk \
              --chdir "$out/share"
          '' + lib.optionalString pkgs.stdenv.isLinux ''
            mkdir -p $out/share/src
            ln -s ${pkgs.libsciter}/lib/libsciter-gtk.so $out/lib/rustdesk
            cp -a $src/src/ui $out/share/src
            install -Dm0644 $src/res/logo.svg $out/share/icons/hicolor/scalable/apps/rustdesk.svg
          '' + lib.optionalString pkgs.stdenv.isDarwin ''
            mkdir -p $out/share/src
            ln -s ${libsciter-darwin}/lib/libsciter.dylib $out/lib/rustdesk/libsciter.dylib
            cp -a $src/src/ui $out/share/src
          '';

          postFixup = lib.optionalString pkgs.stdenv.isLinux ''
            patchelf --add-rpath "${pkgs.libayatana-appindicator}/lib" "$out/lib/rustdesk/rustdesk"
          '';

          env = {
            SODIUM_USE_PKG_CONFIG = true;
            ZSTD_SYS_USE_PKG_CONFIG = true;
          } // lib.optionalAttrs pkgs.stdenv.isDarwin {
            VCPKG_ROOT = "${vcpkgRoot}";
            VCPKG_INSTALLED_ROOT = "${vcpkgRoot}/installed";
          };

          meta = {
            description = "Virtual / remote desktop infrastructure for everyone! Open source TeamViewer / Citrix alternative";
            homepage = "https://github.com/rustdesk/rustdesk";
            license = lib.licenses.agpl3Only;
            mainProgram = "rustdesk";
          };
        };
      in
      {
        packages = {
          rustdesk = rustdesk;
          default = rustdesk;
          source = rustdesk;
        } // lib.optionalAttrs pkgs.stdenv.isLinux {
          nixpkgs = pkgs.rustdesk;
        };

        apps = {
          rustdesk = {
            type = "app";
            program = "${rustdesk}/bin/rustdesk";
          };
          default = {
            type = "app";
            program = "${rustdesk}/bin/rustdesk";
          };
        } // lib.optionalAttrs pkgs.stdenv.isLinux {
          nixpkgs = {
            type = "app";
            program = "${pkgs.rustdesk}/bin/rustdesk";
          };
        };

        checks = {
          build = rustdesk;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.perl
            pkgs.makeWrapper
            pkgs.rustPlatform.bindgenHook
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.copyDesktopItems
            pkgs.wrapGAppsHook3
          ];

          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.bzip2
            pkgs.libgit2
            pkgs.libsodium
            pkgs.libvpx
            pkgs.libyuv
            pkgs.libopus
            pkgs.libaom
            pkgs.openssl
            pkgs.zlib
            pkgs.zstd
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.atk
            pkgs.cairo
            pkgs.dbus
            pkgs.gdk-pixbuf
            pkgs.glib
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gstreamer
            pkgs.gtk3
            pkgs.libpulseaudio
            pkgs.libxtst
            pkgs.libxkbcommon
            pkgs.pam
            pkgs.pango
            pkgs.alsa-lib
            pkgs.xdotool
            pkgs.libsciter
            pkgs.libayatana-appindicator
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.apple-sdk
            vcpkgRoot
            libsciter-darwin
          ];

          RUSTDESK_BUILD_FEATURES = lib.optionalString pkgs.stdenv.isLinux "linux-pkg-config";

          shellHook = ''
            # Set up hbb_common so cargo build works from the source tree.
            if [ ! -e libs/hbb_common/Cargo.toml ]; then
              rm -rf libs/hbb_common
              cp -r ${hbb_common} libs/hbb_common
              chmod -R u+w libs/hbb_common
            fi
          '' + lib.optionalString pkgs.stdenv.isDarwin ''
            export VCPKG_ROOT="${vcpkgRoot}"
            export VCPKG_INSTALLED_ROOT="${vcpkgRoot}/installed"
          '';
        };
      }
    );
}
