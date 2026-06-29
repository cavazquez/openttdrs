#!/usr/bin/env python3
"""Genera frames de animación de paleta de bebidas gaseosas (`fizzy_drink`).

OpenTTD cicla la tabla `fizzy_drink[5]` en `DoPaletteAnimations` (`palette.cpp`).
Sprites Toyland con índices de burbuja/líquido animado:

- Fábrica bebidas gfx 156–158: industry_4763..4765
- Draw proc burbujas: industry_4746..4747

Uso: python3 scripts/gen_fizzy_drink_anim_frames.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"

FIZZY_DRINK = [
    (76, 24, 8),
    (108, 44, 24),
    (144, 72, 52),
    (176, 108, 84),
    (212, 148, 128),
]
FRAME_COUNT = len(FIZZY_DRINK)

FIZZY_DRINK_SPRITE_IDS = [
    4763,
    4764,
    4765,
    4746,
    4747,
]

FIZZY_SLOT = {c: k for k, c in enumerate(FIZZY_DRINK)}


def nearest_slot(r: int, g: int, b: int) -> int:
    best = 0
    best_d = 10**9
    for k, (cr, cg, cb) in enumerate(FIZZY_DRINK):
        d = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
        if d < best_d:
            best_d = d
            best = k
    return best


def is_fizzy_pixel(r: int, g: int, b: int, a: int) -> bool:
    if a == 0:
        return False
    if (r, g, b) in FIZZY_SLOT:
        return True
    # Tonos burbuja/líquido (32bpp puede diferir ligeramente).
    if r >= 60 and g >= 20 and b <= 140 and r > b:
        return True
    return False


def render_frame(base: Image.Image, frame: int) -> Image.Image:
    out = base.copy()
    px = out.load()
    w, h = out.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if not is_fizzy_pixel(r, g, b, a):
                continue
            slot = FIZZY_SLOT.get((r, g, b), nearest_slot(r, g, b))
            px[x, y] = (*FIZZY_DRINK[(slot + frame) % FRAME_COUNT], a)
    return out


def main() -> None:
    total = 0
    for sid in FIZZY_DRINK_SPRITE_IDS:
        src = TILES_DIR / f"industry_{sid}.png"
        if not src.is_file():
            print(f"  (omitido industry_{sid}.png: no existe)")
            continue
        base = Image.open(src).convert("RGBA")
        for f in range(FRAME_COUNT):
            out_name = f"industry_{sid}_fizzy_anim_{f:02d}.png"
            render_frame(base, f).save(TILES_DIR / out_name)
            total += 1
    print(
        f"Generados {total} frames fizzy drink "
        f"({len(FIZZY_DRINK_SPRITE_IDS)} sprites × {FRAME_COUNT}) en {TILES_DIR}"
    )


if __name__ == "__main__":
    main()
