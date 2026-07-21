#!/usr/bin/env python3

import os
import glob
import pathlib
import platform
import zipfile
import urllib.request
import shutil
import hashlib
import subprocess
import argparse
import sys
from pathlib import Path

windows = platform.platform().startswith('Windows')
osx = platform.platform().startswith(
    'Darwin') or platform.platform().startswith("macOS")
hbb_name = 'rustdesk' + ('.exe' if windows else '')
exe_path = 'target/release/' + hbb_name
if windows:
    win_arch = 'arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x64'
    flutter_build_dir = f'build/windows/{win_arch}/runner/Release/'
elif osx:
    flutter_build_dir = 'build/macos/Build/Products/Release/'
else:
    flutter_build_dir = 'build/linux/x64/release/bundle/'
flutter_build_dir_2 = f'flutter/{flutter_build_dir}'
skip_cargo = False


def get_deb_arch() -> str:
    custom_arch = os.environ.get("DEB_ARCH")
    if custom_arch is None:
        return "amd64"
    return custom_arch

def get_deb_extra_depends() -> str:
    custom_arch = os.environ.get("DEB_ARCH")
    if custom_arch == "armhf": # for arm32v7 libsciter-gtk.so
        return ", libatomic1"
    return ""

def system2(cmd):
    exit_code = os.system(cmd)
    if exit_code != 0:
        sys.stderr.write(f"Error occurred when executing: `{cmd}`. Exiting.\n")
        sys.exit(-1)


def get_version():
    with open("Cargo.toml", encoding="utf-8") as fh:
        for line in fh:
            if line.startswith("version"):
                return line.replace("version", "").replace("=", "").replace('"', '').strip()
    return ''


def parse_rc_features(feature):
    available_features = {}
    apply_features = {}
    if not feature:
        feature = []

    def platform_check(platforms):
        if windows:
            return 'windows' in platforms
        elif osx:
            return 'osx' in platforms
        else:
            return 'linux' in platforms

    def get_all_features():
        features = []
        for (feat, feat_info) in available_features.items():
            if platform_check(feat_info['platform']):
                features.append(feat)
        return features

    if isinstance(feature, str) and feature.upper() == 'ALL':
        return get_all_features()
    elif isinstance(feature, list):
        if windows:
            # download third party is deprecated, we use github ci instead.
            # feature.append('PrivacyMode')
            pass
        for feat in feature:
            if isinstance(feat, str) and feat.upper() == 'ALL':
                return get_all_features()
            if feat in available_features:
                if platform_check(available_features[feat]['platform']):
                    apply_features[feat] = available_features[feat]
            else:
                print(f'Unrecognized feature {feat}')
        return apply_features
    else:
        raise Exception(f'Unsupported features param {feature}')


def make_parser():
    parser = argparse.ArgumentParser(description='Build script.')
    parser.add_argument(
        '-f',
        '--feature',
        dest='feature',
        metavar='N',
        type=str,
        nargs='+',
        default='',
        help='Integrate features, windows only.'
             'Available: [Not used for now]. Special value is "ALL" and empty "". Default is empty.')
    parser.add_argument('--flutter', action='store_true',
                        help='Build flutter package', default=False)
    parser.add_argument(
        '--hwcodec',
        action='store_true',
        help='Enable feature hwcodec' + (
            '' if windows or osx else ', need libva-dev.')
    )
    parser.add_argument(
        '--vram',
        action='store_true',
        help='Enable feature vram, only available on windows now.'
    )
    parser.add_argument(
        '--portable',
        action='store_true',
        help='Build windows portable'
    )
    parser.add_argument(
        '--unix-file-copy-paste',
        action='store_true',
        help='Build with unix file copy paste feature'
    )
    parser.add_argument(
        '--drm',
        action='store_true',
        help='Linux only: build the DRM/KMS capture backend (bundles libdrmtap.so, '
             'dlopen-ed in-process by the root service). Off by default.'
    )
    parser.add_argument(
        '--skip-cargo',
        action='store_true',
        help='Skip cargo build process, only flutter version + Linux supported currently'
    )
    if windows:
        parser.add_argument(
            '--skip-portable-pack',
            action='store_true',
            help='Skip packing, only flutter version + Windows supported'
        )
    parser.add_argument(
        "--package",
        type=str
    )
    if osx:
        parser.add_argument(
            '--screencapturekit',
            action='store_true',
            help='Enable feature screencapturekit'
        )
    return parser


