#!/usr/bin/env python3

import os
import glob
import contextlib
import pathlib
import platform
import zipfile
import urllib.request
import shutil
import hashlib
import re
import subprocess
import argparse
import sys
from pathlib import Path

# Captured at import, while cwd is still the repo root: before Python 3.9 the main script's __file__
# stays relative (bpo-20443), so abspath() re-resolves it against the cwd -- and the ubuntu18.04
# packaging container runs 3.6 and chdir's into flutter/ before it reaches the libdrmtap code.
REPO_ROOT = os.path.dirname(os.path.abspath(__file__))

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
        '--print-features',
        action='store_true',
        help='Print the cargo feature list these flags select, and exit without building. For a '
             'caller that runs its own cargo line and then packages with --skip-cargo: it can ask '
             'for the list rather than repeat it, so the two cannot drift.'
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


def linux_packaging_branch():
    """Which packaging path `main()` will take on THIS host.

    MUST mirror the elif chain in main() (pacman / yum / zypper / else), and exists so `--drm` can
    refuse a branch that is not drm-aware instead of silently producing a stock-named package with
    the capture backend compiled in. Only the final `deb` branch reaches `build_flutter_deb`, which
    is what bundles libdrmtap, renames the package, adds Conflicts/Provides and asserts the staged
    binary really is a drm build.
    """
    if os.path.isfile('/usr/bin/pacman'):
        return 'pacman'
    if os.path.isfile('/usr/bin/yum'):
        return 'yum'
    if os.path.isfile('/usr/bin/zypper'):
        return 'zypper'
    return 'deb'


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
    if args.drm:
        # Say so rather than quietly handing back a stock build: the backend is Linux-only, so on
        # any other host the flag cannot be honoured and the resulting binary would look like a
        # DRM build without being one.
        if windows or osx:
            raise Exception('--drm is Linux only')
        # And only on the deb branch. The other three Linux paths (pacman/yum/zypper) package
        # straight from `target/release` without bundling libdrmtap, without the rename, without
        # Conflicts/Provides and without assert_staged_binary_is_drm() -- so they would emit a
        # package NAMED `rustdesk` carrying the consent-bypass backend and the root-side uinput
        # injection. The separate package name is the informed consent this feature rests on, so
        # refuse rather than ship a stock-named build of it.
        branch = linux_packaging_branch()
        if branch != 'deb':
            raise Exception(
                f'--drm is only supported on the deb packaging path; this host would package via '
                f'{branch}, which cannot bundle libdrmtap or name the package distinctly')
        features.append('drm')
        # The display wake is its own compile gate on top of `drm`, and the unattended package is
        # exactly where it belongs: that variant exists to reach a machine nobody is sitting at,
        # and a machine whose screen went dark is the case it is for. Dropping `drm-wake` from
        # this line builds the same capture backend with no wake code in the binary at all.
        # It is ALSO switchable at runtime; see OPTION_ENABLE_DRM_DISPLAY_WAKE.
        features.append('drm-wake')
    if osx:
        if args.screencapturekit:
            features.append('screencapturekit')
    print("features:", features)
    return features


def generate_control_file(version):
    control_file_path = "../res/DEBIAN/control"
    system2('/bin/rm -rf %s' % control_file_path)

    content = """Package: rustdesk
Section: net
Priority: optional
Version: %s
Architecture: %s
Maintainer: rustdesk <info@rustdesk.com>
Homepage: https://rustdesk.com
Depends: libgtk-3-0t64 | libgtk-3-0, libxcb-randr0, libxdo3 | libxdo4, libxfixes3, libxcb-shape0, libxcb-xfixes0, libasound2t64 | libasound2, libsystemd0, curl, libva2, libva-drm2, libva-x11-2, libgstreamer-plugins-base1.0-0, libpam0g, gstreamer1.0-pipewire%s
Recommends: libayatana-appindicator3-1
Description: A remote control software.

""" % (version, get_deb_arch(), get_deb_extra_depends())
    file = open(control_file_path, "w")
    file.write(content)
    file.close()


