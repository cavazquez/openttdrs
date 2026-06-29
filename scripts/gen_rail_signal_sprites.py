#!/usr/bin/env python3
"""Extrae sprites de señal ferroviaria (1275–1699) desde los NFO de OpenGFX.

En OpenGFX2 el mismo ID puede aparecer en `ogfx21_base` y en `ogfx2e_extra` con
recortes distintos; el flujo principal de `descargar_graficos.sh` solo lee el NFO
base y a veces deja PNG diminutos (p. ej. `rail_1275.png` 3×14). Este script elige,
por cada ID, el rectángulo de mayor área entre todos los NFO del set.

Salida: `assets/opengfx/tiles/rail_{id}.png`

Uso: python3 scripts/gen_rail_signal_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
SIGNAL_RANGE = range(1275, 1700)

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def find_sprite_dirs() -> list[Path]:
    out: list[Path] = []
    for sprites_dir in sorted((REPO / "assets" / "opengfx").glob("*/sprites")):
        if any(sprites_dir.glob("*.nfo")):
            out.append(sprites_dir)
    return out


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    sprites_dir = nfo.parent
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in rows:
            sheet = (sprites_dir / Path(m.group(2)).name).as_posix()
            rows[sid] = (
                sheet,
                int(m.group(3)),
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
            )
    return rows


def merge_signal_rows(dirs: list[Path]) -> dict[int, tuple[str, int, int, int, int]]:
    merged: dict[int, tuple[str, int, int, int, int]] = {}
    for sprites_dir in dirs:
        for nfo in sorted(sprites_dir.glob("*.nfo")):
            for sid, row in parse_rows(nfo).items():
                if sid not in SIGNAL_RANGE:
                    continue
                _sheet, _x, _y, w, h = row
                area = w * h
                if sid not in merged:
                    merged[sid] = row
                    continue
                _ms, _mx, _my, mw, mh = merged[sid]
                if area > mw * mh:
                    merged[sid] = row
    return merged


def _key_color_transparent(img_rgba: Image.Image, key_rgb: tuple[int, int, int]) -> None:
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


def dematte_cc_blue_mask(img: Image.Image) -> Image.Image:
    src = img.convert("RGBA")
    data = []
    for r, g, b, a in src.get_flattened_data():
        if a > 0 and r == 0 and g == 0 and b == 255:
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    src.putdata(data)
    return src


def crop_and_save(
    rows: dict[int, tuple[str, int, int, int, int]],
    sid: int,
    mode: str,
    sheet_cache: dict[str, Image.Image],
) -> bool:
    if sid not in rows:
        return False
    sheet_path, x, y, w, h = rows[sid]
    if w <= 0 or h <= 0:
        return False
    if sheet_path not in sheet_cache:
        sheet_cache[sheet_path] = load_sheet(Path(sheet_path), mode)
    sheet = sheet_cache[sheet_path]
    crop_img = dematte_cc_blue_mask(sheet.crop((x, y, x + w, y + h)))
    TILES.mkdir(parents=True, exist_ok=True)
    out = TILES / f"rail_{sid}.png"
    crop_img.save(out)
    return True


def main() -> None:
    dirs = find_sprite_dirs()
    if not dirs:
        sys.exit("No hay carpetas sprites/ en assets/opengfx (corré descargar_graficos.sh)")
    mode = detect_graphics_mode(REPO) or "8bpp"
    rows = merge_signal_rows(dirs)
    sheet_cache: dict[str, Image.Image] = {}
    ok = 0
    for sid in SIGNAL_RANGE:
        if crop_and_save(rows, sid, mode, sheet_cache):
            ok += 1
    print(f"Señales listas: {ok}/{len(SIGNAL_RANGE)} en {TILES}/")


if __name__ == "__main__":
    main()