# Generate build script for docker
#
# it assumes all build dependencies are installed in environments
# Note: do not use it in bare metal, or may break build environments
def generate_build_script_for_docker():
    with open("/tmp/build.sh", "w") as f:
        f.write('''
            #!/bin/bash
            # environment
            export CPATH="$(clang -v 2>&1 | grep "Selected GCC installation: " | cut -d' ' -f4-)/include"
            # flutter
            pushd /opt
            wget https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.0.5-stable.tar.xz
            tar -xvf flutter_linux_3.0.5-stable.tar.xz
            export PATH=`pwd`/flutter/bin:$PATH
            popd
            # flutter_rust_bridge
            dart pub global activate ffigen --version 5.0.1
            pushd /tmp && git clone https://github.com/SoLongAndThanksForAllThePizza/flutter_rust_bridge --depth=1 && popd
            pushd /tmp/flutter_rust_bridge/frb_codegen && cargo install --path . --locked && popd
            pushd flutter && flutter pub get && popd
            ~/.cargo/bin/flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs --dart-output ./flutter/lib/generated_bridge.dart
            # install vcpkg
            pushd /opt
            export VCPKG_ROOT=`pwd`/vcpkg
            git clone https://github.com/microsoft/vcpkg
            vcpkg/bootstrap-vcpkg.sh
            popd
            $VCPKG_ROOT/vcpkg install --x-install-root="$VCPKG_ROOT/installed"
            # build rustdesk
            ./build.py --flutter --hwcodec
        ''')
    system2("chmod +x /tmp/build.sh")
    system2("bash /tmp/build.sh")


# Downloading third party resources is deprecated.
# We can use this function in an offline build environment.
# Even in an online environment, we recommend building third-party resources yourself.
def download_extract_features(features, res_dir):
    import re

    proxy = ''

    def req(url):
        if not proxy:
            return url
        else:
            r = urllib.request.Request(url)
            r.set_proxy(proxy, 'http')
            r.set_proxy(proxy, 'https')
            return r

    for (feat, feat_info) in features.items():
        includes = feat_info['include'] if 'include' in feat_info and feat_info['include'] else []
        includes = [re.compile(p) for p in includes]
        excludes = feat_info['exclude'] if 'exclude' in feat_info and feat_info['exclude'] else []
        excludes = [re.compile(p) for p in excludes]

        print(f'{feat} download begin')
        download_filename = feat_info['zip_url'].split('/')[-1]
        checksum_md5_response = urllib.request.urlopen(
            req(feat_info['checksum_url']))
        for line in checksum_md5_response.read().decode('utf-8').splitlines():
            if line.split()[1] == download_filename:
                checksum_md5 = line.split()[0]
                filename, _headers = urllib.request.urlretrieve(feat_info['zip_url'],
                                                                download_filename)
                md5 = hashlib.md5(open(filename, 'rb').read()).hexdigest()
                if checksum_md5 != md5:
                    raise Exception(f'{feat} download failed')
                print(f'{feat} download end. extract bein')
                zip_file = zipfile.ZipFile(filename)
                zip_list = zip_file.namelist()
                for f in zip_list:
                    file_exclude = False
                    for p in excludes:
                        if p.match(f) is not None:
                            file_exclude = True
                            break
                    if file_exclude:
                        continue

                    file_include = False if includes else True
                    for p in includes:
                        if p.match(f) is not None:
                            file_include = True
                            break
                    if file_include:
                        print(f'extract file {f}')
                        zip_file.extract(f, res_dir)
                zip_file.close()
                os.remove(download_filename)
                print(f'{feat} extract end')


def external_resources(flutter, args, res_dir):
    features = parse_rc_features(args.feature)
    if not features:
        return

    print(f'Build with features {list(features.keys())}')
    if os.path.isdir(res_dir) and not os.path.islink(res_dir):
        shutil.rmtree(res_dir)
    elif os.path.exists(res_dir):
        raise Exception(f'Find file {res_dir}, not a directory')
    os.makedirs(res_dir, exist_ok=True)
    download_extract_features(features, res_dir)
    if flutter:
        os.makedirs(flutter_build_dir_2, exist_ok=True)
        for f in pathlib.Path(res_dir).iterdir():
            print(f'{f}')
            if f.is_file():
                shutil.copy2(f, flutter_build_dir_2)
            else:
                shutil.copytree(f, f'{flutter_build_dir_2}{f.stem}')


def get_features(args):
    features = ['inline'] if not args.flutter else []
    if args.hwcodec:
        features.append('hwcodec')
    if args.vram:
        features.append('vram')
    if args.flutter:
        features.append('flutter')
    if args.unix_file_copy_paste:
        features.append('unix-file-copy-paste')
    if not windows and not osx and args.drm:
        features.append('drm')
    if osx:
        if args.screencapturekit:
            features.append('screencapturekit')
    print("features:", features)
    return features


