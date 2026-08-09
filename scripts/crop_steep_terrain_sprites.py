#!/usr/bin/env python3
"""Recorta los ocho sprites de terreno para pendientes `SLOPE_STEEP_*`.

OpenTTD no usa el valor crudo de `Slope` como offset gráfico. Las pendientes
empinadas 23/27/29/30 se convierten con `SlopeToSpriteOffset` a los slots
15..18. Este script añade solamente esos PNG faltantes, sin reextraer ni
borrar el resto de los assets de OpenGFX.

Uso:
  python3 scripts/crop_steep_terrain_sprites.py
  python3 scripts/crop_steep_terrain_sprites.py --force
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from crop_missing_industry_pngs import crop_sprite, load_sheets, opengfx_paths, parse_sprite_rect


SPRITES: tuple[tuple[int, str], ...] = tuple(
    (3981 + offset, f"terrain_grass_slope_{offset:02}.png")
    for offset in range(15, 19)
) + tuple(
    (4000 + offset, f"terrain_rough_slope_{offset:02}.png")
    for offset in range(15, 19)
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--force",
        action="store_true",
        help="reemplaza los PNG steep aunque ya existan",
    )
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    tiles = repo / "assets" / "opengfx" / "tiles"
    try:
        sprites_dir, nfo, prefix, mode = opengfx_paths(repo)
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 1
    if not nfo.is_file():
        print(f"Falta NFO: {nfo}", file=sys.stderr)
        return 1

    todo = [(sid, name) for sid, name in SPRITES if args.force or not (tiles / name).is_file()]
    if not todo:
        print("Los 8 sprites steep de terreno ya existen.")
        return 0

    rects = parse_sprite_rect(nfo)
    sheets = load_sheets(sprites_dir, prefix, mode)
    print(f"Modo {mode}; recortando {len(todo)} sprites steep desde {nfo.name}…")
    failures: list[str] = []
    for sid, name in todo:
        status = crop_sprite(sid, tiles / name, rects, sheets, mode)
        if status != "ok":
            failures.append(f"{name}: {status}")

    if failures:
        print("No se pudieron recortar: " + ", ".join(failures), file=sys.stderr)
        return 1
    print("Listo. Ejecutá python3 scripts/gen_tile_atlas.py para incluirlos en el atlas.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