def ffi_bindgen_function_refactor():
    # workaround ffigen
    system2(
        'sed -i "s/ffi.NativeFunction<ffi.Bool Function(DartPort/ffi.NativeFunction<ffi.Uint8 Function(DartPort/g" flutter/lib/generated_bridge.dart')


# libdrmtap is fetched at build time from the rustdesk-org fork at a pinned
# commit — the same way rustdesk sources its other native build deps (vcpkg,
# flutter_rust_bridge, ...), rather than carrying a git submodule. It is the ONLY
# pin for the drm backend: rustdesk dlopens this .so at runtime and does not depend on
# the libdrmtap-sys crate (whose build.rs would statically link the C tree, a helper and
# libdrm/seccomp/cap). DRMTAP_REPO, DRMTAP_SHA and DRMTAP_PREBUILT_DIR override it for local testing
# or another fork, and each requires DRMTAP_ALLOW_UNPINNED=1 alongside it (see below).
# The commit is fetched directly by sha, so no branch or tag name takes part in the build: see
# build_libdrmtap_so(). This is the SINGLE source of truth for the pin, deliberately not duplicated in
# any workflow, so a bump is one edit here (plus the informational version comment in
# libs/scrap/Cargo.toml). This commit is libdrmtap v0.5.4.
LIBDRMTAP_REPO_PINNED = 'https://github.com/rustdesk-org/libdrmtap'
LIBDRMTAP_SHA_PINNED = '5da68a3a368db569716d0d0f11cefacbb11b2290'
LIBDRMTAP_REPO = os.environ.get('DRMTAP_REPO', LIBDRMTAP_REPO_PINNED)
LIBDRMTAP_SHA = os.environ.get('DRMTAP_SHA', LIBDRMTAP_SHA_PINNED)
# Every way of getting a different .so than the pin needs the same explicit opt-in. Otherwise the
# claim this feature rests on -- that the privileged capture library is the reviewed object at
# LIBDRMTAP_SHA_PINNED -- would hold only as long as nobody happened to have one of these set, and a
# build that silently used something else would be indistinguishable from one that did not.
# DRMTAP_PREBUILT_DIR is in the list because it is the widest of the three: it skips both the fetch
# and the sha verification and hands over an object built from nothing this script can see.
DRMTAP_UNPINNED_OK = os.environ.get('DRMTAP_ALLOW_UNPINNED') == '1'


def _prebuilt_dir_is_the_pinned_checkout(prebuilt_dir):
    # A .so built from this repo's own third_party/libdrmtap at the pinned sha is the pinned object,
    # not an override, so it must not need the opt-in. This is how CI hands the library from a step
    # that has meson to a packaging container that does not.
    src = os.path.join(REPO_ROOT, 'third_party', 'libdrmtap')
    try:
        inside = os.path.commonpath([os.path.abspath(prebuilt_dir), src]) == src
    except ValueError:
        return False
    if not inside or not os.path.isdir(os.path.join(src, '.git')):
        return False
    try:
        head = subprocess.check_output(['git', '-C', src, 'rev-parse', 'HEAD']).decode().strip()
    except (subprocess.SubprocessError, OSError):
        return False
    return head == LIBDRMTAP_SHA


