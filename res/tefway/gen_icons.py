#!/usr/bin/env python3
"""Derive every Tefway icon from the owner-supplied assets.

`assets/favicon.ico` is the official square mark; `assets/logo.svg` is the
wordmark used in the UI and is copied verbatim, never rewritten. Neither source
file is modified by this script.

Windows needs a multi-size ICO: the supplied ICO carries a single 128x128 frame,
which Windows would stretch for the title bar, taskbar and Alt+Tab. The ICOs
written here add the small frames Windows asks for.

Sizes above the 128px source are upscaled with Lanczos and are marked as derived
in docs/build/TEFWAY-REMOTE-WINDOWS.md: a vector square mark would render them
sharper, and replacing them is a drop-in once the owner supplies one.
"""

import os
import shutil

from PIL import Image

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ASSETS = os.path.join(REPO, "assets")
OUT = os.path.join(REPO, "res", "tefway")

LINUX_SIZES = [16, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

DARK_THEME_FILLS = {
    "#164194": "#8FB3EE",
    "#144677": "#A9C4EC",
    "#008bd2": "#4FC3F7",
}


def load_master():
    source = os.path.join(ASSETS, "favicon.ico")
    image = Image.open(source)
    sizes = image.info.get("sizes") or {image.size}
    best = max(sizes, key=lambda s: s[0] * s[1])
    image.size = best
    return image.convert("RGBA")


def resize(master, size):
    return master.resize((size, size), Image.LANCZOS)


def main():
    os.makedirs(OUT, exist_ok=True)
    master = load_master()
    print(f"source: assets/favicon.ico, best frame {master.size}")

    for size in LINUX_SIZES:
        path = os.path.join(OUT, f"icon-{size}.png")
        resize(master, size).save(path)
        note = " (upscaled)" if size > master.size[0] else ""
        print(f"{os.path.relpath(path, REPO)}{note}")

    ico = os.path.join(OUT, "tefway.ico")
    master.save(ico, sizes=[(s, s) for s in ICO_SIZES])
    print(os.path.relpath(ico, REPO))

    # Flutter assets. The light wordmark is the official file byte for byte.
    flutter_assets = os.path.join(REPO, "flutter", "assets")
    shutil.copyfile(
        os.path.join(ASSETS, "logo.svg"),
        os.path.join(flutter_assets, "tefway_logo.svg"),
    )
    # Dark-theme variant: same artwork, the two brand blues lifted so the
    # wordmark stays legible on a dark background. No geometry changes.
    with open(os.path.join(ASSETS, "logo.svg"), encoding="utf-8") as fh:
        svg = fh.read()
    for dark, light in DARK_THEME_FILLS.items():
        svg = svg.replace(dark, light)
    with open(os.path.join(flutter_assets, "tefway_logo_dark.svg"), "w", encoding="utf-8") as fh:
        fh.write(svg)
    print("-> flutter/assets/tefway_logo.svg, flutter/assets/tefway_logo_dark.svg")

    targets = {
        # Rust binary resource (build.rs winres) and MSI/installer resources.
        os.path.join(REPO, "res", "icon.ico"): ico,
        # Tray icon.
        os.path.join(REPO, "res", "tray-icon.ico"): ico,
        # Flutter Windows runner window/exe icon (Runner.rc).
        os.path.join(
            REPO, "flutter", "windows", "runner", "resources", "app_icon.ico"
        ): ico,
        # Raster icons consumed by the Linux and macOS packaging paths.
        os.path.join(REPO, "res", "32x32.png"): os.path.join(OUT, "icon-32.png"),
        os.path.join(REPO, "res", "64x64.png"): os.path.join(OUT, "icon-64.png"),
        os.path.join(REPO, "res", "128x128.png"): os.path.join(OUT, "icon-128.png"),
        os.path.join(REPO, "res", "128x128@2x.png"): os.path.join(OUT, "icon-256.png"),
    }
    for target, source in targets.items():
        shutil.copyfile(source, target)
        print(f"-> {os.path.relpath(target, REPO)}")


if __name__ == "__main__":
    main()
