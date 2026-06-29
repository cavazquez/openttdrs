#!/usr/bin/env python3
"""Genera frames de animación de fuego de refinería (ciclo `oil_refinery`).

OpenTTD anima los índices de paleta 232–238 con la tabla `oil_refinery[7]`
(`DoPaletteAnimations` en `palette.cpp`). Los PNG RGBA hornean un frame estático;
aquí remapeamos píxeles de llama al slot del ciclo y avanzamos `(slot + f) % 7`.

Sprites: torres de fuego de refinería (gfx 19–22, OpenGFX 2081–2092).

Salida: `industry_{id}_fire_anim_{f:02d}.png` (f=0 idéntico al base).

Uso: python3 scripts/gen_oil_refinery_anim_frames.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"

# `_extra_palette_values.oil_refinery` — table/palettes.h
OIL_REFINERY = [
    (252, 60, 0),
    (252, 84, 0),
    (252, 108, 0),
    (252, 124, 0),
    (252, 148, 0),
    (252, 172, 0),
    (252, 196, 0),
]
FRAME_COUNT = len(OIL_REFINERY)

# Sprites de llama refinería (gfx 19–22, todos los estadios 0–3).
REFINERY_FIRE_SPRITE_IDS = [
    2081,
    2082,
    2083,
    2084,
    2085,
    2086,
    2087,
    2088,
    2089,
    2090,
    2091,
    2092,
]

OIL_SLOT = {c: k for k, c in enumerate(OIL_REFINERY)}


def nearest_slot(r: int, g: int, b: int) -> int:
    best = 0
    best_d = 10**9
    for k, (cr, cg, cb) in enumerate(OIL_REFINERY):
        d = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
        if d < best_d:
            best_d = d
            best = k
    return best


def is_fire_pixel(r: int, g: int, b: int, a: int) -> bool:
    if a == 0:
        return False
    if (r, g, b) in OIL_SLOT:
        return True
    # Llama naranja/amarilla (32bpp puede no coincidir exacto con la tabla).
    if r >= 200 and g >= 100 and b <= 80 and r >= g:
        return True
    if r >= 240 and g >= 50 and b <= 60:
        return True
    return False


def render_frame(base: Image.Image, frame: int) -> Image.Image:
    out = base.copy()
    px = out.load()
    w, h = out.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if not is_fire_pixel(r, g, b, a):
                continue
            slot = OIL_SLOT.get((r, g, b), nearest_slot(r, g, b))
            px[x, y] = (*OIL_REFINERY[(slot + frame) % FRAME_COUNT], a)
    return out


def main() -> None:
    total = 0
    for sid in REFINERY_FIRE_SPRITE_IDS:
        src = TILES_DIR / f"industry_{sid}.png"
        if not src.is_file():
            print(f"  (omitido industry_{sid}.png: no existe)")
            continue
        base = Image.open(src).convert("RGBA")
        for f in range(FRAME_COUNT):
            out_name = f"industry_{sid}_fire_anim_{f:02d}.png"
            render_frame(base, f).save(TILES_DIR / out_name)
            total += 1
    print(
        f"Generados {total} frames de fuego refinería "
        f"({len(REFINERY_FIRE_SPRITE_IDS)} sprites × {FRAME_COUNT}) en {TILES_DIR}"
    )


if __name__ == "__main__":
    main()
