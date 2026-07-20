#!/usr/bin/env python3
"""Genera frames de animación faro/estadio (ciclo `lighthouse`).

OpenTTD anima con la tabla `lighthouse[4]` (`DoPaletteAnimations` en
`palette.cpp`): amarillo → negro → negro → negro (parpadeo de la luz).

Sprites:
- Faro: `object_lighthouse.png` (OpenGFX 2602)
- Estadio luces s2: `house_s1483.png` … `house_s1486.png`

Salida:
- `object_lighthouse_anim_{f:02d}.png`
- `house_s{id}_anim_{f:02d}.png`

Uso: python3 scripts/gen_lighthouse_anim_frames.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"

# `_extra_palette_values.lighthouse` — table/palettes.h
LIGHTHOUSE = [
    (240, 208, 0),
    (0, 0, 0),
    (0, 0, 0),
    (0, 0, 0),
]
FRAME_COUNT = len(LIGHTHOUSE)

# (sprite_id, base_filename_stem)
SOURCES = [
    (2602, "object_lighthouse"),
    (1483, "house_s1483"),
    (1484, "house_s1484"),
    (1485, "house_s1485"),
    (1486, "house_s1486"),
]

LIGHTHOUSE_SLOT = {c: k for k, c in enumerate(LIGHTHOUSE)}


def nearest_slot(r: int, g: int, b: int) -> int:
    best = 0
    best_d = 10**9
    for k, (cr, cg, cb) in enumerate(LIGHTHOUSE):
        d = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
        if d < best_d:
            best_d = d
            best = k
    return best


def is_light_pixel(r: int, g: int, b: int, a: int) -> bool:
    if a == 0:
        return False
    if (r, g, b) in LIGHTHOUSE_SLOT:
        return True
    # OpenGFX2 32ez: linterna en amarillo puro `(255,255,0)`.
    if r >= 240 and g >= 240 and b <= 40:
        return True
    # Amarillo/ámbar de la linterna (8bpp / variantes). No incluir oliva
    # `(80,80,0)` del cristal: nearest_slot lo trata como negro y rompe el ciclo.
    if r >= 200 and g >= 160 and b <= 80 and r > g:
        return True
    if r >= 220 and g >= 180 and b <= 40:
        return True
    return False


def render_frame(base: Image.Image, frame: int) -> Image.Image:
    out = base.copy()
    px = out.load()
    w, h = out.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if not is_light_pixel(r, g, b, a):
                continue
            slot = LIGHTHOUSE_SLOT.get((r, g, b), nearest_slot(r, g, b))
            # Solo el slot 0 (amarillo) cicla; negros se quedan negros.
            if slot != 0 and (r, g, b) == (0, 0, 0):
                continue
            px[x, y] = (*LIGHTHOUSE[(slot + frame) % FRAME_COUNT], a)
    return out


def main() -> None:
    total = 0
    for sid, stem in SOURCES:
        src = TILES_DIR / f"{stem}.png"
        if not src.is_file():
            print(f"  (omitido {stem}.png: no existe)")
            continue
        base = Image.open(src).convert("RGBA")
        for f in range(FRAME_COUNT):
            if sid == 2602:
                out_name = f"object_lighthouse_anim_{f:02d}.png"
            else:
                out_name = f"house_s{sid}_anim_{f:02d}.png"
            render_frame(base, f).save(TILES_DIR / out_name)
            total += 1
    print(f"Generados {total} frames lighthouse/stadium en {TILES_DIR}")


if __name__ == "__main__":
    main()
