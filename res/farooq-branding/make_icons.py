#!/usr/bin/env python3
"""Farooq Remote icon generator.
Input : App-logo-icon.jpg (colour logo on a fake checkerboard / white), Monochrome-tray.jpg (white on checkerboard)
Output: every icon file the RustDesk 1.4.9 tree uses, written into the repo overlay dir.
Usage : python3 res/farooq-branding/make_icons.py res/farooq-branding .   (run from repo root; regenerates every icon)
Source image: App-logo-icon.jpg lives in Y:\FA-020-farooqremote\1-working\Remote-Branding (not in Git, 1.7 MB); copy it next to this script to regenerate.
"""
import sys, os, base64, io, json
from PIL import Image, ImageFilter

src_dir, repo = sys.argv[1], sys.argv[2]
TEAL = (15, 110, 110)

def out(rel):
    p = os.path.join(repo, rel); os.makedirs(os.path.dirname(p), exist_ok=True); return p

def key_out_background(im, thresh=185):
    """Make every neutral light pixel (white / light-grey checkerboard) transparent.
    Works globally, so checker squares enclosed inside the drawing go too.
    Coloured pixels (teal, amber) and dark pixels are kept."""
    im = im.convert("RGBA"); px = im.load(); w, h = im.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if abs(r-g) < 14 and abs(g-b) < 14 and abs(r-b) < 14 and min(r, g, b) > thresh:
                px[x, y] = (r, g, b, 0)
    return im

def square_trim(im, pad=0.08):
    bbox = im.getchannel("A").getbbox(); im = im.crop(bbox)
    w, h = im.size; s = int(max(w, h) * (1 + 2*pad))
    canvas = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    canvas.paste(im, ((s-w)//2, (s-h)//2)); return canvas

def save_png(im, size, rel):
    im.resize((size, size), Image.LANCZOS).save(out(rel), "PNG")

logo = square_trim(key_out_background(Image.open(os.path.join(src_dir, "App-logo-icon.jpg"))))
# tray silhouette = the logo's own shape (the JPG tray file cannot be keyed: white on white/grey checker)
tray = logo
a = tray.getchannel("A"); tray_white = Image.new("RGBA", tray.size, (255, 255, 255, 255)); tray_white.putalpha(a)
tray_black = Image.new("RGBA", tray.size, (0, 0, 0, 255)); tray_black.putalpha(a)

# --- res/ (Rust side, Linux packages, tray) ---
save_png(logo, 1024, "res/icon.png")
for s in (32, 64, 128): save_png(logo, s, f"res/{s}x{s}.png")
save_png(logo, 256, "res/128x128@2x.png")
save_png(logo, 1024, "res/mac-icon.png")
save_png(tray_black, 44, "res/mac-tray-dark-x2.png")   # macOS template icons: shape only
save_png(tray_black, 44, "res/mac-tray-light-x2.png")
logo.resize((256, 256), Image.LANCZOS).save(out("res/icon.ico"), sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])
tray_white.resize((32, 32), Image.LANCZOS).save(out("res/tray-icon.ico"), sizes=[(16,16),(32,32)])
# Windows runner icon
logo.resize((256, 256), Image.LANCZOS).save(out("flutter/windows/runner/resources/app_icon.ico"), sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])

# --- Flutter in-app logo: SVG wrapping the PNG (flutter_svg renders embedded rasters) ---
buf = io.BytesIO(); logo.resize((512, 512), Image.LANCZOS).save(buf, "PNG")
b64 = base64.b64encode(buf.getvalue()).decode()
svg = f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="512" height="512" viewBox="0 0 512 512"><image width="512" height="512" xlink:href="data:image/png;base64,{b64}"/></svg>'
open(out("flutter/assets/icon.svg"), "w").write(svg)
open(out("res/scalable.svg"), "w").write(svg)
open(out("res/logo.svg"), "w").write(svg)

# --- Android launcher (legacy + adaptive) ---
dens = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
for d, s in dens.items():
    save_png(logo, s, f"flutter/android/app/src/main/res/mipmap-{d}/ic_launcher.png")
    # round: mask to circle
    im = logo.resize((s, s), Image.LANCZOS); mask = Image.new("L", (s, s), 0)
    from PIL import ImageDraw; ImageDraw.Draw(mask).ellipse((0, 0, s-1, s-1), fill=255)
    bg = Image.new("RGBA", (s, s), (255, 255, 255, 255)); bg.alpha_composite(im); bg.putalpha(mask)
    bg.save(out(f"flutter/android/app/src/main/res/mipmap-{d}/ic_launcher_round.png"))
    # adaptive foreground: 108dp canvas, logo inside the 72dp safe zone
    fs = int(s * 108 / 48); inner = int(fs * 0.62)
    fg = Image.new("RGBA", (fs, fs), (0, 0, 0, 0)); l = logo.resize((inner, inner), Image.LANCZOS)
    fg.paste(l, ((fs-inner)//2, (fs-inner)//2), l); fg.save(out(f"flutter/android/app/src/main/res/mipmap-{d}/ic_launcher_foreground.png"))
open(out("flutter/android/app/src/main/res/values/ic_launcher_background.xml"), "w").write(
    '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <color name="ic_launcher_background">#FFFFFF</color>\n</resources>\n')

# --- macOS AppIcon.appiconset ---
# upstream ships a single flutter/macos/Runner/AppIcon.icns; Pillow can write ICNS
logo.resize((1024, 1024), Image.LANCZOS).save(out("flutter/macos/Runner/AppIcon.icns"), "ICNS")

# --- iOS AppIcon.appiconset (names from upstream Contents.json) ---
ios = "flutter/ios/Runner/Assets.xcassets/AppIcon.appiconset/"
cj = json.load(open(os.path.join(repo, ios, "Contents.json")))
for img in cj["images"]:
    base = float(img["size"].split("x")[0]); scale = int(img["scale"][0]); px = int(round(base*scale))
    # iOS icons must be opaque: flatten on white
    flat = Image.new("RGBA", logo.size, (255, 255, 255, 255)); flat.alpha_composite(logo)
    flat.convert("RGB").resize((px, px), Image.LANCZOS).save(out(ios + img["filename"]), "PNG")
print("icons written")