def generate_control_file(version, extra_depends="", package_name="rustdesk"):
    control_file_path = "../res/DEBIAN/control"
    system2('/bin/rm -rf %s' % control_file_path)

    # An alternative-build package (e.g. the opt-in unattended-wayland / DRM
    # variant) installs the same files as the stock `rustdesk` package, so it
    # must conflict with / replace it: you install one OR the other, not both.
    variant_control = ""
    if package_name != "rustdesk":
        variant_control = "Conflicts: rustdesk\nReplaces: rustdesk\nProvides: rustdesk\n"

    content = """Package: %s
Section: net
Priority: optional
Version: %s
Architecture: %s
Maintainer: rustdesk <info@rustdesk.com>
Homepage: https://rustdesk.com
%sDepends: libgtk-3-0t64 | libgtk-3-0, libxcb-randr0, libxdo3 | libxdo4, libxfixes3, libxcb-shape0, libxcb-xfixes0, libasound2t64 | libasound2, libsystemd0, curl, libva2, libva-drm2, libva-x11-2, libgstreamer-plugins-base1.0-0, libpam0g, gstreamer1.0-pipewire%s%s
Recommends: libayatana-appindicator3-1
Description: A remote control software.

""" % (package_name, version, get_deb_arch(), variant_control, get_deb_extra_depends(), extra_depends)
    file = open(control_file_path, "w")
    file.write(content)
    file.close()


def ffi_bindgen_function_refactor():
    # workaround ffigen
    system2(
        'sed -i "s/ffi.NativeFunction<ffi.Bool Function(DartPort/ffi.NativeFunction<ffi.Uint8 Function(DartPort/g" flutter/lib/generated_bridge.dart')


# libdrmtap is fetched at build time by cloning the rustdesk-org fork at a pinned
# ref — the same way rustdesk sources its other native build deps (vcpkg,
# flutter_rust_bridge, ...), rather than carrying a git submodule. It is the ONLY
# pin for the drm backend: rustdesk dlopens this .so at runtime and does not depend on
# the libdrmtap-sys crate (whose build.rs would statically link the C tree, a helper and
# libdrm/seccomp/cap). Override the repo/ref via env (DRMTAP_REPO / DRMTAP_REF) for
# local testing or another fork.
# Point at the maintainer-owned rustdesk-org repo. It has no release tag yet, so track its `main`
# branch and pin the exact commit via LIBDRMTAP_SHA below: the post-clone sha check makes this
# fail-closed, so `main` moving off the pinned commit fails the build instead of silently shipping a
# different .so. If rustdesk-org later publishes an immutable vX.Y.Z tag, set DRMTAP_REF to it.
LIBDRMTAP_REPO = os.environ.get('DRMTAP_REPO', 'https://github.com/rustdesk-org/libdrmtap')
LIBDRMTAP_REF = os.environ.get('DRMTAP_REF', 'main')
# The immutable commit the ref must resolve to. `git clone --branch` follows a mutable ref (a branch
# even more than a tag), so verifying this after clone catches a moved/compromised ref swapping the
# .so. Keep in sync with LIBDRMTAP_REF on every bump (override via DRMTAP_SHA together with DRMTAP_REF
# for a local fork). This commit is libdrmtap v0.4.13.
LIBDRMTAP_SHA = os.environ.get('DRMTAP_SHA', 'c9cf0938f3b10a3d4a9eeb9c6f97aaa1606c6b4a')


def _single_real_so(paths, where):
    # Return the one real libdrmtap.so.0.* object among `paths`, failing if there are zero or several.
    # glob order is arbitrary, so silently taking [0] could ship a stale or wrong-arch object left
    # over from an earlier build; a mismatch should fail the build loudly instead.
    real = sorted(p for p in paths if os.path.isfile(p) and not os.path.islink(p))
    if len(real) != 1:
        raise Exception(
            f'expected exactly one real libdrmtap.so.0.* in {where}, found {len(real)}: {real}')
    return real[0]


