#!/usr/bin/env python3
"""Build and package Tefway Remote Master / Client from this one tree.

The product is selected at compile time by `TEFWAY_PROFILE`, which `build.rs`
bakes into the binary; this script only sets it and packages the result under the
matching product identity. RustDesk stays the upstream base: `build.py` is reused
for compiling, and nothing here changes the stock packaging paths.
"""

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TEFWAY = REPO / "res" / "tefway"
PKG_DIR = TEFWAY / "pkg"
OUT = REPO / "build" / "tefway"
BUNDLE_LINUX = REPO / "flutter" / "build" / "linux" / "x64" / "release" / "bundle"
BUNDLE_WINDOWS = REPO / "flutter" / "build" / "windows" / "x64" / "runner" / "Release"
ICON_SIZES = [16, 32, 48, 64, 128, 256, 512]

PROFILES = {
    "master": {
        "pkg": "tefway-remote-master",
        "name": "Tefway Remote Master",
        "exe": "TefwayRemoteMaster",
        "description": "Tecnico: inicia e controla conexoes remotas. Nao recebe sessoes.",
    },
    "client": {
        "pkg": "tefway-remote-client",
        "name": "Tefway Remote Client",
        "exe": "TefwayRemoteClient",
        "description": "Cliente: recebe solicitacoes e autoriza cada conexao manualmente.",
    },
}


