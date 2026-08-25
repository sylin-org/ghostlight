#!/usr/bin/env python3
"""Generate every Ghostlight raster from the pixel-art master.

The master is pixel art (extension/icons/ghostlight-mascot.png, 100x100, 1:1
logical pixels), so the whole point is to keep the pixel grid ALIGNED -- never
fractionally resample it into mush. Two regimes, both crisp:

  * target >= master: nearest-neighbor INTEGER upscale, then center on a
    transparent square canvas (the pixels stay square and aligned).
  * target <  master: you cannot integer-upscale into a smaller box, so build a
    supersample canvas whose side is an integer multiple of the target (the
    master padded so its grid lands on that multiple), then reduce by that exact
    integer factor with a box filter. E.g. a 32px icon is rendered by padding the
    master onto a 128px canvas (4x) and boxing 128 -> 32. This is the "pad to a
    larger aligned version, then reduce" rule.

Requires Pillow (`pip install pillow`). Run from anywhere:

    python scripts/gen-icons.py

Re-run whenever the master changes.
"""
from __future__ import annotations

import math
from pathlib import Path

from PIL import Image

try:
    NEAREST = Image.Resampling.NEAREST
    BOX = Image.Resampling.BOX
except AttributeError:  # Pillow < 9.1
    NEAREST = Image.NEAREST
    BOX = Image.BOX

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "extension" / "icons"
MASTER = ICONS / "ghostlight-mascot.png"

# (output path relative to ROOT, target square size in px). The extension icons
# the manifest references (16/32/48/128), the hi-res store/README asset (512),
# the site mascot, and the marketplace logo -- all from the one master.
TARGETS: list[tuple[str, int]] = [
    ("extension/icons/icon16.png", 16),
    ("extension/icons/icon32.png", 32),
    ("extension/icons/icon48.png", 48),
    ("extension/icons/icon128.png", 128),
    ("extension/icons/icon512.png", 512),
    ("site/mascot.png", 512),
    ("docs/assets/logo-400.png", 400),
]


def render(master: Image.Image, size: int) -> Image.Image:
    """Render the master at `size`x`size`, keeping the pixel grid aligned."""
    src = master.width  # square master
    if size >= src:
        # Largest integer scale that fits, nearest-neighbor, centered.
        k = size // src
        scaled = master.resize((src * k, src * k), NEAREST)
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        off = (size - src * k) // 2
        canvas.paste(scaled, (off, off), scaled)
        return canvas

    # size < master: supersample to the smallest integer multiple of `size` that
    # still holds the master, pad-align, then reduce by that exact factor.
    factor = math.ceil(src / size)
    super_side = size * factor
    supered = Image.new("RGBA", (super_side, super_side), (0, 0, 0, 0))
    off = (super_side - src) // 2
    supered.paste(master, (off, off), master)
    return supered.resize((size, size), BOX)


def main() -> None:
    master = Image.open(MASTER).convert("RGBA")
    if master.width != master.height:
        raise SystemExit(f"master must be square, got {master.size}")
    for rel, size in TARGETS:
        out = ROOT / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        render(master, size).save(out)
        print(f"wrote {rel}  ({size}x{size})")


if __name__ == "__main__":
    main()
