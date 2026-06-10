#!/usr/bin/env python3
"""Genera los frames de animación de paleta del agua (dark + glitter water).

OpenTTD anima el agua ciclando las entradas de paleta 245–249 (dark water,
5 colores) y 250–254 (glitter water, tabla de 15 colores) — ver
`DoPaletteAnimations` en `palette.cpp` y `_extra_palette_values` en
`table/palettes.h`. Nuestros PNG tienen esos índices horneados con el color
estático de la paleta base, que coincide exactamente con `dark_water[k]` /
`glitter_water[k]`; eso permite invertir el mapeo por color RGB.

Para cada frame `f` (0..14):
- píxel con color `dark_water[k]`   → `dark_water[(k + f) % 5]`
- píxel con color `glitter_water[k]` → `glitter_water[(k + f) % 15]`

Salida: `water_anim_{f:02d}.png` y `shore_full_{i:02d}_anim_{f:02d}.png`
(frame 0 es idéntico al sprite base). Las orillas son el set completo de 18
sprites extraído por `gen_shore_full_set.py`.

Uso: python3 scripts/gen_water_anim_frames.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"

# `_extra_palette_values` de table/palettes.h (clima templado).
DARK_WATER = [
    (32, 68, 112),
    (36, 72, 116),
    (40, 76, 120),
    (44, 80, 124),
    (48, 84, 128),
]
GLITTER_WATER = [
    (216, 244, 252),
    (172, 208, 224),
    (132, 172, 196),
    (100, 132, 168),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (100, 132, 168),
    (132, 172, 196),
    (172, 208, 224),
]
FRAME_COUNT = 15

# La paleta base (índices 250–254) hornea los 5 primeros valores del glitter.
DARK_SLOT = {c: k for k, c in enumerate(DARK_WATER)}
GLITTER_SLOT = {c: k for k, c in enumerate(GLITTER_WATER[:5])}


def render_frame(base: Image.Image, frame: int) -> Image.Image:
    out = base.copy()
    px = out.load()
    w, h = out.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            slot = DARK_SLOT.get((r, g, b))
            if slot is not None:
                px[x, y] = (*DARK_WATER[(slot + frame) % 5], a)
                continue
            slot = GLITTER_SLOT.get((r, g, b))
            if slot is not None:
                px[x, y] = (*GLITTER_WATER[(slot + frame) % 15], a)
    return out


def main() -> None:
    jobs = [("water.png", "water_anim_{f:02d}.png")] + [
        (f"shore_full_{i:02d}.png", f"shore_full_{i:02d}_anim_{{f:02d}}.png")
        for i in range(18)
    ]
    total = 0
    for src_name, out_pattern in jobs:
        base = Image.open(TILES_DIR / src_name).convert("RGBA")
        for f in range(FRAME_COUNT):
            render_frame(base, f).save(TILES_DIR / out_pattern.format(f=f))
            total += 1
    print(f"Generados {total} frames de animación de agua en {TILES_DIR}")


if __name__ == "__main__":
    main()