def run(cmd, cwd=None, env=None):
    printable = " ".join(str(c) for c in cmd)
    print(f"+ {printable}")
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def version():
    for line in (REPO / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("version not found in Cargo.toml")


def profile_env(profile):
    env = dict(os.environ)
    env["TEFWAY_PROFILE"] = profile
    return env


def build(profile, features=(), skip_cargo=False):
    """Compile the profile.

    The two cargo/flutter commands are the same ones `build.py --flutter`
    issues on its Debian path; going through them directly keeps the build
    identical on Fedora, Debian and Windows, where `build.py`'s Linux branches
    end in a distro-specific package step this script replaces.
    """
    env = profile_env(profile)
    if os.name == "nt":
        run(
            ["cargo", "build", "--locked", "--release"],
            cwd=REPO / "libs" / "virtual_display" / "dylib",
            env=env,
        )
    if not skip_cargo:
        feats = ",".join(("flutter",) + tuple(features))
        run(
            ["cargo", "build", "--locked", "--features", feats, "--lib", "--release"],
            cwd=REPO,
            env=env,
        )
    run(
        ["flutter", "build", "linux" if os.name != "nt" else "windows", "--release"],
        cwd=REPO / "flutter",
        env=env,
    )
    if os.name == "nt":
        shutil.copyfile(
            REPO / "target" / "release" / "deps" / "dylib_virtual_display.dll",
            BUNDLE_WINDOWS,
        )


def render(template, mapping):
    text = (PKG_DIR / template).read_text(encoding="utf-8")
    for key, value in mapping.items():
        text = text.replace(f"@{key}@", value)
    return text


def mapping_for(profile, pkg_version):
    info = PROFILES[profile]
    return {
        "PKG": info["pkg"],
        "NAME": info["name"],
        "BIN": info["exe"],
        "VERSION": pkg_version,
        "DESCRIPTION": info["description"],
        "ARCH": "amd64",
        "SIZE": "0",
    }


def stage(profile, pkg_version):
    """Collect the bundle, icons, desktop entry and unit for this profile."""
    info = PROFILES[profile]
    stage_dir = OUT / profile / "stage"
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    (stage_dir / "files").mkdir(parents=True)
    (stage_dir / "icons").mkdir()

    if os.name == "nt":
        source = BUNDLE_WINDOWS
    else:
        source = BUNDLE_LINUX
    if not source.is_dir():
        raise SystemExit(
            f"bundle not found at {source}; run with --build (or run build.py --flutter first)"
        )
    shutil.copytree(source, stage_dir / "bundle")

    # Distinct executable name so the process, window class and package never
    # collide with a stock RustDesk installation on the same machine.
    bundle = stage_dir / "bundle"
    for candidate in ("rustdesk.exe", "rustdesk"):
        src = bundle / candidate
        if src.is_file():
            target = bundle / (info["exe"] + (".exe" if candidate.endswith(".exe") else ""))
            src.rename(target)

    values = mapping_for(profile, pkg_version)
    for template, target in (
        ("tefway-remote.desktop.in", f'{info["pkg"]}.desktop'),
        ("tefway-remote.service.in", f'{info["pkg"]}.service'),
    ):
        (stage_dir / "files" / target).write_text(render(template, values), encoding="utf-8")

    for size in ICON_SIZES:
        shutil.copyfile(TEFWAY / f"icon-{size}.png", stage_dir / "icons" / f"icon-{size}.png")
    return stage_dir


def package_deb(profile, pkg_version, stage_dir):
    info = PROFILES[profile]
    tree = OUT / profile / "deb"
    if tree.exists():
        shutil.rmtree(tree)
    root = tree / "root"
    share = root / "usr" / "share" / info["pkg"]
    share.parent.mkdir(parents=True)
    shutil.copytree(stage_dir / "bundle", share)
    (share / "files").mkdir(exist_ok=True)
    for name in (f'{info["pkg"]}.desktop', f'{info["pkg"]}.service'):
        shutil.copyfile(stage_dir / "files" / name, share / "files" / name)
    for size in ICON_SIZES:
        icon_dir = root / "usr" / "share" / "icons" / "hicolor" / f"{size}x{size}" / "apps"
        icon_dir.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(
            stage_dir / "icons" / f"icon-{size}.png", icon_dir / f'{info["pkg"]}.png'
        )
    (root / "usr" / "bin").mkdir(parents=True, exist_ok=True)
    os.symlink(
        f'/usr/share/{info["pkg"]}/{info["exe"]}',
        root / "usr" / "bin" / info["exe"],
    )

    control_dir = tree / "control"
    control_dir.mkdir()
    values = mapping_for(profile, pkg_version)
    values["SIZE"] = str(sum(f.stat().st_size for f in share.rglob("*") if f.is_file()) // 1024)
    (control_dir / "control").write_text(render("control.in", values), encoding="utf-8")
    for script, mode in (
        ("postinst.in", "postinst"),
        ("prerm.in", "prerm"),
        ("postrm.in", "postrm"),
    ):
        path = control_dir / mode
        path.write_text(render(script, values), encoding="utf-8")
        path.chmod(0o755)
    (control_dir / "md5sums").write_text("", encoding="utf-8")

    deb = OUT / profile / f'{info["pkg"]}_{pkg_version}_amd64.deb'
    run(
        [
            "dpkg-deb",
            "--root-owner-group",
            "-b",
            str(control_dir),
            str(deb),
        ]
    )
    # dpkg-deb -b wants the payload next to DEBIAN/ inside one directory.
    print(f"deb: {deb}")
    return deb


def package_rpm(profile, pkg_version, stage_dir):
    info = PROFILES[profile]
    topdir = OUT / profile / "rpm"
    for sub in ("BUILD", "RPMS", "SOURCES", "SPECS", "SRPMS"):
        (topdir / sub).mkdir(parents=True, exist_ok=True)
    spec = topdir / "SPECS" / f'{info["pkg"]}.spec'
    spec.write_text(render("tefway-remote.spec.in", mapping_for(profile, pkg_version)), encoding="utf-8")
    run(
        [
            "rpmbuild",
            "-bb",
            "--define",
            f"_topdir {topdir}",
            "--define",
            f"tefway_stage {stage_dir}",
            str(spec),
        ]
    )
    built = list((topdir / "RPMS").rglob("*.rpm"))
    print("rpm: " + ", ".join(str(p) for p in built))
    return built


def package_appimage(profile, pkg_version, deb_path):
    info = PROFILES[profile]
    recipe_dir = OUT / profile / "appimage"
    recipe_dir.mkdir(parents=True, exist_ok=True)
    recipe = recipe_dir / "AppImageBuilder.yml"
    recipe.write_text(
        render(
            "AppImageBuilder.yml.in",
            {
                "PKG": info["pkg"],
                "NAME": info["name"],
                "BIN": info["exe"],
                "VERSION": pkg_version,
                "DEB": str(deb_path.resolve()),
            },
        ),
        encoding="utf-8",
    )
    run(["appimage-builder", "--recipe", str(recipe)], cwd=recipe_dir)
    produced = list(recipe_dir.glob("*.AppImage"))
    if not produced:
        raise SystemExit("appimage-builder finished without producing an AppImage")
    target = OUT / profile / f'{info["exe"]}-x86_64.AppImage'
    produced[0].rename(target)
    print(f"appimage: {target}")
    return target


def package_windows(profile, stage_dir):
    info = PROFILES[profile]
    target = OUT / profile / info["exe"]
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(stage_dir / "bundle", target)
    print(f"portable windows tree: {target}")
    print(f"executable: {target / (info['exe'] + '.exe')}")
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, choices=sorted(PROFILES))
    parser.add_argument("--format", required=True, choices=["deb", "rpm", "appimage", "windows"])
    parser.add_argument("--build", action="store_true", help="compile before packaging")
    parser.add_argument(
        "--skip-cargo",
        action="store_true",
        help="reuse an existing librustdesk.so and only run the Flutter build",
    )
    parser.add_argument(
        "--with-hwcodec", action="store_true", help="enable the hwcodec cargo feature"
    )
    parser.add_argument(
        "--with-unix-file-copy-paste",
        action="store_true",
        help="enable the unix-file-copy-paste cargo feature",
    )
    args = parser.parse_args()

    pkg_version = version()
    features = []
    if args.with_hwcodec:
        features.append("hwcodec")
    if args.with_unix_file_copy_paste:
        features.append("unix-file-copy-paste")
    if args.build:
        build(args.profile, features, args.skip_cargo)
    stage_dir = stage(args.profile, pkg_version)

    if args.format == "windows":
        package_windows(args.profile, stage_dir)
    elif args.format == "deb":
        package_deb(args.profile, pkg_version, stage_dir)
    elif args.format == "rpm":
        package_rpm(args.profile, pkg_version, stage_dir)
    else:
        package_appimage(args.profile, pkg_version, package_deb(args.profile, pkg_version, stage_dir))


if __name__ == "__main__":
    main()
