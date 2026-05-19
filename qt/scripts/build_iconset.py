#!/usr/bin/env python3
"""Render Snapforge "Snap brackets" iconset at all macOS sizes.

Design (option 02 from docs/app-icon-mocks.html):
- Dark gradient background, macOS rounded-square shape (no border)
- White corner brackets: top-left and bottom-right
- White diagonal slash from top-right to bottom-left
- Red record dot tucked into the top-right corner

Outputs to qt/resources/AppIcon.iconset/icon_*.png at every size that
iconutil expects (16, 32, 128, 256, 512 plus @2x for each).
"""

from __future__ import annotations
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = REPO / "qt" / "resources" / "AppIcon.iconset"
OUT_DIR.mkdir(parents=True, exist_ok=True)

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


def render(size: int) -> Image.Image:
    """Compose background + foreground glyph for `size` px output."""
    bg = make_background(size)

    # Render foreground at 2× then downsample for crisp anti-aliasing.
    super_size = size * 2
    over = Image.new("RGBA", (super_size, super_size), (0, 0, 0, 0))
    od = ImageDraw.Draw(over)
    s = super_size / REF  # scale: design coord → super-canvas px

    # Reference (1024 canvas) measurements.
    margin = 256
    arm = 220
    stroke = 64
    slash_inset = 160

    def sx(v):
        return v * s

    sw = max(2.0, sx(stroke))
    cap_r = sw / 2.0
    canvas = super_size

    ink = (255, 255, 255, 255)

    def line(x0, y0, x1, y1):
        od.line([(x0, y0), (x1, y1)], fill=ink, width=int(sw), joint="curve")
        # Round both ends with circles so caps look smooth at small sizes.
        od.ellipse((x0 - cap_r, y0 - cap_r, x0 + cap_r, y0 + cap_r), fill=ink)
        od.ellipse((x1 - cap_r, y1 - cap_r, x1 + cap_r, y1 + cap_r), fill=ink)

    m = sx(margin)
    L = sx(arm)

    # Top-left bracket ┌
    line(m, m + L, m, m)
    line(m, m, m + L, m)

    # Bottom-right bracket ┘
    br = canvas - m
    line(br - L, br, br, br)
    line(br, br - L, br, br)

    # Diagonal slash from top-right toward bottom-left, inset so it doesn't
    # touch the brackets.
    inset = sx(slash_inset)
    line(canvas - m - inset, m + inset,
         m + inset, canvas - m - inset)

    # Red record dot, top-right corner.
    dot_r = sx(70)
    dot_cx = canvas - sx(margin - 40)
    dot_cy = sx(margin - 40)

    halo = Image.new("RGBA", (super_size, super_size), (0, 0, 0, 0))
    hd = ImageDraw.Draw(halo)
    hr = dot_r * 1.9
    hd.ellipse((dot_cx - hr, dot_cy - hr, dot_cx + hr, dot_cy + hr),
               fill=(255, 59, 48, 110))
    halo = halo.filter(ImageFilter.GaussianBlur(radius=sx(22)))
    over = Image.alpha_composite(over, halo)
    od = ImageDraw.Draw(over)

    od.ellipse((dot_cx - dot_r, dot_cy - dot_r,
                dot_cx + dot_r, dot_cy + dot_r),
               fill=(255, 59, 48, 255))

    # Subtle specular highlight on the dot for a hint of dimension.
    spec_r = dot_r * 0.30
    spec_cx = dot_cx - dot_r * 0.30
    spec_cy = dot_cy - dot_r * 0.30
    od.ellipse((spec_cx - spec_r, spec_cy - spec_r,
                spec_cx + spec_r, spec_cy + spec_r),
               fill=(255, 255, 255, 180))

    # Downsample to final size, then clip to the rounded-square mask so the
    # anti-aliased glyph respects the icon's corner radius.
    over_small = over.resize((size, size), Image.LANCZOS)

    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, size - 1, size - 1),
        radius=round(size * 0.2237),
        fill=255,
    )
    masked = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    masked.paste(over_small, (0, 0), mask)
    return Image.alpha_composite(bg, masked)


def main():
    cache = {}
    for size, fname in SIZES:
        if size not in cache:
            cache[size] = render(size)
        out_path = OUT_DIR / fname
        cache[size].save(out_path, "PNG", optimize=True)
        print(f"  wrote {out_path.relative_to(REPO)} ({size}x{size})")
    print(f"\n{len(SIZES)} icons written.")


if __name__ == "__main__":
    main()
