#!/usr/bin/env python3
"""Conversión exacta de hojas OpenGFX clásicas indexadas (paleta DOS).

OpenGFX 8bpp declara ``palette = DOS`` en ``opengfx.obg``. ``grfcodec``
puede guardar un PNG con paleta de trabajo Windows (``-p 2``), pero sus
píxeles siguen siendo índices DOS. Convertir por RGB en ese punto mezcla dos
espacios de color: los índices 1..9 (metal/asfalto) se vuelven magenta y
215..226 (padding transparente) se vuelven agua. Esta utilidad convierte por
índice antes de recortar cualquier sprite.
"""

from __future__ import annotations

import re
from functools import lru_cache
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
PALETTES_H = ROOT / "third_party" / "openttd" / "table" / "palettes.h"

# OpenGFX usa estos doce slots rosa como padding de las hojas. En el PNG
# histórico -p2 aparecían como tonos de agua y causaban halos azulados.
UNUSED_TRANSPARENT_INDICES = frozenset(range(215, 227))

# ``DoPaletteAnimations`` parte de contador 0 y lo incrementa a 8. Estos son
# los valores iniciales de sus 28 slots para clima templado. El pipeline de
# agua reemplaza 245..254 por todos sus frames más adelante.
_FIZZY = ((76, 24, 8), (108, 44, 24), (144, 72, 52), (176, 108, 84), (212, 148, 128))
_OIL = (
    (252, 60, 0), (252, 84, 0), (252, 108, 0), (252, 124, 0),
    (252, 148, 0), (252, 172, 0), (252, 196, 0),
)
_LIGHTHOUSE = ((240, 208, 0), (0, 0, 0), (0, 0, 0), (0, 0, 0))
_DARK_WATER = ((32, 68, 112), (36, 72, 116), (40, 76, 120), (44, 80, 124), (48, 84, 128))
_GLITTER_WATER = (
    (216, 244, 252), (172, 208, 224), (132, 172, 196), (100, 132, 168),
    (72, 100, 144), (72, 100, 144), (72, 100, 144), (72, 100, 144),
    (72, 100, 144), (72, 100, 144), (72, 100, 144), (72, 100, 144),
    (100, 132, 168), (132, 172, 196), (172, 208, 224),
)


@lru_cache(maxsize=1)
def dos_palette() -> tuple[tuple[int, int, int], ...]:
    """Devuelve los 256 colores de ``_palette`` indexados como OpenTTD."""
    text = PALETTES_H.read_text(encoding="utf-8")
    start = text.index("static const Palette _palette")
    end = text.index("\n\t},\n\t0,  // First dirty", start)
    values = [
        tuple(map(int, match))
        for match in re.findall(r"M\(\s*(\d+),\s*(\d+),\s*(\d+)\)", text[start:end])
    ]
    if len(values) != 255:
        raise RuntimeError(f"paleta DOS inválida: esperaba 255 M(...), hay {len(values)}")
    return ((0, 0, 0), *values)


@lru_cache(maxsize=1)
def _initial_animated_palette() -> dict[int, tuple[int, int, int]]:
    """Replica el primer ``DoPaletteAnimations`` de OpenTTD (contador = 8)."""
    fizzy_start = ((~8 * 512) & 0xFFFF) * len(_FIZZY) >> 16
    oil_start = ((~8 * 512) & 0xFFFF) * len(_OIL) >> 16
    lighthouse_start = ((8 * 256) & 0xFFFF) * len(_LIGHTHOUSE) >> 16
    dark_start = ((8 * 320) & 0xFFFF) * len(_DARK_WATER) >> 16
    glitter_start = ((8 * 128) & 0xFFFF) * len(_GLITTER_WATER) >> 16

    values: dict[int, tuple[int, int, int]] = {}
    for offset in range(len(_FIZZY)):
        values[227 + offset] = _FIZZY[(fizzy_start + offset) % len(_FIZZY)]
    for offset in range(len(_OIL)):
        values[232 + offset] = _OIL[(oil_start + offset) % len(_OIL)]
    i = (8 >> 1) & 0x7F
    first = 255 if i < 0x3F else (128 if i < 0x4A or i >= 0x75 else 20)
    i ^= 0x40
    second = 255 if i < 0x3F else (128 if i < 0x4A or i >= 0x75 else 20)
    values[239] = (first, 0, 0)
    values[240] = (second, 0, 0)
    for offset in range(len(_LIGHTHOUSE)):
        values[241 + offset] = _LIGHTHOUSE[(lighthouse_start + offset) % len(_LIGHTHOUSE)]
    for offset in range(len(_DARK_WATER)):
        values[245 + offset] = _DARK_WATER[(dark_start + offset) % len(_DARK_WATER)]
    for offset in range(5):
        values[250 + offset] = _GLITTER_WATER[(glitter_start + 3 * offset) % len(_GLITTER_WATER)]
    return values


def indexed_dos_to_rgba(image: Image.Image) -> Image.Image:
    """Convierte una imagen ``P`` de OpenGFX a RGBA sin perder índices."""
    if image.mode != "P":
        return image.convert("RGBA")

    palette = dos_palette()
    animated = _initial_animated_palette()
    pixels: list[tuple[int, int, int, int]] = []
    for index in image.get_flattened_data():
        if index == 0 or index in UNUSED_TRANSPARENT_INDICES:
            pixels.append((0, 0, 0, 0))
            continue
        red, green, blue = animated.get(index, palette[index])
        pixels.append((red, green, blue, 255))
    rgba = Image.new("RGBA", image.size)
    rgba.putdata(pixels)
    return rgba


def dematte_legacy_colorkey(image: Image.Image) -> Image.Image:
    """Fallback para imágenes no indexadas que ya perdieron sus índices."""
    rgba = image.convert("RGBA")
    data = []
    for red, green, blue, alpha in rgba.get_flattened_data():
        magenta = red >= 220 and blue >= 220 and green <= 40 and abs(red - blue) <= 24
        if alpha > 0 and ((red, green, blue) == (0, 0, 255) or magenta):
            data.append((0, 0, 0, 0))
        else:
            data.append((red, green, blue, alpha))
    rgba.putdata(data)
    return rgba
