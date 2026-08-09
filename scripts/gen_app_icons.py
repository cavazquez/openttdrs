#!/usr/bin/env python3
"""Genera tamaños hicolor desde static/app/openttdrs-icon.png."""
from __future__ import annotations

from collections import deque
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    raise SystemExit("Instala Pillow: pip install Pillow") from None

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "static/app/openttdrs-icon.png"
OUT = ROOT / "static/app/icons"

SIZES = (16, 32, 48, 64, 128, 256)


def remove_outer_white_background(image: Image.Image) -> Image.Image:
    """Vuelve transparente solo el blanco conectado al borde del lienzo.

    El arte contiene blancos legítimos (nubes, rótulo y detalles metálicos),
    así que no se puede eliminar todo píxel blanco: el flood-fill desde los
    bordes preserva el interior del emblema.
    """
    rgba = image.convert("RGBA")
    if rgba.getchannel("A").getextrema()[0] < 255:
        return rgba

    width, height = rgba.size
    pixels = rgba.load()

    def is_outer_white(x: int, y: int) -> bool:
        red, green, blue, alpha = pixels[x, y]
        return alpha == 255 and min(red, green, blue) >= 235 and max(red, green, blue) - min(
            red, green, blue
        ) <= 12

    pending: deque[tuple[int, int]] = deque()
    seen: set[tuple[int, int]] = set()
    for x in range(width):
        pending.extend(((x, 0), (x, height - 1)))
    for y in range(1, height - 1):
        pending.extend(((0, y), (width - 1, y)))

    while pending:
        x, y = pending.popleft()
        if (x, y) in seen or not is_outer_white(x, y):
            continue
        seen.add((x, y))
        red, green, blue, _ = pixels[x, y]
        pixels[x, y] = (red, green, blue, 0)
        if x > 0:
            pending.append((x - 1, y))
        if x + 1 < width:
            pending.append((x + 1, y))
        if y > 0:
            pending.append((x, y - 1))
        if y + 1 < height:
            pending.append((x, y + 1))
    return rgba


def main() -> int:
    if not SRC.is_file():
        print(f"Falta {SRC}", flush=True)
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    img = remove_outer_white_background(Image.open(SRC))
    # El icono fuente es el que consume winit; guardar el alfa aquí evita que
    # la barra de título/tray siga mostrando un rectángulo blanco.
    img.save(SRC)
    for size in SIZES:
        path = OUT / f"{size}x{size}.png"
        img.resize((size, size), Image.Resampling.LANCZOS).save(path)
        print(f"Escrito {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