def _validate_libdrmtap_pin():
    # Called from build_libdrmtap_so(), NOT at import: a stock (non --drm) build must stay
    # byte-identical to upstream in behaviour too, and leftover DRMTAP_* variables in the
    # environment (or a malformed sha) must not be able to fail a build that never touches
    # libdrmtap.
    # `or None` so an empty value reads as unset here exactly as it does in build_libdrmtap_so(),
    # which tests it for truthiness.
    prebuilt = os.environ.get('DRMTAP_PREBUILT_DIR') or None
    if prebuilt and _prebuilt_dir_is_the_pinned_checkout(prebuilt):
        prebuilt = None
    overridden = [
        name
        for name, value, pinned in (
            ('DRMTAP_REPO', LIBDRMTAP_REPO, LIBDRMTAP_REPO_PINNED),
            ('DRMTAP_SHA', LIBDRMTAP_SHA, LIBDRMTAP_SHA_PINNED),
            ('DRMTAP_PREBUILT_DIR', prebuilt, None),
        )
        if value != pinned
    ]
    if overridden and not DRMTAP_UNPINNED_OK:
        raise Exception(
            f'{", ".join(overridden)} would build libdrmtap from something other than the pinned '
            f'{LIBDRMTAP_REPO_PINNED} at {LIBDRMTAP_SHA_PINNED}. That is supported for local work and '
            'cross-builds, but it has to be deliberate: set DRMTAP_ALLOW_UNPINNED=1 as well.')
    if overridden:
        print(f'WARNING: libdrmtap is NOT the pinned build ({", ".join(overridden)} set)')
    # Both are interpolated into shell commands below, and both are env-overridable, so validate
    # their SHAPE before they get there. This is not only about a hostile environment: a truncated
    # or abbreviated sha would otherwise reach `git fetch` and fail with something far less obvious
    # than saying so here, and an abbreviated one would defeat the point of pinning.
    if not re.fullmatch(r'[0-9a-f]{40}', LIBDRMTAP_SHA):
        raise Exception(
            f'DRMTAP_SHA must be a full 40-character commit sha, got {LIBDRMTAP_SHA!r}')
    if not re.fullmatch(r'(https://|git@)[A-Za-z0-9._~:/@-]+', LIBDRMTAP_REPO):
        raise Exception(f'DRMTAP_REPO does not look like a git remote url: {LIBDRMTAP_REPO!r}')


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
    # Build libdrmtap.so from the rustdesk-org fork, fetched at the pinned LIBDRMTAP_SHA. The
    # pivot dlopen-s this .so in-process in the root service (which already holds
    # CAP_SYS_ADMIN) — no setcap helper, no privileged child. Only the shared
    # library target is built (the source also carries a helper binary we do not
    # ship). Returns the path to the built versioned .so (e.g. libdrmtap.so.0.4.x).
    _validate_libdrmtap_pin()
    # Allow a caller (e.g. CI) to build the .so ahead of time and hand it in via
    # DRMTAP_PREBUILT_DIR (must contain the real libdrmtap.so.0.* object).
    prebuilt_dir = os.environ.get('DRMTAP_PREBUILT_DIR')
    if prebuilt_dir:
        # DRMTAP_PREBUILT_DIR explicitly names the artifact source, so honor it strictly: fail
        # (rather than silently falling back to a source build) if it holds no single real .so.
        prebuilt = glob.glob(os.path.join(prebuilt_dir, 'libdrmtap.so.0.*'))
        so = _single_real_so(prebuilt, f'DRMTAP_PREBUILT_DIR={prebuilt_dir}')
        # Check the stub case HERE too, not only on the source path below. This is the widest
        # override of the three -- no fetch, no sha verification, an object built by something this
        # script cannot see -- so it is the likeliest to hand over a CPU-only build, and skipping the
        # assertion on exactly this path would leave the check guarding only the case that was
        # already trustworthy.
        _assert_so_has_egl(so)
        return so
    # Fetch the pinned source if it is not already present. third_party/libdrmtap is not a submodule
    # anymore; it is git-ignored. The commit is fetched BY SHA rather than by cloning a branch:
    # `clone --depth 1 --branch main` only ever fetches the tip, so the moment upstream pushes to
    # `main` the pinned commit is not in the shallow clone at all and the build fails on an unreachable
    # object. Fetching the sha needs no branch name, so it keeps working across every upstream push and
    # is immune to a ref being moved or repointed.
    src = os.path.join(REPO_ROOT, 'third_party', 'libdrmtap')
    if not os.path.exists(os.path.join(src, 'meson.build')):
        if os.path.isdir(src):
            shutil.rmtree(src)
        os.makedirs(src, exist_ok=True)
        system2(f'git -C "{src}" init -q')
        system2(f'git -C "{src}" remote add origin {LIBDRMTAP_REPO}')
        system2(f'git -C "{src}" fetch --depth 1 origin {LIBDRMTAP_SHA}')
        system2(f'git -C "{src}" checkout -q FETCH_HEAD')
    # Verify the pin whenever the source is a GIT checkout. A fetch by sha cannot resolve to anything
    # else, so this now guards the OTHER case: a reused checkout left by an earlier build at a
    # different pin, which is what a bump leaves behind. Reject and remove it so the next run re-fetches
    # cleanly. A NON-git tree placed here on purpose (a developer building unreleased local libdrmtap
    # source) has nothing to verify and is used as-is.
    if os.path.isdir(os.path.join(src, '.git')):
        got_sha = subprocess.check_output(
            ['git', '-C', src, 'rev-parse', 'HEAD']).decode().strip()
        if got_sha != LIBDRMTAP_SHA:
            shutil.rmtree(src, ignore_errors=True)
            raise Exception(
                f'libdrmtap at {src} is {got_sha}, expected {LIBDRMTAP_SHA} '
                f'(stale checkout from a different pin; removed, re-run to re-fetch)')
    build_dir = os.path.join(src, 'build-pkg')
    if not os.path.exists(os.path.join(build_dir, 'build.ninja')):
        system2(f'meson setup "{build_dir}" "{src}" --buildtype=release')
    # Build only the shared library, not the bundled helper binary or the static archive. Since
    # libdrmtap 0.4.11 the project is `both_libraries` (a version-scripted .so + a static .a), so the
    # bare `drmtap` target is ambiguous ("drmtap:shared_library" vs "drmtap:static_library"); ask for
    # the shared one explicitly (rustdesk dlopens the .so and never needs the archive).
    system2(f'meson compile -C "{build_dir}" drmtap:shared_library')
    sos = glob.glob(os.path.join(build_dir, 'libdrmtap.so.0.*'))
    # keep the real object (libdrmtap.so.0.4.x), not the .so/.so.0 symlinks or meson's .p dir, and
    # require exactly one so a stale object from an earlier build is never silently picked.
    so = _single_real_so(sos, f'the libdrmtap meson build dir {build_dir}')
    _assert_so_has_egl(so)
    return so