def build_libdrmtap_so():
    # Build libdrmtap.so from the rustdesk-org fork cloned at LIBDRMTAP_REF. The
    # pivot dlopen-s this .so in-process in the root service (which already holds
    # CAP_SYS_ADMIN) — no setcap helper, no privileged child. Only the shared
    # library target is built (the source also carries a helper binary we do not
    # ship). Returns the path to the built versioned .so (e.g. libdrmtap.so.0.4.x).
    repo_root = os.path.dirname(os.path.abspath(__file__))
    # Allow a caller (e.g. CI) to build the .so ahead of time and hand it in via
    # DRMTAP_PREBUILT_DIR (must contain the real libdrmtap.so.0.* object).
    prebuilt_dir = os.environ.get('DRMTAP_PREBUILT_DIR')
    if prebuilt_dir:
        # DRMTAP_PREBUILT_DIR explicitly names the artifact source, so honor it strictly: fail
        # (rather than silently falling back to a source build) if it holds no single real .so.
        prebuilt = glob.glob(os.path.join(prebuilt_dir, 'libdrmtap.so.0.*'))
        return _single_real_so(prebuilt, f'DRMTAP_PREBUILT_DIR={prebuilt_dir}')
    # Clone the pinned source if it is not already present (a shallow clone at the
    # ref). third_party/libdrmtap is not a submodule anymore; it is git-ignored.
    src = os.path.join(repo_root, 'third_party', 'libdrmtap')
    if not os.path.exists(os.path.join(src, 'meson.build')):
        if os.path.isdir(src):
            shutil.rmtree(src)
        os.makedirs(os.path.dirname(src), exist_ok=True)
        system2(f'git clone --depth 1 --branch {LIBDRMTAP_REF} {LIBDRMTAP_REPO} {src}')
    # Verify the immutable-commit pin whenever the source is a GIT checkout — a fresh clone OR a
    # reused/stale/mismatched one left by an earlier or failed clone: reject and remove it (the next
    # run re-clones cleanly). A NON-git tree placed here on purpose (a developer building unreleased
    # local libdrmtap source) has no tag to verify and is used as-is.
    if os.path.isdir(os.path.join(src, '.git')):
        got_sha = subprocess.check_output(
            ['git', '-C', src, 'rev-parse', 'HEAD']).decode().strip()
        if got_sha != LIBDRMTAP_SHA:
            shutil.rmtree(src, ignore_errors=True)
            raise Exception(
                f'libdrmtap {LIBDRMTAP_REF} at {src} is {got_sha}, expected {LIBDRMTAP_SHA} '
                f'(moved/compromised tag or stale checkout; removed, re-run to re-clone)')
    build_dir = os.path.join(src, 'build-pkg')
    if not os.path.exists(os.path.join(build_dir, 'build.ninja')):
        system2(f'meson setup {build_dir} {src} --buildtype=release')
    # Build only the shared library, not the bundled helper binary or the static archive. Since
    # libdrmtap 0.4.11 the project is `both_libraries` (a version-scripted .so + a static .a), so the
    # bare `drmtap` target is ambiguous ("drmtap:shared_library" vs "drmtap:static_library"); ask for
    # the shared one explicitly (rustdesk dlopens the .so and never needs the archive).
    system2(f'meson compile -C {build_dir} drmtap:shared_library')
    sos = glob.glob(os.path.join(build_dir, 'libdrmtap.so.0.*'))
    # keep the real object (libdrmtap.so.0.4.x), not the .so/.so.0 symlinks or meson's .p dir, and
    # require exactly one so a stale object from an earlier build is never silently picked.
    return _single_real_so(sos, f'the libdrmtap meson build dir {build_dir}')


def append_drm_ldconfig_postinst():
    # The DRM package installs libdrmtap.so under a private dir; register it with the
    # dynamic linker so the in-process dlopen("libdrmtap.so.0") resolves. Only the DRM
    # package calls this, so the stock package's postinst stays byte-identical to upstream.
    #
    # This block is appended AFTER the stock postinst, which has already run
    # `systemctl start rustdesk`. On a FRESH install that ordering is a trap: the root
    # service's DRM pre-warm can dlopen("libdrmtap.so.0") BEFORE this ldconfig has
    # populated the linker cache, the dlopen fails, and that failure is cached in the
    # DRMTAP_LIB OnceLock for the life of the process — so DRM stays disabled until a
    # manual restart. So immediately after ldconfig we `try-restart` the unit: it re-runs
    # the pre-warm against the now-resolvable soname. `try-restart` is a no-op when the
    # unit is not running, so it never spuriously starts the service.
    with open('tmpdeb/DEBIAN/postinst', 'a') as f:
        f.write(
            '\n'
            'if [ "$1" = configure ] && [ -d /usr/lib/rustdesk ]; then\n'
            '\tldconfig /usr/lib/rustdesk 2>/dev/null || ldconfig 2>/dev/null || true\n'
            '\tif command -v systemctl >/dev/null 2>&1; then\n'
            '\t\tsystemctl try-restart rustdesk 2>/dev/null || true\n'
            '\tfi\n'
            'fi\n'
        )


