#!/usr/bin/env python3
"""Render the Snapforge "aperture" iconset at all macOS sizes.

The dock/app icon mirrors the menu-bar tray glyph (TrayIcon::makeIdleIcon):
an aperture-style six-blade hex ring inside a rounded viewfinder frame, with a
small shutter notch in the top-right corner — drawn white on the same dark
rounded-square background the brand already uses. Keeping both marks identical
means the app icon and tray icon finally read as the same product.

The glyph geometry is a 1:1 port of makeIdleIcon's 18-unit layout, scaled into
a centred box on the 1024 reference canvas so the tray frame becomes the app
icon's inner viewfinder outline.

Outputs qt/resources/AppIcon.iconset/icon_*.png at every size iconutil wants
(16, 32, 128, 256, 512 plus @2x). Run with --preview <path> to emit a single
512px PNG for design review instead.
"""

from __future__ import annotations
import argparse
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = REPO / "qt" / "resources" / "AppIcon.iconset"

SIZES = [
    (16,   "icon_16x16.png"),
    (32,   "icon_16x16@2x.png"),
    (32,   "icon_32x32.png"),
    (64,   "icon_32x32@2x.png"),
    (128,  "icon_128x128.png"),
    (256,  "icon_128x128@2x.png"),
    (256,  "icon_256x256.png"),
    (512,  "icon_256x256@2x.png"),
    (512,  "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
]

REF = 1024  # design reference resolution

# --- aperture glyph layout, in the tray's 18-unit logical space -------------
LOGICAL = 18.0
FRAME_MARGIN = 1.5
FRAME_RADIUS = 3.5
STROKE = 1.2          # tray pen width, in logical units
HEX_R = 4.6
PUPIL = 1.4
NOTCH = 2.4
NOTCH_RADIUS = 0.6

# Centred glyph box on the REF canvas (leaves an even margin around the mark).
GLYPH_BOX = 660.0
GLYPH_ORIGIN = (REF - GLYPH_BOX) / 2.0
U = GLYPH_BOX / LOGICAL            # logical unit -> REF px

INK = (255, 255, 255, 255)
RED = (255, 59, 48, 255)


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def make_background(size: int) -> Image.Image:
    """Rounded-square dark diagonal gradient (macOS Big Sur+ shape)."""
    grad = Image.new("RGBA", (size, size))
    px = grad.load()
    top = (38, 38, 44)   # #26262c
    bot = (10, 10, 14)   # #0a0a0e
    denom = max(1, 2 * (size - 1))
    for y in range(size):
        for x in range(size):
            t = (x + y) / denom
            r, g, b = lerp(top, bot, t)
            px[x, y] = (r, g, b, 255)

    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, size - 1, size - 1),
        radius=round(size * 0.2237),
        fill=255,
    )
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    return out


def render(size: int, notch_color=RED, glow=True) -> Image.Image:
    """Compose background + aperture glyph for `size` px output."""
    bg = make_background(size)

    # Render foreground at 2x then downsample for crisp anti-aliasing.
    super_size = size * 2
    over = Image.new("RGBA", (super_size, super_size), (0, 0, 0, 0))
    od = ImageDraw.Draw(over)
    scale = super_size / REF          # REF px -> super-canvas px

    def X(v):  # logical unit -> super-canvas px
        return (GLYPH_ORIGIN + v * U) * scale

    sw = max(2.0, STROKE * U * scale)
    cap_r = sw / 2.0

    def line(x0, y0, x1, y1):
        od.line([(x0, y0), (x1, y1)], fill=INK, width=int(round(sw)), joint="curve")
        od.ellipse((x0 - cap_r, y0 - cap_r, x0 + cap_r, y0 + cap_r), fill=INK)
        od.ellipse((x1 - cap_r, y1 - cap_r, x1 + cap_r, y1 + cap_r), fill=INK)

    # Outer rounded viewfinder frame.
    fm = X(FRAME_MARGIN)
    fe = X(LOGICAL - FRAME_MARGIN)
    od.rounded_rectangle((fm, fm, fe, fe), radius=(FRAME_RADIUS * U * scale),
                         outline=INK, width=int(round(sw)))

    # Aperture: tilted hexagon ring (flat edge at top -> reads as an iris).
    cx = X(LOGICAL / 2.0)
    cy = X(LOGICAL / 2.0)
    rpx = HEX_R * U * scale
    hexpts = []
    for i in range(6):
        a = (math.pi / 3.0) * i + (math.pi / 6.0)
        hexpts.append((cx + rpx * math.cos(a), cy + rpx * math.sin(a)))
    od.polygon(hexpts, outline=INK, width=int(round(sw)))
    # Round the hex corners (polygon joins are mitred otherwise).
    for (px_, py_) in hexpts:
        od.ellipse((px_ - cap_r, py_ - cap_r, px_ + cap_r, py_ + cap_r), fill=INK)

    # Three blade ticks: alternating hex vertices toward centre, stopping short
    # of the pupil so the iris stays open.
    pupil_px = PUPIL * U * scale
    for i in range(0, 6, 2):
        vx, vy = hexpts[i]
        dx, dy = (cx - vx), (cy - vy)
        ln = math.hypot(dx, dy)
        if ln <= pupil_px:
            continue
        t = (ln - pupil_px) / ln
        line(vx, vy, vx + dx * t, vy + dy * t)

    # Shutter notch: small filled rounded square, top-right of the frame.
    nx0 = X(LOGICAL - 4.6)
    ny0 = X(FRAME_MARGIN - 0.2)
    nx1 = nx0 + NOTCH * U * scale
    ny1 = ny0 + NOTCH * U * scale

    if glow and notch_color == RED:
        halo = Image.new("RGBA", (super_size, super_size), (0, 0, 0, 0))
        hd = ImageDraw.Draw(halo)
        ncx, ncy = (nx0 + nx1) / 2.0, (ny0 + ny1) / 2.0
        hr = (nx1 - nx0) * 1.6
        hd.ellipse((ncx - hr, ncy - hr, ncx + hr, ncy + hr), fill=(255, 59, 48, 120))
        halo = halo.filter(ImageFilter.GaussianBlur(radius=18 * scale))
        over = Image.alpha_composite(over, halo)
        od = ImageDraw.Draw(over)

    od.rounded_rectangle((nx0, ny0, nx1, ny1),
                         radius=NOTCH_RADIUS * U * scale, fill=notch_color)

    over_small = over.resize((size, size), Image.LANCZOS)
    return Image.alpha_composite(bg, over_small)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview", help="write a single 512px PNG here and exit")
    ap.add_argument("--notch", choices=["red", "white"], default="red",
                    help="shutter-notch accent colour")
    args = ap.parse_args()
    notch = RED if args.notch == "red" else INK

    if args.preview:
        render(512, notch_color=notch).save(args.preview, "PNG", optimize=True)
        print(f"wrote preview {args.preview} (notch={args.notch})")
        return

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    cache = {}
    for size, fname in SIZES:
        if size not in cache:
            cache[size] = render(size, notch_color=notch)
        out_path = OUT_DIR / fname
        cache[size].save(out_path, "PNG", optimize=True)
        print(f"  wrote {out_path.relative_to(REPO)} ({size}x{size})")
    print(f"\n{len(SIZES)} icons written.")


if __name__ == "__main__":
    main()
