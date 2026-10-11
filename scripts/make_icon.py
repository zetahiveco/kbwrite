#!/usr/bin/env python3
"""Generates the KbWrite app icons.

Draws a red-pink rounded square (macOS style) with a white file/document
shape (folded corner) and bold "KB" on the file. Produces:

  assets/icon_1024.png      - full-resolution icon
  assets/icon_256.rgba      - raw RGBA bytes (embedded as the window icon)
  assets/icon.iconset/*.png - all sizes required by `iconutil`
"""

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "assets")
os.makedirs(ASSETS, exist_ok=True)

# Draw supersampled, then downscale for smooth anti-aliased edges.
S = 2048
OUT = 1024

# Red → pink gradient (rose-600 → deep pink).
TOP = (236, 50, 110)
BOT = (190, 18, 80)

INK = (219, 39, 119)      # "KB" text color (deep pink)
FOLD = (246, 213, 226)    # folded-corner flap (light pink)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def load_bold_font(size):
    candidates = [
        ("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 0),
        ("/System/Library/Fonts/SFNS.ttf", 0),
        ("/System/Library/Fonts/Helvetica.ttc", 1),
        ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
        ("C:\\Windows\\Fonts\\arialbd.ttf", 0),
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 0),
    ]
    for path, index in candidates:
        try:
            font = ImageFont.truetype(path, size, index=index)
            if "SFNS" in path:
                try:
                    font.set_variation_by_name("Bold")
                except Exception:
                    pass
            return font
        except Exception:
            continue
    return ImageFont.load_default()


def make_icon():
    icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    # macOS icon grid: the artwork occupies ~80.5% of the canvas so the icon
    # renders the same size as other apps' icons in the Dock/Finder.
    art = int(0.805 * S)
    m = (S - art) // 2

    # --- Background: rounded square with vertical red->pink gradient ---
    grad = Image.new("RGB", (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        gd.line([(0, y), (S, y)], fill=lerp(TOP, BOT, y / (S - 1)))

    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [m, m, m + art, m + art], radius=int(0.225 * art), fill=255
    )
    icon.paste(grad, (0, 0), mask)

    # --- File geometry (proportions relative to the artwork area) ---
    fw, fh = int(0.58 * art), int(0.72 * art)
    fx0 = (S - fw) // 2
    fy0 = (S - fh) // 2
    fx1, fy1 = fx0 + fw, fy0 + fh
    fold = int(0.28 * fw)
    radius = int(0.083 * fw)

    # Page silhouette mask (rounded rect with the top-right corner cut
    # away). Used for both the white page and the shadow clipping.
    page_mask = Image.new("L", (S, S), 0)
    pm = ImageDraw.Draw(page_mask)
    pm.rounded_rectangle(
        [fx0, fy0, fx1, fy1],
        radius=radius,
        fill=255,
        corners=(True, False, True, True),
    )
    # The corner triangle beyond the fold diagonal is removed entirely.
    pm.polygon([(fx1 - fold, fy0), (fx1, fy0), (fx1, fy0 + fold)], fill=0)

    # Soft shadow visible ONLY just below the page: the blurred shape is
    # masked by the page silhouette shifted down, so it never peeks past
    # the side edges or through the cut folded corner.
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        [fx0 + 10, fy0, fx1 - 10, fy1 + 30],
        radius=radius,
        fill=(60, 0, 20, 70),
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(18))
    shadow_mask = Image.new("L", (S, S), 0)
    shadow_mask.paste(page_mask, (0, int(0.026 * art)))
    r, g, b, a = shadow.split()
    shadow = Image.merge("RGBA", (r, g, b, ImageChops.multiply(a, shadow_mask)))
    icon.alpha_composite(shadow)

    # White page composited through the mask.
    page = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    page.paste(Image.new("RGBA", (S, S), (255, 255, 255, 255)), (0, 0), page_mask)
    icon.alpha_composite(page)

    # Folded flap: triangle inside the page, below-left of the diagonal.
    flap = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(flap).polygon(
        [(fx1 - fold, fy0), (fx1, fy0 + fold), (fx1 - fold, fy0 + fold)],
        fill=FOLD + (255,),
    )
    icon.alpha_composite(flap)

    # --- "KB" text on the file ---
    font = load_bold_font(int(0.42 * fh))
    td = ImageDraw.Draw(icon)
    bbox = td.textbbox((0, 0), "KB", font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    tx = fx0 + (fw - tw) // 2 - bbox[0]
    ty = fy0 + (fh - th) // 2 - bbox[1] + int(0.02 * art)
    td.text((tx, ty), "KB", font=font, fill=INK + (255,))

    return icon.resize((OUT, OUT), Image.LANCZOS)


icon = make_icon()

# Full-size PNG.
icon.save(os.path.join(ASSETS, "icon_1024.png"))

# Raw RGBA for the in-app window icon (256x256).
small = icon.resize((256, 256), Image.LANCZOS)
with open(os.path.join(ASSETS, "icon_256.rgba"), "wb") as f:
    f.write(small.tobytes("raw", "RGBA"))

# macOS iconset (every size `iconutil` expects).
iconset = os.path.join(ASSETS, "icon.iconset")
os.makedirs(iconset, exist_ok=True)
for base in (16, 32, 128, 256, 512):
    for suffix, scale in (("", 1), ("@2x", 2)):
        size = base * scale
        icon.resize((size, size), Image.LANCZOS).save(
            os.path.join(iconset, f"icon_{base}x{base}{suffix}.png")
        )

print("Icon written to", ASSETS)