def _assert_so_has_egl(so_path):
    # libdrmtap treats egl/glesv2 as OPTIONAL dependencies: without their headers and pkg-config
    # files, meson silently builds a CPU-only stub. That stub still exports every symbol the loader
    # checks for, so nothing downstream notices -- and the split architecture depends entirely on the
    # unprivileged side EGL-detiling the scanout it receives. The result is a build where DRM capture
    # quietly degrades to PipeWire on every tiled-scanout host, which is most of them.
    #
    # Assert on the ARTIFACT rather than passing an option that demands it: `-Degl=enabled` exists
    # only in libdrmtap past 0.4.15, and checking what was actually produced also catches a stale or
    # hand-substituted object, which a build flag cannot.
    #
    # EGL is reached by lazy dlopen, on purpose, so that the privileged service never links the GPU
    # stack. That means there is no DT_NEEDED to look for and an ELF-level check reports "no EGL" on a
    # perfectly good library; the dlopen name and an extension symbol are what a CPU-only stub really
    # lacks.
    try:
        with open(so_path, 'rb') as f:
            blob = f.read()
    except OSError as err:
        raise Exception(f'cannot read the built libdrmtap at {so_path}: {err}') from err
    missing = [m for m in (b'libEGL.so.1', b'eglCreateImageKHR') if m not in blob]
    if missing:
        raise Exception(
            f'{so_path} looks like a CPU-only libdrmtap stub (missing '
            f'{", ".join(m.decode() for m in missing)}): the EGL detile path the split capture '
            'depends on is not in it, and DRM capture would silently fall back to PipeWire. '
            'Install the EGL development packages and rebuild (Debian/Ubuntu: libegl-dev '
            'libgles2-mesa-dev; Arch: mesa libglvnd).')


DRM_PACKAGE_NAME = 'rustdesk-unattended-wayland'


