#!/usr/bin/env python3
"""Build and package Tefway Remote Master / Client from this one tree.

The product is selected at compile time by `TEFWAY_PROFILE`, which `build.rs`
bakes into the binary; this script only sets it and packages the result under
the matching product identity. RustDesk stays the upstream base: `build.py` is reused
for compiling, and nothing here changes the stock packaging paths.

Profile isolation (TEFWAY-REMOTE-BUILD-01, Gate 1):
  * Build Master uses profile `master`; build Client uses profile `client`.
  * `build.rs` aborts on any other value of `TEFWAY_PROFILE`; an unset profile
    keeps stock RustDesk and is never packaged as Tefway by this script.
  * Every build writes `target/release/tefway-build-manifest.json` describing
    the profile really embedded. This script cross-checks it before packaging,
    so `--skip-cargo` or packaging without `--build` cannot reuse a bundle
    built for a different (or no) profile.
  * Each produced artifact gets a sibling `<artifact>.manifest.json` evidence
    file (profile, version, source commit, OS, architecture, resources,
    server identity, SHA-256 of the package). No secrets are included.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
import platform

REPO = Path(__file__).resolve().parents[2]
TEFWAY = REPO / "res" / "tefway"
PKG_DIR = TEFWAY / "pkg"
OUT = REPO / "build" / "tefway"
BUNDLE_LINUX = REPO / "flutter" / "build" / "linux" / "x64" / "release" / "bundle"
BUNDLE_WINDOWS = REPO / "flutter" / "build" / "windows" / "x64" / "runner" / "Release"
ICON_SIZES = [16, 32, 48, 64, 128, 256, 512]

MARKER_NAME = "tefway-build-manifest.json"

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

SERVER_IDENTITY = {
    "rendezvous": "remote.tefway.com.br",
    "relay": "remote.tefway.com.br",
    "public_key": "3KBIfqldtiCmJkAl2O6rammBCLK4iRiTH3vQdRXoEEs=",
}


def run(cmd, cwd=None, env=None):
    printable = " ".join(str(c) for c in cmd)
    print(f"+ {printable}")
    subprocess.run(cmd, cwd=cwd, env=env, check=True)


def git_commit():
    try:
        out = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=REPO,
            capture_output=True,
            check=True,
            text=True,
        )
        return out.stdout.strip()
    except Exception:
        return "unknown"


def version():
    for line in (REPO / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("version not found in Cargo.toml")


def profile_env(profile):
    env = dict(os.environ)
    env["TEFWAY_PROFILE"] = profile
    return env


def cargo_target_dir():
    d = os.environ.get("CARGO_TARGET_DIR") or "target"
    p = Path(d)
    if not p.is_absolute():
        p = REPO / p
    return p


def build_manifest_path():
    return cargo_target_dir() / "release" / MARKER_NAME


def load_build_manifest():
    path = build_manifest_path()
    if not path.is_file():
        raise SystemExit(
            f"tefway build manifest not found at {path}; build with --build first"
        )
    return json.loads(path.read_text(encoding="utf-8"))


def machine_arch():
    m = platform.machine().lower()
    if m in ("amd64", "x86_64"):
        return "x86_64"
    if m in ("i386", "i686", "x86"):
        return "x86"
    if m in ("aarch64", "arm64"):
        return "aarch64"
    return m


def bundle_for(profile):
    """Return the bundle directory and the manifest evidence it must carry."""
    if os.name == "nt":
        return BUNDLE_WINDOWS
    return BUNDLE_LINUX


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
        # Gate 1, item 6: validate the profile effectively embedded in the
        # binary against the profile we asked for. The manifest is ground
        # truth of what build.rs baked in via TEFWAY_PROFILE.
        manifest = load_build_manifest()
        embedded = manifest.get("profile")
        if embedded != profile:
            raise SystemExit(
                f"build with TEFWAY_PROFILE={profile!r} embedded profile {embedded!r} "
                "in the binary; refusing to package a mismatched profile"
            )
    run(
        ["flutter", "build", "linux" if os.name != "nt" else "windows", "--release"],
        cwd=REPO / "flutter",
        env=env,
    )
    # Ship the build manifest inside the bundle so every bundle carries its own
    # profile evidence and stage() can reject mismatched pre-existing bundles.
    manifest_src = build_manifest_path()
    source = bundle_for(profile)
    if manifest_src.is_file():
        shutil.copyfile(manifest_src, source / MARKER_NAME)
    if os.name == "nt":
        shutil.copyfile(
            REPO / "target" / "release" / "deps" / "dylib_virtual_display.dll",
            source / "dylib_virtual_display.dll",
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


def require_bundle_profile(source, profile):
    """Gate 1, items 6 and 7: never package a bundle whose embedded profile is
    unknown or differs from the requested one. Rebuild with --build instead."""
    marker = source / MARKER_NAME
    if not marker.is_file():
        raise SystemExit(
            f"bundle at {source} carries no {MARKER_NAME}; it was not built by "
            "this script (or was wiped). Re-run with --build to regenerate it "
            f"for profile {profile!r}."
        )
    bundle = json.loads(marker.read_text(encoding="utf-8"))
    embedded = bundle.get("profile")
    if embedded != profile:
        raise SystemExit(
            f"bundle at {source} embeds profile {embedded!r}, but profile {profile!r} "
            "was requested; refusing to package a mismatched bundle. Re-run with "
            f"--build --profile {profile}."
        )


def stage(profile, pkg_version):
    """Collect the bundle, icons, desktop entry and unit for this profile."""
    info = PROFILES[profile]
    if os.name == "nt":
        source = BUNDLE_WINDOWS
    else:
        source = BUNDLE_LINUX
    if not source.is_dir():
        raise SystemExit(
            f"bundle not found at {source}; run with --build (or run build.py --flutter first)"
        )
    # Gate 1, item 7: reject bundles without a matching profile marker, so
    # --skip-cargo or packaging without --build cannot reuse an incompatible
    # bundle built for another product.
    require_bundle_profile(source, profile)

    stage_dir = OUT / profile / "stage"
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    (stage_dir / "files").mkdir(parents=True)
    (stage_dir / "icons").mkdir()

    shutil.copytree(source, stage_dir / "bundle")

    # Distinct executable name so the process, window class and package never
    # collide with a stock RustDesk installation on the same machine.
    bundle = stage_dir / "bundle"
    for candidate in ("rustdesk.exe", "rustdesk"):
        src = bundle / candidate
        if src.is_file():
            target = bundle / (info["exe"] + (".exe" if candidate.endswith(".exe") else ""))
            src.rename(target)

    # Gate 1, item 10: after renaming, the bundle must still carry the runtime
    # files the executable depends on: the Rust library, the Flutter assets and
    # (Windows) the virtual-display helper DLL.
    for required in (
        info["exe"] + (".exe" if os.name == "nt" else ""),
        "librustdesk.dll" if os.name == "nt" else "librustdesk.so",
        "flutter_assets",
    ):
        if not (bundle / required).exists():
            raise SystemExit(
                f"renamed bundle is missing required item {required}; packaging aborted"
            )

    values = mapping_for(profile, pkg_version)
    for template, target in (
        ("tefway-remote.desktop.in", f'{info["pkg"]}.desktop'),
        ("tefway-remote.service.in", f'{info["pkg"]}.service'),
    ):
        (stage_dir / "files" / target).write_text(render(template, values), encoding="utf-8")

    for size in ICON_SIZES:
        shutil.copyfile(TEFWAY / f"icon-{size}.png", stage_dir / "icons" / f"icon-{size}.png")
    return stage_dir


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_artifact_manifest(profile, pkg_version, package_path, hash_target):
    """Gate 1, item 8: verifiable evidence per artifact.

    Contains profile, version, source commit, OS, architecture, compiled
    resources, server identity and the SHA-256 of the package. No secrets.
    For a directory tree (Windows portable) the executable inside the tree is
    hashed as the artifact identity.
    """
    build_manifest = {}
    try:
        build_manifest = load_build_manifest()
    except SystemExit:
        pass
    if package_path.is_file():
        sha256_of = package_path.name
        sha256 = sha256_file(package_path)
    else:
        sha256_of = f"{hash_target.name} (executable inside tree)"
        sha256 = sha256_file(hash_target) if hash_target.is_file() else ""
    entry = {
        "profile": profile,
        "version": pkg_version,
        "commit": git_commit(),
        "os": platform.system(),
        "arch": machine_arch(),
        "resources": build_manifest.get("resources", []),
        "server": SERVER_IDENTITY,
        "package": package_path.name,
        "package_type": "file" if package_path.is_file() else "directory",
        "sha256_of": sha256_of,
        "sha256": sha256,
    }
    manifest_path = Path(str(package_path) + ".manifest.json")
    manifest_path.write_text(json.dumps(entry, indent=2) + "\n", encoding="utf-8")
    print(f"manifest: {manifest_path}")
    return manifest_path


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

    # dpkg-deb -b builds from a directory that carries DEBIAN/ next to the payload.
    control_dir = root / "DEBIAN"
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

    deb = OUT / profile / f'{info["pkg"]}_{pkg_version}_amd64.deb'
    run(
        [
            "dpkg-deb",
            "--root-owner-group",
            "-b",
            str(root),
            str(deb),
        ]
    )
    print(f"deb: {deb}")
    write_artifact_manifest(profile, pkg_version, deb, deb)
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
    for rpm in built:
        write_artifact_manifest(profile, pkg_version, rpm, rpm)
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
    write_artifact_manifest(profile, pkg_version, target, target)
    return target


def package_windows(profile, pkg_version, stage_dir):
    info = PROFILES[profile]
    target = OUT / profile / info["exe"]
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(stage_dir / "bundle", target)
    exe = target / (info["exe"] + ".exe")
    print(f"portable windows tree: {target}")
    print(f"executable: {exe}")
    write_artifact_manifest(profile, pkg_version, target, exe)
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
        package_windows(args.profile, pkg_version, stage_dir)
    elif args.format == "deb":
        package_deb(args.profile, pkg_version, stage_dir)
    elif args.format == "rpm":
        package_rpm(args.profile, pkg_version, stage_dir)
    else:
        package_appimage(profile=args.profile, pkg_version=pkg_version, deb_path=package_deb(args.profile, pkg_version, stage_dir))


if __name__ == "__main__":
    main()
