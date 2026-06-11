#!/usr/bin/env python3
"""Genera assets/opengfx/tiles/tile_select.png: rombo blanco hueco (rejilla de
selección de teselas, como SPR_SELECT_TILE de OpenTTD). Se usa en el fantasma
de colocación de estaciones para marcar la huella completa.

La silueta se deriva del canal alfa de grass_rough.png (rombo isométrico
64×31): contorno opaco de ~2 px en blanco, interior transparente.
"""

from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
TILES = ROOT / "assets" / "opengfx" / "tiles"


def main() -> None:
    src = Image.open(TILES / "grass_rough.png").convert("RGBA")
    alpha = np.array(src)[:, :, 3]
    h, w = alpha.shape
    out = np.zeros((h, w, 4), dtype=np.uint8)

    opaque = alpha > 0
    pad = np.pad(opaque, 1, constant_values=False)
    interior = pad[:-2, 1:-1] & pad[2:, 1:-1] & pad[1:-1, :-2] & pad[1:-1, 2:]
    edge = opaque & ~interior
    # Engrosar el contorno 1 px hacia adentro para que se vea con zoom out.
    pad2 = np.pad(edge, 1, constant_values=False)
    thick = edge | (
        opaque & (pad2[:-2, 1:-1] | pad2[2:, 1:-1] | pad2[1:-1, :-2] | pad2[1:-1, 2:])
    )
    out[thick] = [255, 255, 255, 255]

    dest = TILES / "tile_select.png"
    Image.fromarray(out).save(dest)
    print(f"OK {dest.relative_to(ROOT)} ({w}x{h})")


if __name__ == "__main__":
    main()