def assert_so_satisfies_the_runtime_abi_gate(so_path):
    """The .so we are about to ship must be one the RUNTIME will actually accept.

    `abi_accepted` in libs/scrap/src/common/drmtap_dl.rs is the only place the pinned library's
    version is ever validated, and it runs at dlopen time on the USER's machine. Nothing in the
    build or in CI compared the two, so the pin and the gate could drift apart and every existing
    assertion would still pass: the EGL check does not look at the version, the CI symbol contract
    does not call drmtap_version(), and the deb-contents regex matches any `libdrmtap.so.0.X.Y`.
    A green pipeline could therefore produce a deb in which DRM capture can never start, and the
    only symptom on the host is one log line before it falls back to the portal.

    So parse the gate out of the Rust and apply it here, to the object being staged. This is the
    same rule, not a copy of the numbers: if someone bumps the constants, this reads the new ones.
    """
    m = re.search(r'libdrmtap\.so\.(\d+)\.(\d+)\.(\d+)', os.path.basename(so_path))
    if not m:
        # Not a versioned soname (a local dev build, say). The gate cannot be evaluated, and
        # inventing a verdict would be worse than saying so.
        print(f'[drm] cannot read a version out of {so_path}; skipping the ABI-gate cross-check')
        return
    so_ver = tuple(int(g) for g in m.groups())
    # REPO_ROOT, not abspath(__file__): both callers have chdir'd into flutter/ by now.
    gate_path = os.path.join(REPO_ROOT, 'libs', 'scrap', 'src', 'common', 'drmtap_dl.rs')
    with open(gate_path) as f:
        gate_src = f.read()

    def _const(name):
        mm = re.search(rf'const {name}: c_int = (\d+);', gate_src)
        return int(mm.group(1)) if mm else None

    major, minor = _const('DRMTAP_ABI_MAJOR'), _const('DRMTAP_ABI_MINOR')
    mm = re.search(r'const DRMTAP_MIN_MINOR_PATCH: \(c_int, c_int\) = \((\d+), (\d+)\);', gate_src)
    floor = (int(mm.group(1)), int(mm.group(2))) if mm else None
    if major is None or minor is None or floor is None:
        raise Exception(
            'could not parse the libdrmtap ABI gate out of drmtap_dl.rs (DRMTAP_ABI_MAJOR / '
            'DRMTAP_ABI_MINOR / DRMTAP_MIN_MINOR_PATCH). The gate moved and this check did not; '
            'fix the check rather than removing it, or the pin and the gate can drift silently.')
    accepted = so_ver[0] == major and so_ver[1] == minor and (so_ver[1], so_ver[2]) >= floor
    if not accepted:
        raise Exception(
            f'the libdrmtap being packaged is {so_ver[0]}.{so_ver[1]}.{so_ver[2]}, which the '
            f'runtime loader would REFUSE: drmtap_dl.rs accepts exactly major {major}, minor '
            f'{minor}, patch >= {floor[1]}. Shipping it produces a deb whose DRM capture can never '
            'start. Move the build pin and the gate together, or fix whichever one is wrong.')
    print(f'[drm] libdrmtap {so_ver[0]}.{so_ver[1]}.{so_ver[2]} satisfies the runtime ABI gate '
          f'(major {major}, minor {minor}, patch >= {floor[1]})')


def stage_libdrmtap_into_deb(so_path):
    # Put the built libdrmtap object plus its soname symlink into the staged deb. Only the soname
    # symlink is needed: libdrmtap is resolved by ABSOLUTE path (/usr/lib/rustdesk/libdrmtap.so.0) at
    # the in-process dlopen site (drmtap_dl.rs), so the deb does NOT drop /usr/lib/rustdesk into the
    # system-wide /etc/ld.so.conf.d search path, which would let this private library shadow a system
    # library for every binary on the host (Debian Policy 10.2 forbids that). No ld.so.conf.d drop-in
    # and no ldconfig trigger are shipped, so the stock postinst is used unchanged.
    assert_so_satisfies_the_runtime_abi_gate(so_path)
    so_basename = os.path.basename(so_path)
    system2('mkdir -p tmpdeb/usr/lib/rustdesk')
    # Quoted: so_path comes from the repo root or from DRMTAP_PREBUILT_DIR, either of which can
    # contain a space, and an unquoted interpolation would split the argument and fail obscurely.
    system2(f'cp "{so_path}" tmpdeb/usr/lib/rustdesk/')
    system2(f'ln -sf "{so_basename}" tmpdeb/usr/lib/rustdesk/libdrmtap.so.0')


