#!/usr/bin/env python3
"""Genera las cuatro fases del parpadeo rojo de la boya.

OpenTTD no cambia el sprite de ``SPR_IMG_BUOY`` (693): ``DoPaletteAnimations``
recolorea globalmente los índices 239 y 240, usados por la luz de radio. El
atlas RGBA no conserva esos índices, así que se hornean las cuatro parejas
que puede producir ``palette.cpp`` a partir del recorte 8bpp original.

Salidas:
- ``buoy_radio_anim_00.png`` … ``buoy_radio_anim_03.png``

Uso: ``python3 scripts/gen_radio_blink_anim_frames.py``
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

from nfo_sprite_meta import active_global_sprite_nfo, detect_graphics_mode, parse_global_sprite_rects
from opengfx_palette import indexed_dos_to_rgba
from pillow_compat import flattened_data

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
SPR_IMG_BUOY = 693
RADIO_INDEXES = (239, 240)

# Valores que ``RadioTowerBlink`` puede escribir en palette.cpp. El contador
# global avanza 8 unidades por tick y las dos entradas están desfasadas 0x40.
RADIO_FRAMES = (
    (255, 128),
    (255, 20),
    (128, 255),
    (20, 255),
)


def find_buoy_crop() -> Image.Image | None:
    """Devuelve SPR_IMG_BUOY indexado del baseset 8bpp activo."""
    if detect_graphics_mode(REPO) != "8bpp":
        print("  (omitidos frames de boya: el perfil activo no es 8bpp)")
        return None
    nfo = active_global_sprite_nfo(REPO, "8bpp")
    if nfo is None:
        raise SystemExit("No se encontró el NFO OpenGFX 8bpp activo")
    rect = parse_global_sprite_rects(nfo, "8bpp").get(SPR_IMG_BUOY)
    if rect is None:
        raise SystemExit(f"No se encontró SPR_IMG_BUOY {SPR_IMG_BUOY} en {nfo}")
    sheet = nfo.parent / rect.sheet
    if not sheet.is_file():
        raise SystemExit(f"No se encontró la hoja 8bpp de la boya: {sheet}")
    with Image.open(sheet) as source:
        if source.mode != "P":
            raise SystemExit(f"{sheet} no es indexada (modo {source.mode})")
        return source.crop((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h))


def render_frame(base: Image.Image, values: tuple[int, int]) -> Image.Image:
    """Convierte DOS a RGBA y reemplaza sólo los dos índices de radio."""
    rgba = indexed_dos_to_rgba(base)
    src = list(flattened_data(base))
    dst = list(flattened_data(rgba))
    for index, palette_index in enumerate(src):
        if palette_index == RADIO_INDEXES[0]:
            dst[index] = (values[0], 0, 0, 255)
        elif palette_index == RADIO_INDEXES[1]:
            dst[index] = (values[1], 0, 0, 255)
    rgba.putdata(dst)
    return rgba


def main() -> None:
    base = find_buoy_crop()
    if base is None:
        return
    for frame, values in enumerate(RADIO_FRAMES):
        render_frame(base, values).save(TILES_DIR / f"buoy_radio_anim_{frame:02d}.png")
    print(f"Generados {len(RADIO_FRAMES)} frames de boya en {TILES_DIR}")


if __name__ == "__main__":
    main()
