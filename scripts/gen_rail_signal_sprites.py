#!/usr/bin/env python3
"""Extrae sprites de señal ferroviaria (1275–1699) desde OpenGFX2 32bpp.

OpenGFX2 guarda muchas señales en `ogfx2e_extra` como máscaras CC (~7×13) que
OpenTTD recolorea en runtime. Este script las hornea a rojo/verde en el PNG.

Cuando el extra no define una máscara, usa el recorte pequeño y colorido del NFO
base (`ogfx21_base`), rechazando reutilizaciones grandes (p. ej. topadora 64×31).

No requiere OpenGFX 8bpp.

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

MAX_SIGNAL_W = 32
MAX_SIGNAL_H = 32
MIN_COLORFUL_COLORS = 4
EXTRA_MASK_MAX_W = 8
EXTRA_MASK_MAX_H = 16
# IDs del NFO base reutilizados para otro gráfico (p. ej. topadora en 1416–1419).
BASE_ID_BLACKLIST = frozenset({1416, 1417, 1418, 1419, 1420})
# Bloque eléctrico clásico (`SPR_ORIGINAL_SIGNALS_BASE`): siempre usar base colorido.
MAIN_SIGNAL_COLOR_RANGE = range(1275, 1291)
# OpenGFX2 reutiliza 1416–1419 en base; en juego variant=1 → mapear a 1275–1278.
ELECTRIC_CLASSIC_ALIASES: dict[int, int] = {1416 + i: 1275 + i for i in range(4)}

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)

SignalRow = tuple[str, int, int, int, int, int, int]


def find_sprite_dirs() -> list[Path]:
    out: list[Path] = []
    for sprites_dir in sorted((REPO / "assets" / "opengfx").glob("*/sprites")):
        if any(sprites_dir.glob("*.nfo")):
            out.append(sprites_dir)
    return out


def parse_rows(nfo: Path) -> dict[int, SignalRow]:
    sprites_dir = nfo.parent
    rows: dict[int, SignalRow] = {}
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
                int(m.group(7)),
                int(m.group(8)),
            )
    return rows


def nfo_source_priority(nfo: Path) -> int:
    name = nfo.name.lower()
    if "extra" in name:
        return 2
    if "base" in name:
        return 1
    return 0


def is_magenta_key(r: int, g: int, b: int) -> bool:
    return r > 200 and b > 200 and g < 80


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


def dematte_sprite(img: Image.Image) -> Image.Image:
    """Quita azul CC, magenta de remapeo y otros colorkeys típicos de OpenGFX."""
    src = img.convert("RGBA")
    data = []
    for r, g, b, a in src.get_flattened_data():
        if a == 0:
            data.append((0, 0, 0, 0))
        elif (r, g, b) == (0, 0, 255) or is_magenta_key(r, g, b):
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    src.putdata(data)
    return src


def opaque_color_count(img: Image.Image) -> int:
    return len({px for px in img.get_flattened_data() if px[3] > 0})


def recolor_cc_signal_mask(img: Image.Image, sid: int) -> Image.Image:
    """Tinta máscaras CC según offset rojo/verde en el bloque 1275 o 1352."""
    base = 1275 if sid < 1352 else 1352
    is_green = (sid - base) % 2 == 1
    color = (40, 200, 60, 255) if is_green else (220, 30, 30, 255)
    src = img.convert("RGBA")
    data = [(color if a > 0 else (0, 0, 0, 0)) for _r, _g, _b, a in src.get_flattened_data()]
    src.putdata(data)
    return src


def is_extra_cc_mask(w: int, h: int, pri: int) -> bool:
    return pri == 2 and w <= EXTRA_MASK_MAX_W and h <= EXTRA_MASK_MAX_H


def collect_candidates(
    dirs: list[Path],
) -> dict[int, list[tuple[int, SignalRow]]]:
    by_id: dict[int, list[tuple[int, SignalRow]]] = {}
    for sprites_dir in dirs:
        for nfo in sorted(sprites_dir.glob("*.nfo")):
            pri = nfo_source_priority(nfo)
            for sid, row in parse_rows(nfo).items():
                if sid not in SIGNAL_RANGE:
                    continue
                by_id.setdefault(sid, []).append((pri, row))
    return by_id


def pick_signal_row(
    sid: int,
    rows: list[tuple[int, SignalRow]],
    mode: str,
    sheet_cache: dict[str, Image.Image],
) -> tuple[SignalRow, str] | None:
    """Base colorido (3×14…) > máscara extra CC; rechaza vehículos 64×31."""
    extra_masks: list[tuple[int, SignalRow]] = []
    colorful: list[tuple[int, int, SignalRow]] = []

    for pri, row in rows:
        sheet_path, x, y, w, h, _xrel, _yrel = row
        if w <= 0 or h <= 0:
            continue
        if sheet_path not in sheet_cache:
            sheet_cache[sheet_path] = load_sheet(Path(sheet_path), mode)
        crop = dematte_sprite(sheet_cache[sheet_path].crop((x, y, x + w, y + h)))
        nc = opaque_color_count(crop)
        if is_extra_cc_mask(w, h, pri) and nc >= 1:
            extra_masks.append((w * h, row))
        elif (
            pri == 1
            and w <= MAX_SIGNAL_W
            and h <= MAX_SIGNAL_H
            and w < 48
            and h < 40
            and nc >= MIN_COLORFUL_COLORS
        ):
            colorful.append((nc, w * h, row))

    def best_colorful() -> tuple[SignalRow, str] | None:
        if not colorful:
            return None
        _nc, _area, row = max(colorful, key=lambda item: (item[0], item[1]))
        return row, "color"

    def best_extra() -> tuple[SignalRow, str] | None:
        if not extra_masks:
            return None
        _area, row = max(extra_masks, key=lambda item: item[0])
        return row, "cc_mask"

    if sid in MAIN_SIGNAL_COLOR_RANGE:
        return best_colorful() or best_extra()

    if sid in BASE_ID_BLACKLIST:
        return best_extra() or best_colorful()

    return best_colorful() or best_extra()


def merge_signal_rows(
    dirs: list[Path],
    mode: str,
    sheet_cache: dict[str, Image.Image],
) -> dict[int, tuple[SignalRow, str]]:
    merged: dict[int, tuple[SignalRow, str]] = {}
    by_id = collect_candidates(dirs)
    for sid in SIGNAL_RANGE:
        rows = by_id.get(sid, [])
        if not rows:
            continue
        picked = pick_signal_row(sid, rows, mode, sheet_cache)
        if picked is not None:
            merged[sid] = picked
    return merged


def crop_and_save(
    merged: dict[int, tuple[SignalRow, str]],
    sid: int,
    mode: str,
    sheet_cache: dict[str, Image.Image],
) -> bool:
    if sid not in merged:
        return False
    row, kind = merged[sid]
    sheet_path, x, y, w, h, _xrel, _yrel = row
    if sheet_path not in sheet_cache:
        sheet_cache[sheet_path] = load_sheet(Path(sheet_path), mode)
    crop_img = dematte_sprite(sheet_cache[sheet_path].crop((x, y, x + w, y + h)))
    if kind == "cc_mask":
        crop_img = recolor_cc_signal_mask(crop_img, sid)
    TILES.mkdir(parents=True, exist_ok=True)
    crop_img.save(TILES / f"rail_{sid}.png")
    return True


def apply_electric_classic_aliases() -> None:
    """Sustituye PNG rotos del bloque eléctrico (1416/1419) por 1275/1278."""
    import shutil

    for dst, src in ELECTRIC_CLASSIC_ALIASES.items():
        src_path = TILES / f"rail_{src}.png"
        dst_path = TILES / f"rail_{dst}.png"
        if not src_path.is_file():
            sys.exit(f"Falta {src_path.name} para alias eléctrico → rail_{dst}.png")
        shutil.copy2(src_path, dst_path)


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    dirs = find_sprite_dirs()
    if not dirs:
        sys.exit("No hay carpetas sprites/ en assets/opengfx (corré descargar_graficos.sh --32bpp)")
    sheet_cache: dict[str, Image.Image] = {}
    merged = merge_signal_rows(dirs, mode, sheet_cache)
    ok = 0
    for sid in SIGNAL_RANGE:
        if crop_and_save(merged, sid, mode, sheet_cache):
            ok += 1

    apply_electric_classic_aliases()

    for sid in (1275, 1276, 1416, 1417, 1418, 1419):
        path = TILES / f"rail_{sid}.png"
        if not path.is_file():
            sys.exit(f"Falta {path.name} tras exportar señales")
        im = Image.open(path).convert("RGBA")
        w, h = im.size
        if w > MAX_SIGNAL_W or h > MAX_SIGNAL_H:
            sys.exit(f"{path.name} mide {w}x{h}: recorte incorrecto (¿topadora/vehículo?)")
        nc = opaque_color_count(im)
        if nc < 1:
            sys.exit(f"{path.name} está vacío")
        for r, g, b, a in im.get_flattened_data():
            if a > 0 and is_magenta_key(r, g, b):
                sys.exit(f"{path.name} tiene magenta CC sin horneado")
        if sid in (1275, 1276, 1416, 1417, 1418, 1419) and (w > 6 or h > 16):
            sys.exit(f"{path.name} mide {w}x{h}: esperaba señal 3×14, no topadora/máscara CC")

    print(f"Señales listas: {ok}/{len(SIGNAL_RANGE)} en {TILES}/ (fuente: OpenGFX2 32bpp)")


if __name__ == "__main__":
    main()