def _max_glibc_minor(path):
    # Read from .dynstr rather than via objdump so packaging needs no binutils; chunked because
    # librustdesk.so is ~45 MB.
    best = 0
    with open(path, 'rb') as f:
        tail = b''
        while True:
            chunk = f.read(1 << 20)
            if not chunk:
                return best
            for m in re.finditer(rb'GLIBC_2\.(\d+)', tail + chunk):
                best = max(best, int(m.group(1)))
            tail = chunk[-16:]


def measured_glibc_floor():
    # libdrmtap is built on a newer base than the rest of the deb, so the floor is whichever staged
    # object is higher -- and it moves whenever either base does.
    paths = [p for p in glob.glob('tmpdeb/usr/lib/rustdesk/libdrmtap.so.0.*')
             + glob.glob('tmpdeb/usr/share/rustdesk/lib/librustdesk.so')
             + glob.glob('tmpdeb/usr/share/rustdesk/rustdesk')
             if os.path.isfile(p) and not os.path.islink(p)]
    minor = max((_max_glibc_minor(p) for p in paths), default=0)
    if not minor:
        raise Exception(
            f'could not measure a GLIBC_2.x floor from any staged object ({paths or "none found"}); '
            'refusing to ship the unattended-wayland variant with an undeclared libc6 floor, which '
            'is what lets it install on a host where libdrmtap can never load')
    return f'2.{minor}'


