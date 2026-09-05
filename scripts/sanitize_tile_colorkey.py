#!/usr/bin/env python3
"""Elimina la clave magenta de transparencia de los tiles OpenGFX 8bpp.

Algunos generadores recortan una hoja ya decodificada y convierten el PNG a
RGBA sin conservar el índice transparente. El resultado es un rectángulo
fucsia opaco en puentes, túneles o controles. Este paso final actúa sobre el
catálogo de ``tiles/`` completo, antes de empaquetarlo en el atlas.

Sólo tiene efecto si el perfil activo registrado en ``.graphics_mode`` es
``8bpp``. El magenta es una clave del paquete clásico; no se toca el perfil
32bpp porque allí podría ser un píxel artístico válido.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from PIL import Image

from pillow_compat import flattened_data


ROOT = Path(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OPENGFX_DIR = ROOT / "assets" / "opengfx"
TILES_DIR = OPENGFX_DIR / "tiles"


def active_mode() -> str:
    try:
        return (OPENGFX_DIR / ".graphics_mode").read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def is_magenta_colorkey(red: int, green: int, blue: int) -> bool:
    """Acepta las variantes cuantizadas de la clave #EE00EE clásica."""
    return red >= 220 and blue >= 220 and green <= 40 and abs(red - blue) <= 24


def sanitize(path: Path, write: bool) -> int:
    with Image.open(path) as source:
        rgba = source.convert("RGBA")
    pixels = list(flattened_data(rgba))
    changed = sum(
        alpha > 0 and is_magenta_colorkey(red, green, blue)
        for red, green, blue, alpha in pixels
    )
    if changed and write:
        rgba.putdata(
            [
                (0, 0, 0, 0)
                if alpha > 0 and is_magenta_colorkey(red, green, blue)
                else (red, green, blue, alpha)
                for red, green, blue, alpha in pixels
            ]
        )
        rgba.save(path)
    return changed


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="falla si un tile 8bpp todavía conserva la clave magenta",
    )
    args = parser.parse_args(argv)

    if active_mode() != "8bpp":
        print("Sanitización de colorkey: omitida (perfil activo no es 8bpp)")
        return 0
    if not TILES_DIR.is_dir():
        print(f"No existe {TILES_DIR}", file=sys.stderr)
        return 2

    affected: list[tuple[Path, int]] = []
    for path in sorted(TILES_DIR.glob("*.png")):
        count = sanitize(path, write=not args.check)
        if count:
            affected.append((path, count))

    pixels = sum(count for _path, count in affected)
    if args.check:
        if affected:
            examples = ", ".join(
                f"{path.name} ({count})" for path, count in affected[:8]
            )
            print(
                "DRIFT: quedan "
                f"{pixels} píxeles de colorkey en {len(affected)} tiles: {examples}",
                file=sys.stderr,
            )
            return 1
        print("OK: ningún tile 8bpp conserva colorkey magenta opaco")
        return 0

    print(
        "Sanitización 8bpp: "
        f"{pixels} píxeles transparentados en {len(affected)} tiles"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