def finalize_deb(version, ships_so, so_basename=None):
    # Shared deb finalization for build_flutter_deb / build_deb_from_folder. Any DRM .so is assumed
    # already staged at tmpdeb/usr/lib/rustdesk/. For a DRM build this adds the soname symlink + the
    # ld.so.conf.d drop-in, names the package rustdesk-unattended-wayland with libdrmtap's runtime
    # deps (libdrm / EGL / GLESv2), and appends the ldconfig postinst; otherwise it builds the stock
    # rustdesk package. Then it writes the control, checksums, builds, and renames the .deb.
    if ships_so:
        system2(f'ln -sf {so_basename} tmpdeb/usr/lib/rustdesk/libdrmtap.so.0')
        # TODO(drm, Debian Policy 10.2): dropping /usr/lib/rustdesk into the SYSTEM-WIDE
        # linker search path (/etc/ld.so.conf.d) lets a privately-bundled library shadow a
        # system library for EVERY binary on the host, which Debian Policy 10.2 forbids.
        # The correct fix is to make the in-process dlopen resolve libdrmtap by ABSOLUTE
        # path ("/usr/lib/rustdesk/libdrmtap.so.0") -- or link the rustdesk cdylib with an
        # rpath of /usr/lib/rustdesk -- and then drop this ld.so.conf.d drop-in entirely.
        # That change lives at the dlopen call site in src/ (drmtap_dl.rs), owned by
        # another engineer, so it is out of scope for this packaging file; kept until then.
        system2('mkdir -p tmpdeb/etc/ld.so.conf.d')
        with open('tmpdeb/etc/ld.so.conf.d/rustdesk-unattended-wayland.conf', 'w') as f:
            f.write('/usr/lib/rustdesk\n')
    package_name = 'rustdesk-unattended-wayland' if ships_so else 'rustdesk'
    drm_depends = ", libdrm2, libegl1, libgles2" if ships_so else ""
    system2('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version, drm_depends, package_name)
    system2('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    if ships_so:
        append_drm_ldconfig_postinst()
    md5_file_folder("tmpdeb/")
    system2('dpkg-deb -b tmpdeb rustdesk.deb;')
    system2('/bin/rm -rf tmpdeb/')
    system2('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('rustdesk.deb', f'../{package_name}-{version}.deb')


def build_flutter_deb(version, features):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
        ffi_bindgen_function_refactor()
    os.chdir('flutter')
    system2('flutter build linux --release')
    system2('mkdir -p tmpdeb/usr/bin/')
    system2('mkdir -p tmpdeb/usr/share/rustdesk')
    system2('mkdir -p tmpdeb/etc/rustdesk/')
    system2('mkdir -p tmpdeb/etc/pam.d/')
    system2('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
    system2('mkdir -p tmpdeb/usr/share/applications/')
    system2('mkdir -p tmpdeb/usr/share/polkit-1/actions')
    system2('rm tmpdeb/usr/bin/rustdesk || true')
    system2(
        f'cp -r {flutter_build_dir}/* tmpdeb/usr/share/rustdesk/')
    system2(
        'cp ../res/rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
    system2(
        'cp ../res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/rustdesk.png')
    system2(
        'cp ../res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/rustdesk.svg')
    system2(
        'cp ../res/rustdesk.desktop tmpdeb/usr/share/applications/rustdesk.desktop')
    system2(
        'cp ../res/rustdesk-link.desktop tmpdeb/usr/share/applications/rustdesk-link.desktop')
    system2(
        'cp ../res/startwm.sh tmpdeb/etc/rustdesk/')
    system2(
        'cp ../res/xorg.conf tmpdeb/etc/rustdesk/')
    system2(
        'cp ../res/pam.d/rustdesk.debian tmpdeb/etc/pam.d/rustdesk')
    system2(
        "echo \"#!/bin/sh\" >> tmpdeb/usr/share/rustdesk/files/polkit && chmod a+x tmpdeb/usr/share/rustdesk/files/polkit")
    # Bundle libdrmtap.so for the DRM/KMS capture path — but ONLY when this build
    # actually enabled the `drm` feature, so normal packages stay opt-out. The root
    # service dlopen-s it in-process (no setcap helper); it lives in a private dir
    # that postinst registers with ldconfig so dlopen("libdrmtap.so.0") resolves.
    # Bundle libdrmtap.so for a DRM build (opt-in), then finalize the deb. A DRM build ships as a
    # separately-named rustdesk-unattended-wayland package (finalize_deb marks it
    # Conflicts/Replaces/Provides rustdesk), so installing it is an explicit choice.
    ships_so = 'drm' in features
    so_basename = None
    if ships_so:
        so_path = build_libdrmtap_so()
        so_basename = os.path.basename(so_path)
        system2('mkdir -p tmpdeb/usr/lib/rustdesk')
        system2(f'cp {so_path} tmpdeb/usr/lib/rustdesk/')
    finalize_deb(version, ships_so, so_basename)
    os.chdir("..")


def build_deb_from_folder(version, binary_folder, want_drm=False):
    os.chdir('flutter')
    system2('mkdir -p tmpdeb/usr/bin/')
    system2('mkdir -p tmpdeb/usr/share/rustdesk')
    system2('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
    system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
    system2('mkdir -p tmpdeb/usr/share/applications/')
    system2('mkdir -p tmpdeb/usr/share/polkit-1/actions')
    system2('rm tmpdeb/usr/bin/rustdesk || true')
    system2(
        f'cp -r ../{binary_folder}/* tmpdeb/usr/share/rustdesk/')
    system2(
        'cp ../res/rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
    system2(
        'cp ../res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/rustdesk.png')
    system2(
        'cp ../res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/rustdesk.svg')
    system2(
        'cp ../res/rustdesk.desktop tmpdeb/usr/share/applications/rustdesk.desktop')
    system2(
        'cp ../res/rustdesk-link.desktop tmpdeb/usr/share/applications/rustdesk-link.desktop')
    system2(
        "echo \"#!/bin/sh\" >> tmpdeb/usr/share/rustdesk/files/polkit && chmod a+x tmpdeb/usr/share/rustdesk/files/polkit")
    # A staged bundle (binary_folder) carries its own libdrmtap.so.0* for a --drm build, so we do
    # not rebuild it here; the `cp -r` above placed it under usr/share/rustdesk/. Move it to the
    # private lib dir, then finalize the deb the same way build_flutter_deb does.
    bundled_glob = glob.glob('tmpdeb/usr/share/rustdesk/libdrmtap.so.0.*')
    ships_so = any(os.path.isfile(p) and not os.path.islink(p) for p in bundled_glob)
    # The variant must be decided by the EXPLICIT --drm request, not merely by what happens
    # to be staged. Cross-check the two and fail loudly on a mismatch: a drm binary staged
    # WITHOUT its libdrmtap.so.0.* would otherwise be silently shipped as the stock
    # `rustdesk` package (no drm deps, no ldconfig, a dlopen that can never resolve), and a
    # bundle that DOES carry the .so would be shipped as the consent-bypass variant even
    # when --drm was never asked for.
    if want_drm and not ships_so:
        raise Exception(
            '--drm was requested but no real libdrmtap.so.0.* is staged under '
            'usr/share/rustdesk/ in the bundle; refusing to package a drm binary as the '
            'stock rustdesk package (it would ship without the capture library or its deps)')
    if ships_so and not want_drm:
        raise Exception(
            'the staged bundle carries libdrmtap.so.0.* but --drm was not passed; refusing '
            'to silently ship the consent-bypass unattended-wayland variant (pass --drm to '
            'build it deliberately)')
    so_basename = None
    if ships_so:
        so = _single_real_so(bundled_glob, 'the staged --drm bundle')
        so_basename = os.path.basename(so)
        system2('mkdir -p tmpdeb/usr/lib/rustdesk')
        system2(f'mv {so} tmpdeb/usr/lib/rustdesk/')
        system2('rm -f tmpdeb/usr/share/rustdesk/libdrmtap.so tmpdeb/usr/share/rustdesk/libdrmtap.so.0')
    finalize_deb(version, ships_so, so_basename)
    os.chdir("..")


def build_flutter_dmg(version, features):
    if not skip_cargo:
        # set minimum osx build target, now is 10.14, which is the same as the flutter xcode project
        system2(
            f'MACOSX_DEPLOYMENT_TARGET=10.14 cargo build --locked --features {features} --release')
    # copy dylib
    system2(
        "cp target/release/liblibrustdesk.dylib target/release/librustdesk.dylib")
    os.chdir('flutter')
    # cargo builds a single-arch dylib for the host; restrict Xcode to the same arch
    # so the universal-by-default ARCHS_STANDARD doesn't try to link a missing slice.
    # FLUTTER_XCODE_* env vars are forwarded to xcodebuild as build settings.
    mac_arch = 'arm64' if platform.machine().lower() in ('arm64', 'aarch64') else 'x86_64'
    system2(
        f'FLUTTER_XCODE_ARCHS={mac_arch} FLUTTER_XCODE_ONLY_ACTIVE_ARCH=YES flutter build macos --release')
    system2('cp -rf ../target/release/service ./build/macos/Build/Products/Release/RustDesk.app/Contents/MacOS/')
    '''
    system2(
        "create-dmg --volname \"RustDesk Installer\" --window-pos 200 120 --window-size 800 400 --icon-size 100 --app-drop-link 600 185 --icon RustDesk.app 200 190 --hide-extension RustDesk.app rustdesk.dmg ./build/macos/Build/Products/Release/RustDesk.app")
    os.rename("rustdesk.dmg", f"../rustdesk-{version}.dmg")
    '''
    os.chdir("..")


def build_flutter_arch_manjaro(version, features):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
    ffi_bindgen_function_refactor()
    os.chdir('flutter')
    system2('flutter build linux --release')
    system2(f'strip {flutter_build_dir}/lib/librustdesk.so')
    os.chdir('../res')
    system2('HBB=`pwd`/.. FLUTTER=1 makepkg -f')


def build_flutter_windows(version, features, skip_portable_pack):
    if not skip_cargo:
        system2(f'cargo build --locked --features {features} --lib --release')
        if not os.path.exists("target/release/librustdesk.dll"):
            print("cargo build failed, please check rust source code.")
            exit(-1)
    os.chdir('flutter')
    system2('flutter build windows --release')
    os.chdir('..')
    shutil.copy2('target/release/deps/dylib_virtual_display.dll',
                 flutter_build_dir_2)
    if skip_portable_pack:
        return
    os.chdir('libs/portable')
    system2('pip3 install -r requirements.txt')
    system2(
        f'python3 ./generate.py -f ../../{flutter_build_dir_2} -o . -e ../../{flutter_build_dir_2}/rustdesk.exe')
    os.chdir('../..')
    if os.path.exists('./rustdesk_portable.exe'):
        os.replace('./target/release/rustdesk-portable-packer.exe',
                   './rustdesk_portable.exe')
    else:
        os.rename('./target/release/rustdesk-portable-packer.exe',
                  './rustdesk_portable.exe')
    print(
        f'output location: {os.path.abspath(os.curdir)}/rustdesk_portable.exe')
    os.rename('./rustdesk_portable.exe', f'./rustdesk-{version}-install.exe')
    print(
        f'output location: {os.path.abspath(os.curdir)}/rustdesk-{version}-install.exe')


def main():
    global skip_cargo
    parser = make_parser()
    args = parser.parse_args()

    if os.path.exists(exe_path):
        os.unlink(exe_path)
    if os.path.isfile('/usr/bin/pacman'):
        system2('git checkout src/ui/common.tis')
    version = get_version()
    features = ','.join(get_features(args))
    flutter = args.flutter
    if not flutter:
        system2('python3 res/inline-sciter.py')
    print(args.skip_cargo)
    if args.skip_cargo:
        skip_cargo = True
    portable = args.portable
    package = args.package
    if package:
        build_deb_from_folder(version, package, args.drm)
        return
    res_dir = 'resources'
    external_resources(flutter, args, res_dir)
    if windows:
        # build virtual display dynamic library
        os.chdir('libs/virtual_display/dylib')
        system2('cargo build --locked --release')
        os.chdir('../../..')

        if flutter:
            build_flutter_windows(version, features, args.skip_portable_pack)
            return
        system2('cargo build --locked --release --features ' + features)
        # system2('upx.exe target/release/rustdesk.exe')
        system2('mv target/release/rustdesk.exe target/release/RustDesk.exe')
        pa = os.environ.get('P')
        if pa:
            # https://certera.com/kb/tutorial-guide-for-safenet-authentication-client-for-code-signing/
            system2(
                f'signtool sign /a /v /p {pa} /debug /f .\\cert.pfx /t http://timestamp.digicert.com  '
                'target\\release\\rustdesk.exe')
        else:
            print('Not signed')
        os.makedirs(res_dir, exist_ok=True)
        system2(
            f'cp -rf target/release/RustDesk.exe {res_dir}')
        os.chdir('libs/portable')
        system2('pip3 install -r requirements.txt')
        system2(
            f'python3 ./generate.py -f ../../{res_dir} -o . -e ../../{res_dir}/rustdesk-{version}-win7-install.exe')
        system2(f'mv ../../{res_dir}/rustdesk-{version}-win7-install.exe ../..')
    elif os.path.isfile('/usr/bin/pacman'):
        # pacman -S -needed base-devel
        system2("sed -i 's/pkgver=.*/pkgver=%s/g' res/PKGBUILD" % version)
        if flutter:
            build_flutter_arch_manjaro(version, features)
        else:
            system2('cargo build --locked --release --features ' + features)
            system2('git checkout src/ui/common.tis')
            system2('strip target/release/rustdesk')
            system2('ln -s res/pacman_install && ln -s res/PKGBUILD')
            system2('HBB=`pwd` makepkg -f')
        system2('mv rustdesk-%s-0-x86_64.pkg.tar.zst rustdesk-%s-manjaro-arch.pkg.tar.zst' % (
            version, version))
        # pacman -U ./rustdesk.pkg.tar.zst
    elif os.path.isfile('/usr/bin/yum'):
        system2('cargo build --locked --release --features ' + features)
        system2('strip target/release/rustdesk')
        system2(
            "sed -i 's/Version:    .*/Version:    %s/g' res/rpm.spec" % version)
        system2('HBB=`pwd` rpmbuild -ba res/rpm.spec')
        system2(
            'mv $HOME/rpmbuild/RPMS/x86_64/rustdesk-%s-0.x86_64.rpm ./rustdesk-%s-fedora28-centos8.rpm' % (
                version, version))
        # yum localinstall rustdesk.rpm
    elif os.path.isfile('/usr/bin/zypper'):
        system2('cargo build --locked --release --features ' + features)
        system2('strip target/release/rustdesk')
        system2(
            "sed -i 's/Version:    .*/Version:    %s/g' res/rpm-suse.spec" % version)
        system2('HBB=`pwd` rpmbuild -ba res/rpm-suse.spec')
        system2(
            'mv $HOME/rpmbuild/RPMS/x86_64/rustdesk-%s-0.x86_64.rpm ./rustdesk-%s-suse.rpm' % (
                version, version))
        # yum localinstall rustdesk.rpm
    else:
        if flutter:
            if osx:
                build_flutter_dmg(version, features)
                pass
            else:
                # system2(
                #     'mv target/release/bundle/deb/rustdesk*.deb ./flutter/rustdesk.deb')
                build_flutter_deb(version, features)
        else:
            system2('cargo --locked bundle --release --features ' + features)
            if osx:
                system2(
                    'strip target/release/bundle/osx/RustDesk.app/Contents/MacOS/rustdesk')
                system2(
                    'cp libsciter.dylib target/release/bundle/osx/RustDesk.app/Contents/MacOS/')
                # https://github.com/sindresorhus/create-dmg
                system2('/bin/rm -rf *.dmg')
                pa = os.environ.get('P')
                if pa:
                    system2('''
    # buggy: rcodesign sign ... path/*, have to sign one by one
    # install rcodesign via cargo install apple-codesign
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/rustdesk
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/libsciter.dylib
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./target/release/bundle/osx/RustDesk.app
    # goto "Keychain Access" -> "My Certificates" for below id which starts with "Developer ID Application:"
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/RustDesk.app/Contents/MacOS/*
    codesign -s "Developer ID Application: {0}" --force --options runtime  ./target/release/bundle/osx/RustDesk.app
    '''.format(pa))
                system2(
                    'create-dmg "RustDesk %s.dmg" "target/release/bundle/osx/RustDesk.app"' % version)
                os.rename('RustDesk %s.dmg' %
                          version, 'rustdesk-%s.dmg' % version)
                if pa:
                    system2('''
    # https://pyoxidizer.readthedocs.io/en/apple-codesign-0.14.0/apple_codesign.html
    # https://pyoxidizer.readthedocs.io/en/stable/tugger_code_signing.html
    # https://developer.apple.com/developer-id/
    # goto xcode and login with apple id, manager certificates (Developer ID Application and/or Developer ID Installer) online there (only download and double click (install) cer file can not export p12 because no private key)
    #rcodesign sign --p12-file ~/.p12/rustdesk-developer-id.p12 --p12-password-file ~/.p12/.cert-pass --code-signature-flags runtime ./rustdesk-{1}.dmg
    codesign -s "Developer ID Application: {0}" --force --options runtime ./rustdesk-{1}.dmg
    # https://appstoreconnect.apple.com/access/api
    # https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_getting_started.html#apple-codesign-app-store-connect-api-key
    # p8 file is generated when you generate api key (can download only once)
    rcodesign notary-submit --api-key-path ../.p12/api-key.json  --staple rustdesk-{1}.dmg
    # verify:  spctl -a -t exec -v /Applications/RustDesk.app
    '''.format(pa, version))
                else:
                    print('Not signed')
            else:
                # build deb package
                system2(
                    'mv target/release/bundle/deb/rustdesk*.deb ./rustdesk.deb')
                system2('dpkg-deb -R rustdesk.deb tmpdeb')
                system2('mkdir -p tmpdeb/usr/share/rustdesk/files/systemd/')
                system2('mkdir -p tmpdeb/usr/share/icons/hicolor/256x256/apps/')
                system2('mkdir -p tmpdeb/usr/share/icons/hicolor/scalable/apps/')
                system2(
                    'cp res/rustdesk.service tmpdeb/usr/share/rustdesk/files/systemd/')
                system2(
                    'cp res/128x128@2x.png tmpdeb/usr/share/icons/hicolor/256x256/apps/rustdesk.png')
                system2(
                    'cp res/scalable.svg tmpdeb/usr/share/icons/hicolor/scalable/apps/rustdesk.svg')
                system2(
                    'cp res/rustdesk.desktop tmpdeb/usr/share/applications/rustdesk.desktop')
                system2(
                    'cp res/rustdesk-link.desktop tmpdeb/usr/share/applications/rustdesk-link.desktop')
                os.system('mkdir -p tmpdeb/etc/rustdesk/')
                os.system('cp -a res/startwm.sh tmpdeb/etc/rustdesk/')
                os.system('mkdir -p tmpdeb/etc/X11/rustdesk/')
                os.system('cp res/xorg.conf tmpdeb/etc/X11/rustdesk/')
                os.system('cp -a DEBIAN/* tmpdeb/DEBIAN/')
                os.system('mkdir -p tmpdeb/etc/pam.d/')
                os.system('cp pam.d/rustdesk.debian tmpdeb/etc/pam.d/rustdesk')
                system2('strip tmpdeb/usr/bin/rustdesk')
                system2('mkdir -p tmpdeb/usr/share/rustdesk')
                system2('mv tmpdeb/usr/bin/rustdesk tmpdeb/usr/share/rustdesk/')
                system2('cp libsciter-gtk.so tmpdeb/usr/share/rustdesk/')
                md5_file_folder("tmpdeb/")
                system2('dpkg-deb -b tmpdeb rustdesk.deb; /bin/rm -rf tmpdeb/')
                os.rename('rustdesk.deb', 'rustdesk-%s.deb' % version)


def md5_file(fn):
    md5 = hashlib.md5(open('tmpdeb/' + fn, 'rb').read()).hexdigest()
    system2('echo "%s  /%s" >> tmpdeb/DEBIAN/md5sums' % (md5, fn))

def md5_file_folder(base_dir):
    base_path = Path(base_dir)
    for file in base_path.rglob('*'):
        if file.is_file() and 'DEBIAN' not in file.parts:
            relative_path = file.relative_to(base_path)
            md5_file(str(relative_path))


if __name__ == "__main__":
    main()