def retarget_control_to_drm_variant():
    # Rewrite the control file that generate_control_file just produced, instead of parameterizing that
    # function: the stock packaging path stays exactly as upstream wrote it, and everything specific to
    # this variant lives here. The variant installs the same files as the stock package, so it must
    # conflict with and replace it: you install one or the other, never both. It also needs libdrmtap's
    # own runtime deps, which the stock package has no reason to carry.
    path = '../res/DEBIAN/control'
    floor = measured_glibc_floor()
    print(f'[drm] {DRM_PACKAGE_NAME} libc6 floor measured at {floor}')
    with open(path) as f:
        lines = f.readlines()
    out = []
    for line in lines:
        if line.startswith('Package: rustdesk'):
            out.append(f'Package: {DRM_PACKAGE_NAME}\n')
            out.append('Conflicts: rustdesk\nReplaces: rustdesk\nProvides: rustdesk\n')
        elif line.startswith('Depends:'):
            # 2.4.101 is where drmModeGetFB2 landed; below it libdrmtap loads and can never capture.
            out.append(line.rstrip('\n') + ', libdrm2 (>= 2.4.101), libegl1, libgles2, '
                       f'libc6 (>= {floor})\n')
        else:
            out.append(line)
    body = ''.join(out)
    # Fail loudly rather than silently shipping a package that says `rustdesk`: a stock control file
    # that stopped matching either anchor would otherwise produce a variant deb wearing the stock name.
    if f'Package: {DRM_PACKAGE_NAME}\n' not in body or 'libegl1' not in body:
        raise Exception(f'could not retarget {path} to the drm variant; upstream control layout changed')
    with open(path, 'w') as f:
        f.write(body)


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
    # Bundle libdrmtap.so only when this build actually enabled the `drm` feature, so stock packages
    # stay exactly what they were. The root service dlopens it in-process by absolute path.
    # `features` is the comma-joined string, so split it: a bare substring test would also match any
    # future feature merely containing "drm" (drm-lease, vaapi-drm) and rename the deb to the
    # consent-bypass variant without --drm ever being passed.
    ships_so = 'drm' in features.split(',')
    if ships_so:
        # Same artifact assertion as the --package path. Under --skip-cargo nothing here rebuilt the
        # binary, so `features` says what was ASKED for while the staged bundle can be anything.
        assert_staged_binary_is_drm()
        stage_libdrmtap_into_deb(build_libdrmtap_so())

    system2('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version)
    if ships_so:
        retarget_control_to_drm_variant()
    system2('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    md5_file_folder("tmpdeb/")
    system2('dpkg-deb -b tmpdeb rustdesk.deb;')

    system2('/bin/rm -rf tmpdeb/')
    system2('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('rustdesk.deb', '../rustdesk-%s.deb' % version)
    if ships_so:
        # Named apart from the stock package so installing the consent-free variant is a deliberate act.
        os.rename('../rustdesk-%s.deb' % version, f'../{DRM_PACKAGE_NAME}-{version}.deb')
    os.chdir("..")


DRMTAP_DLOPEN_MARKER = b'/usr/lib/rustdesk/libdrmtap.so.0'
# Present only when `drm-wake` is compiled in: the runtime option constant is itself
# #[cfg(feature = "drm-wake")] (src/ipc/drm.rs). The dlopen marker above cannot stand in for it -
# `--features drm` alone produces a binary that carries the dlopen path and NO wake code, and that
# is exactly the deb this assertion is here to refuse.
DRMTAP_WAKE_MARKER = b'enable-drm-display-wake'


def _carries_drmtap_marker(path, marker=DRMTAP_DLOPEN_MARKER):
    # Chunked, with an overlap of len(marker)-1 so the marker cannot be missed at a chunk boundary:
    # librustdesk.so is ~45 MB and there is no reason to hold it all in memory, and the `with`
    # closes deterministically instead of relying on refcounting.
    with open(path, 'rb') as f:
        tail = b''
        while True:
            chunk = f.read(1 << 20)
            if not chunk:
                return False
            if marker in tail + chunk:
                return True
            tail = chunk[-(len(marker) - 1):]


def assert_staged_binary_is_drm():
    """The staged BINARY must really be a drm build before it is named the unattended-wayland
    variant. That package conflicts with and replaces the stock one, so shipping a stock binary
    under that name produces something that can never capture and cannot be installed alongside
    what it replaced. The marker is the absolute dlopen path from drmtap_dl.rs, present only when
    the feature is compiled in -- assert what was produced, not what was asked for.

    Called from BOTH packaging paths. It used to guard only one of them, and `--skip-cargo` (which
    is how CI packages) reaches the other, where nothing had rebuilt the binary at all.
    """
    binaries = [p for p in glob.glob('tmpdeb/usr/share/rustdesk/lib/librustdesk.so')
                + glob.glob('tmpdeb/usr/share/rustdesk/rustdesk') if os.path.isfile(p)]
    if not any(_carries_drmtap_marker(p) for p in binaries):
        raise Exception(
            f'--drm was requested but the staged bundle does not look like a drm build (no '
            f'{DRMTAP_DLOPEN_MARKER.decode()} dlopen path in {binaries or "any staged binary"}); '
            'refusing to package it as the unattended-wayland variant, which conflicts with and '
            'replaces the stock package but could never capture')
    # And the WAKE half. `--drm` enables `drm-wake` too (see get_features), and the deb is named and
    # documented as the variant that can reach a machine whose screen has gone dark. The dlopen
    # marker above does not distinguish them: `--features drm` alone carries it and has no wake code
    # at all. Asserting only the first half is how a deb can be named for a feature it does not have.
    if not any(_carries_drmtap_marker(p, DRMTAP_WAKE_MARKER) for p in binaries):
        raise Exception(
            f'--drm was requested but the staged binary has no {DRMTAP_WAKE_MARKER.decode()} '
            f'marker in {binaries or "any staged binary"}, so it was built without `drm-wake`; '
            'refusing to package it as the unattended-wayland variant, which is named and '
            'documented as the build that can wake an idle-disabled display. If this fired under '
            '--skip-cargo, the cargo line that produced the bundle is missing the feature: '
            '--features ...,drm,drm-wake')


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
    # Where the capture library comes from for a `--package <folder> --drm` build. Two shapes are
    # supported, because two exist in practice: a bundle that already carries libdrmtap.so.0.*
    # (someone staged it, e.g. a CI artifact), and a plain bundle, which is what every build path
    # here actually produces -- the flutter deb builds the library straight into the staged deb, so
    # nothing ever puts it inside the bundle folder. Demanding it in the bundle made this flag
    # combination impossible to satisfy.
    bundled_glob = glob.glob('tmpdeb/usr/share/rustdesk/libdrmtap.so.0.*')
    bundle_carries_so = any(os.path.isfile(p) and not os.path.islink(p) for p in bundled_glob)
    # The variant must be decided by the EXPLICIT --drm request, not merely by what happens to be
    # staged: a bundle that carries the .so must NOT be shipped as the consent-bypass variant when
    # --drm was never passed.
    if bundle_carries_so and not want_drm:
        raise Exception(
            'the staged bundle carries libdrmtap.so.0.* but --drm was not passed; refusing '
            'to silently ship the consent-bypass unattended-wayland variant (pass --drm to '
            'build it deliberately)')
    if want_drm:
        # Whichever shape we are in, the staged BINARY must really be a drm build. This is the
        # property the old presence-of-the-.so test stood in for, badly: a stock binary packaged as
        # the unattended-wayland variant would carry the consent-bypass name, conflict with and
        # replace the stock package, and never be able to capture. The marker is the absolute
        # dlopen path from drmtap_dl.rs, present only when the feature is compiled in -- the same
        # kind of artifact assertion as _assert_so_has_egl, and for the same reason: assert what
        # was produced, not what was asked for.
        assert_staged_binary_is_drm()
        if bundle_carries_so:
            so = _single_real_so(bundled_glob, 'the staged --drm bundle')
            # The THIRD artifact source, and the last one that was missing the check: --package
            # takes the .so straight out of a bundle somebody else produced, so it has the same
            # exposure as DRMTAP_PREBUILT_DIR (see the comment on that branch). A CPU-only stub
            # would ship, the loader would accept it, and capture would degrade to PipeWire
            # without a word.
            _assert_so_has_egl(so)
            stage_libdrmtap_into_deb(so)
            system2(f'rm -f "{so}"')
            system2('rm -f tmpdeb/usr/share/rustdesk/libdrmtap.so tmpdeb/usr/share/rustdesk/libdrmtap.so.0')
        else:
            # Build it here, exactly as the flutter deb path does (build_libdrmtap_so asserts the
            # EGL backend itself). The library is independent of the staged binary.
            stage_libdrmtap_into_deb(build_libdrmtap_so())

    system2('mkdir -p tmpdeb/DEBIAN')
    generate_control_file(version)
    # Keyed on the EXPLICIT request, not on what happened to be staged: by here a --drm build has
    # its library in tmpdeb whichever of the two shapes it came from.
    if want_drm:
        retarget_control_to_drm_variant()
    system2('cp -a ../res/DEBIAN/* tmpdeb/DEBIAN/')
    md5_file_folder("tmpdeb/")
    system2('dpkg-deb -b tmpdeb rustdesk.deb;')

    system2('/bin/rm -rf tmpdeb/')
    system2('/bin/rm -rf ../res/DEBIAN/control')
    os.rename('rustdesk.deb', '../rustdesk-%s.deb' % version)
    if want_drm:
        os.rename('../rustdesk-%s.deb' % version, f'../{DRM_PACKAGE_NAME}-{version}.deb')
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

    # Before anything with a side effect: this is a query, and a caller uses it to build the very
    # binary it will then package. `get_features` stays the single definition of what a flag
    # combination means; a caller that hardcodes the list instead is one edit away from compiling
    # something other than what it ships.
    if args.print_features:
        # stdout carries the list and nothing else, so a caller can use it directly in a command
        # substitution. `get_features` prints a human-readable line of its own; send that to stderr
        # for this call rather than silencing it, which would change what every other path prints.
        with contextlib.redirect_stdout(sys.stderr):
            feats = ','.join(get_features(args))
        print(feats)
        return

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
