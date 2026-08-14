#!/usr/bin/env python3
"""Recorta el set incremental de roca usado por ``DrawTile_Clear``.

OpenTTD puede habilitar una segunda base con el misc bit
``SecondRockyTileSet`` y después suma ``SlopeToSpriteOffset``. OpenGFX y
OpenGFX2 usan la primera serie, pero el renderer conserva ambas variantes
completas de 19 sprites para no degradar una pendiente al cambiar de baseset.

Uso:
  python3 scripts/crop_clear_land_sprites.py
  python3 scripts/crop_clear_land_sprites.py --force

Luego ejecutá ``python3 scripts/gen_tile_atlas.py``.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from crop_missing_industry_pngs import load_sheets, opengfx_paths
from nfo_sprite_meta import parse_global_sprite_rects


ROCKY_BASES = (4023, 4042)
SPRITES: tuple[tuple[int, str], ...] = tuple(
    (base + offset, f"terrain_rocky_{variant}_{offset:02}.png")
    for variant, base in enumerate(ROCKY_BASES, start=1)
    for offset in range(19)
)


def crop_clear_land_sprites(repo: Path, *, force: bool) -> tuple[int, list[str]]:
    """Recorta las variantes pendientes y devuelve ``(cantidad, fallos)``.

    ``parse_global_sprite_rects`` conserva la continuación ``|`` de OpenGFX2
    y elige su fila ``normal`` 32bpp; el parser lineal histórico de industria
    no puede hacerlo porque sólo ve las filas que comienzan con un ID.
    """
    tiles = repo / "assets" / "opengfx" / "tiles"
    sprites_dir, nfo, prefix, mode = opengfx_paths(repo)
    if not nfo.is_file():
        raise FileNotFoundError(f"Falta NFO: {nfo}")

    todo = [(sid, name) for sid, name in SPRITES if force or not (tiles / name).is_file()]
    rects = parse_global_sprite_rects(nfo, mode)
    sheets = load_sheets(sprites_dir, prefix, mode)
    failures: list[str] = []
    for sid, name in todo:
        rect = rects.get(sid)
        if rect is None:
            failures.append(f"{name}: no_nfo")
            continue
        sheet = rect.sheet
        if sheet not in sheets:
            alt = Path(sheet).with_suffix(".pcx").name
            sheet = alt if alt in sheets else ""
        if not sheet:
            failures.append(f"{name}: no_sheet")
            continue
        crop = sheets[sheet].crop((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h))
        out = tiles / name
        out.parent.mkdir(parents=True, exist_ok=True)
        crop.save(out)
    return len(todo), failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="reemplaza PNG existentes")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    try:
        count, failures = crop_clear_land_sprites(repo, force=args.force)
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 1
    if count == 0:
        print(f"Las {len(SPRITES)} variantes rocosas ya existen.")
        return 0
    mode = (repo / "assets" / "opengfx" / ".graphics_mode").read_text(encoding="utf-8").strip()
    print(f"Modo {mode}; recortando {count} sprites rocosos…")
    if failures:
        print("No se pudieron recortar: " + ", ".join(failures), file=sys.stderr)
        return 1
    print("Listo. Ejecutá python3 scripts/gen_tile_atlas.py para incluirlos en el atlas.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
