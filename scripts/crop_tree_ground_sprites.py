#!/usr/bin/env python3
"""Recorta los suelos completos que usa ``DrawTile_Trees``.

OpenTTD combina el tipo de suelo con ``SlopeToSpriteOffset`` antes de dibujar
los árboles. Los PNG históricos solo incluían las pendientes de césped pleno,
rough y el snow/desert plano; eso ocultaba las diferencias de densidad de
``TreeGround`` de una partida SAV.

Este script es la vía incremental (sin descargar ni reextraer OpenGFX entero)
para generar las dos tablas de 4 densidades × 19 pendientes:

* ``terrain_grass_density_<0..3>_<00..18>.png``;
* ``terrain_snow_desert_<0..3>_<00..18>.png``.

Uso:
  python3 scripts/crop_tree_ground_sprites.py
  python3 scripts/crop_tree_ground_sprites.py --force

Luego ejecutá ``python3 scripts/gen_tile_atlas.py``.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from crop_missing_industry_pngs import crop_sprite, load_sheets, opengfx_paths, parse_sprite_rect


GRASS_BASES = (3924, 3943, 3962, 3981)
SNOW_DESERT_BASES = (4493, 4512, 4531, 4550)
SLOPE_OFFSETS = range(19)

SPRITES: tuple[tuple[int, str], ...] = tuple(
    (base + offset, f"terrain_grass_density_{density}_{offset:02}.png")
    for density, base in enumerate(GRASS_BASES)
    for offset in SLOPE_OFFSETS
) + tuple(
    (base + offset, f"terrain_snow_desert_{density}_{offset:02}.png")
    for density, base in enumerate(SNOW_DESERT_BASES)
    for offset in SLOPE_OFFSETS
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--force",
        action="store_true",
        help="reemplaza los PNG aunque ya existan",
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
        print(f"Los {len(SPRITES)} sprites de suelo de árboles ya existen.")
        return 0

    rects = parse_sprite_rect(nfo)
    sheets = load_sheets(sprites_dir, prefix, mode)
    print(f"Modo {mode}; recortando {len(todo)} suelos de árboles desde {nfo.name}…")
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
