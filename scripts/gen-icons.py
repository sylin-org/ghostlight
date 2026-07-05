#!/usr/bin/env python3
"""Generate the Ghostlight extension icons from the master mascot PNG.

Trims the transparent margin off extension/icons/mascot.png, then fits the
artwork (aspect ratio preserved) centered on a transparent square canvas at each
target size with a small uniform padding. High-quality Lanczos downscaling.

Requires Pillow (`pip install pillow`). Run from anywhere:

    python scripts/gen-icons.py

The manifest references icon16/32/48/128; icon512 is a hi-res asset for the
store listing and README. Re-run this whenever mascot.png changes.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "extension" / "icons"
MASTER = ICONS / "mascot.png"

# (size_px, pad_px): 16/32 fill the frame for toolbar legibility; larger sizes
# get a little breathing room.
TARGETS = [(16, 0), (32, 1), (48, 2), (128, 5), (512, 20)]


def main() -> None:
    src = Image.open(MASTER).convert("RGBA")
    bbox = src.getbbox()
    art = src.crop(bbox) if bbox else src
    w, h = art.size
    for size, pad in TARGETS:
        inner = size - 2 * pad
        scale = min(inner / w, inner / h)
        nw, nh = max(1, round(w * scale)), max(1, round(h * scale))
        resized = art.resize((nw, nh), Image.LANCZOS)
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        canvas.paste(resized, ((size - nw) // 2, (size - nh) // 2), resized)
        out = ICONS / f"icon{size}.png"
        canvas.save(out)
        print(f"wrote {out.relative_to(ROOT)}  ({nw}x{nh} art in {size}x{size})")


if __name__ == "__main__":
    main()
