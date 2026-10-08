#!/usr/bin/env python3
"""Trim the quicksilver wave spritesheet's frames to their non-transparent
content and repack them into a uniform 5x5 grid.

The source frames are 256x256 with the wave art sitting as a narrow vertical
strip surrounded by transparent padding. Drawn in game, that padding made the
wave look wrong. This crops every frame to its content bounding box, packs the
crops into uniform cells (sized to the largest crop), and rewrites:

    assets/sprite_packs/Effects/Quicksilver Wave/spritesheet.png
    assets/sprite_packs/Effects/Quicksilver Wave/atlas.json

The original is preserved as spritesheet_untrimmed.png.

Usage:  python3 tools/fix_wave_atlas.py
"""

import json
import os
from PIL import Image

SHEET_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "assets", "sprite_packs", "Effects", "Quicksilver Wave",
)
COLS = 5
ROWS = 5


def main() -> None:
    src_path = os.path.join(SHEET_DIR, "spritesheet.png")
    img = Image.open(src_path).convert("RGBA")
    frame_w = img.width // COLS
    frame_h = img.height // ROWS

    crops = []
    for row in range(ROWS):
        for col in range(COLS):
            frame = img.crop(
                (col * frame_w, row * frame_h, (col + 1) * frame_w, (row + 1) * frame_h)
            )
            bbox = frame.getbbox()
            if bbox is None:
                raise SystemExit(f"frame {row * COLS + col} is fully transparent")
            crops.append(frame.crop(bbox))

    cell_w = max(crop.width for crop in crops)
    cell_h = max(crop.height for crop in crops)
    print(f"source frame {frame_w}x{frame_h} -> trimmed cell {cell_w}x{cell_h}")

    # Preserve the original before overwriting it.
    backup = os.path.join(SHEET_DIR, "spritesheet_untrimmed.png")
    if not os.path.exists(backup):
        img.save(backup)

    out = Image.new("RGBA", (cell_w * COLS, cell_h * ROWS), (0, 0, 0, 0))
    frames = {}
    for index, crop in enumerate(crops):
        col = index % COLS
        row = index // COLS
        x = col * cell_w
        y = row * cell_h
        out.paste(crop, (x, y))
        frames[str(index)] = {"x": x, "y": y, "w": cell_w, "h": cell_h, "duration": 1}

    out.save(os.path.join(SHEET_DIR, "spritesheet.png"))

    atlas = {
        "frames": frames,
        "meta": {
            "size": {"w": cell_w * COLS, "h": cell_h * ROWS},
            "frame_size": {"w": cell_w, "h": cell_h},
            "notes": "frames trimmed to non-transparent content by tools/fix_wave_atlas.py",
        },
    }
    with open(os.path.join(SHEET_DIR, "atlas.json"), "w") as handle:
        json.dump(atlas, handle, indent=2)
        handle.write("\n")

    print(f"wrote sheet {out.width}x{out.height} and atlas.json")


if __name__ == "__main__":
    main()
