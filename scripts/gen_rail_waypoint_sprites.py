#!/usr/bin/env python3
"""Extrae los postes de waypoint ferroviario del GRF extra de OpenGFX.

Sprites en `table/sprites.h` (`SPR_OPENTTD_BASE` = 4896):
- SPR_WAYPOINT_X_1 = 4974, SPR_WAYPOINT_X_2 = 4975
- SPR_WAYPOINT_Y_1 = 4976, SPR_WAYPOINT_Y_2 = 4977

Secuencias en `station_land.h` (`_station_display_datas_waypoint_X/Y`).

Salida: `assets/opengfx/tiles/rail_{4974..4977}.png`

Uso: python3 scripts/gen_rail_waypoint_sprites.py
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"

WAYPOINT_SPRITES = [
    (4974, "rail_waypoint_x_1.png"),
    (4975, "rail_waypoint_x_2.png"),
    (4976, "rail_waypoint_y_1.png"),
    (4977, "rail_waypoint_y_2.png"),
]

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def find_extra_nfo() -> Path:
    for sprites_dir in (REPO / "assets" / "opengfx").glob("*/sprites"):
        for nfo in sprites_dir.glob("*extra*.nfo"):
            return nfo
        for grf in sprites_dir.parent.glob("*extra*.grf"):
            print(f"Decodificando {grf.name} con grfcodec...")
            subprocess.run(
                ["grfcodec", "-d", "-o", "png", grf.name, "sprites/"],
                cwd=grf.parent,
                check=True,
                stdout=subprocess.DEVNULL,
            )
            for nfo in sprites_dir.glob("*extra*.nfo"):
                return nfo
    sys.exit("No se encontró el GRF extra de OpenGFX (corré descargar_graficos.sh)")


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    sprites_dir = nfo.parent
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in rows:
            rows[sid] = (
                (sprites_dir / Path(m.group(2)).name).as_posix(),
                int(m.group(3)),
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
            )
    return rows


def _key_color_transparent(img_rgba: Image.Image, key_rgb: tuple[int, int, int]) -> None:
    """Convierte un color clave (índice 0 de paleta o azul 8bpp) en alpha 0."""
    keyed = [
        (0, 0, 0, 0) if px[:3] == key_rgb else px for px in img_rgba.get_flattened_data()
    ]
    img_rgba.putdata(keyed)


def load_sheet(path: Path, mode: str) -> Image.Image:
    img = Image.open(path)
    if img.mode == "P":
        pal = img.getpalette()
        transparent_rgb = tuple(pal[0:3]) if pal else None
        img_rgba = img.convert("RGBA")
        if transparent_rgb is not None:
            _key_color_transparent(img_rgba, transparent_rgb)
        return img_rgba
    img_rgba = img.convert("RGBA")
    if mode != "32bpp":
        _key_color_transparent(img_rgba, (0, 0, 255))
    return img_rgba


def crop(rows: dict[int, tuple[str, int, int, int, int]], sid: int, out_name: str) -> None:
    if sid not in rows:
        raise SystemExit(f"sprite {sid} no encontrado en NFO extra")
    sheet_path, x, y, w, h = rows[sid]
    sheet = load_sheet(Path(sheet_path), detect_graphics_mode(REPO) or "8bpp")
    crop_img = sheet.crop((x, y, x + w, y + h))
    TILES.mkdir(parents=True, exist_ok=True)
    crop_img.save(TILES / out_name)
    rail_alias = TILES / f"rail_{sid}.png"
    crop_img.save(rail_alias)
    print(f"  rail_{sid}.png <- {Path(sheet_path).name} ({w}x{h})")


def main() -> None:
    nfo = find_extra_nfo()
    rows = parse_rows(nfo)
    last_real: Path | None = None
    for sid, _name in WAYPOINT_SPRITES:
        if sid in rows:
            crop(rows, sid, f"rail_{sid}.png")
            last_real = TILES / f"rail_{sid}.png"
            continue
        # 4977 suele ser pseudo-sprite de recolor (`4977 * 8 …`) sin hoja propia.
        if last_real is not None:
            dst = TILES / f"rail_{sid}.png"
            shutil.copy2(last_real, dst)
            print(f"  rail_{sid}.png <- {last_real.name} (pseudo-sprite NFO, fallback)")
            continue
        raise SystemExit(f"sprite {sid} no encontrado en NFO extra")
    print(f"Waypoints listos en {TILES}/")


if __name__ == "__main__":
    main()
