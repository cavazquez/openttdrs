#!/usr/bin/env python3
"""Hornea los overlays PBS con el remapeo exacto ``PALETTE_CRASH``.

OpenTTD no pinta una reserva PBS mezclando un color naranja sobre el sprite.
En gráficos 8bpp aplica la pseudo-sprite de recolor 804 a cada índice de la
imagen.  Esa tabla reduce el sprite a la escala gris de una reserva; dos
índices con el mismo RGB pueden mapear a destinos distintos, por lo que el
remapeo tiene que suceder antes de perder los índices paletizados.

Este script genera ``rail_pbs_<id>.png`` para los ``SINGLE_*`` rail/mono/
maglev y para los doce overlays inclinados del GRF extra. El renderer los usa
solamente cuando OpenTTD pide ``PALETTE_CRASH``.

Uso::

  python3 scripts/extract_rail_pbs_palette_sprites.py
  python3 scripts/extract_rail_pbs_palette_sprites.py --check
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from PIL import Image

from extract_bridge_pbs_reservation_sprites import parse_action5_slope_rects
from opengfx_palette import UNUSED_TRANSPARENT_INDICES, dos_palette

ROOT = Path(__file__).resolve().parents[1]
OPENGFX = ROOT / "assets" / "opengfx"
TILES = OPENGFX / "tiles"
PALETTE_CRASH = 804

SINGLE_IDS = tuple(range(1005, 1011)) + tuple(range(1087, 1093)) + tuple(range(1169, 1175))
SLOPE_IDS = tuple(range(5401, 5413))
RAIL_PBS_SPRITE_IDS = SINGLE_IDS + SLOPE_IDS

ENTRY_RE = re.compile(
    r"^\s*(?P<sprite_id>\d+)\s+(?P<sheet>\S+?\.(?:png|pcx))\s+"
    r"8bpp\s+(?P<x>\d+)\s+(?P<y>\d+)\s+(?P<w>\d+)\s+(?P<h>\d+)\s+-?\d+\s+-?\d+"
)
PSEUDO_HEADER_RE = re.compile(r"^\s*(?P<id>\d+)\s+\*\s+(?P<size>\d+)\b")
NORMAL_SPRITE_RE = re.compile(r"^\s*\d+\s+\S+?\.(?:png|pcx)\b")


def source_dirs() -> list[Path]:
    """Directorios 8bpp, activo primero y side-cache como respaldo."""
    active = sorted(OPENGFX.glob("opengfx-*/sprites"), reverse=True)
    side = OPENGFX / ".signal-src-8bpp" / "sprites"
    return [*active, side]


def find_source(name: str) -> tuple[Path, Path]:
    """Encuentra NFO y directorio de una fuente clásica 8bpp."""
    for directory in source_dirs():
        nfo = directory / name
        if nfo.is_file():
            return directory, nfo
    raise FileNotFoundError(
        f"falta {name} 8bpp; ejecutá scripts/descargar_graficos.sh --8bpp primero"
    )


def _decode_nfo_bytes(blob: str, expected: int) -> list[int]:
    """Decodifica bytes hexadecimales y cadenas latin-1 de una pseudo-sprite."""
    data: list[int] = []
    i = 0
    while i < len(blob) and len(data) < expected:
        while i < len(blob) and blob[i].isspace():
            i += 1
        if i >= len(blob):
            break
        if blob[i] == '"':
            i += 1
            while i < len(blob):
                char = blob[i]
                if char == "\\" and i + 1 < len(blob):
                    data.append(ord(blob[i + 1]))
                    i += 2
                elif char == '"':
                    i += 1
                    break
                else:
                    data.append(ord(char))
                    i += 1
                if len(data) == expected:
                    break
            continue
        match = re.match(r"[0-9A-Fa-f]{2}", blob[i:])
        if match is not None:
            data.append(int(match.group(), 16))
            i += 2
        else:
            i += 1
    if len(data) != expected:
        raise ValueError(f"pseudo-sprite incompleta: {len(data)}/{expected} bytes")
    return data


def parse_recolour_table(lines: list[str], sprite_id: int) -> tuple[int, ...]:
    """Devuelve los 256 destinos de una pseudo-sprite de recolor.

    La primera palabra del bloque es el byte de acción de NewGRF; C++ hace
    ``GetNonSprite(...)+1``, por eso se descarta exactamente ese byte aquí.
    """
    start = next(
        (
            index
            for index, line in enumerate(lines)
            if (match := PSEUDO_HEADER_RE.match(line))
            and int(match.group("id")) == sprite_id
            and int(match.group("size")) == 257
        ),
        None,
    )
    if start is None:
        raise ValueError(f"no encontré pseudo-sprite {sprite_id} (*257)")

    chunk = [lines[start]]
    for line in lines[start + 1 :]:
        if PSEUDO_HEADER_RE.match(line) or NORMAL_SPRITE_RE.match(line):
            break
        chunk.append(line)
    header = PSEUDO_HEADER_RE.match(chunk[0])
    assert header is not None
    payload = chunk[0][header.end() :] + "\n" + "\n".join(chunk[1:])
    raw = _decode_nfo_bytes(payload, 257)
    if raw[0] != 0:
        raise ValueError(f"pseudo-sprite {sprite_id} no empieza con acción 00")
    return tuple(raw[1:])


def parse_base_rects(nfo_path: Path) -> dict[int, tuple[str, int, int, int, int]]:
    """Rectángulos 8bpp para los SINGLE_* que usa PBS."""
    rects: dict[int, tuple[str, int, int, int, int]] = {}
    wanted = set(SINGLE_IDS)
    for line in nfo_path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = ENTRY_RE.match(line)
        if match is None:
            continue
        sprite_id = int(match.group("sprite_id"))
        if sprite_id not in wanted:
            continue
        rects[sprite_id] = (
            Path(match.group("sheet")).name,
            int(match.group("x")),
            int(match.group("y")),
            int(match.group("w")),
            int(match.group("h")),
        )
    return rects


def remap_indexed_crash(
    image: Image.Image,
    table: tuple[int, ...],
    palette: tuple[tuple[int, int, int], ...] | None = None,
) -> Image.Image:
    """Aplica ``PALETTE_CRASH`` sobre una imagen ``P`` sin inferir por RGB."""
    if image.mode != "P":
        raise ValueError(f"PALETTE_CRASH requiere imagen indexada P, no {image.mode}")
    if len(table) != 256:
        raise ValueError(f"tabla de recolor inválida: {len(table)} entradas")
    palette = dos_palette() if palette is None else palette
    pixels: list[tuple[int, int, int, int]] = []
    for source in image.get_flattened_data():
        if source == 0 or source in UNUSED_TRANSPARENT_INDICES:
            pixels.append((0, 0, 0, 0))
            continue
        red, green, blue = palette[table[source]]
        pixels.append((red, green, blue, 255))
    out = Image.new("RGBA", image.size)
    out.putdata(pixels)
    return out


def source_crops() -> dict[int, Image.Image]:
    """Carga los 30 recortes indexados que puede emitir PBS."""
    base_dir, base_nfo = find_source("ogfx1_base.nfo")
    extra_dir, extra_nfo = find_source("ogfxe_extra.nfo")
    base_rects = parse_base_rects(base_nfo)
    slope_rects = parse_action5_slope_rects(extra_nfo, "8bpp")
    rects = {**base_rects, **slope_rects}
    missing = [sid for sid in RAIL_PBS_SPRITE_IDS if sid not in rects]
    if missing:
        raise ValueError(f"faltan rectángulos PBS: {missing}")

    sheets: dict[tuple[Path, str], Image.Image] = {}
    crops: dict[int, Image.Image] = {}
    for sid in RAIL_PBS_SPRITE_IDS:
        sheet_name, x, y, width, height = rects[sid]
        directory = base_dir if sid in SINGLE_IDS else extra_dir
        key = (directory, sheet_name)
        sheet = sheets.get(key)
        if sheet is None:
            path = directory / sheet_name
            if not path.is_file():
                raise FileNotFoundError(f"falta hoja PBS {path.relative_to(ROOT)}")
            sheet = Image.open(path)
            if sheet.mode != "P":
                raise ValueError(f"{path.relative_to(ROOT)} no es 8bpp indexado ({sheet.mode})")
            sheets[key] = sheet
        crops[sid] = sheet.crop((x, y, x + width, y + height))
    return crops


def build_outputs() -> dict[Path, Image.Image]:
    """Genera en memoria cada PNG derivado para testearlo con ``--check``."""
    _base_dir, base_nfo = find_source("ogfx1_base.nfo")
    lines = base_nfo.read_bytes().decode("latin-1").splitlines()
    table = parse_recolour_table(lines, PALETTE_CRASH)
    return {
        TILES / f"rail_pbs_{sid}.png": remap_indexed_crash(crop, table)
        for sid, crop in source_crops().items()
    }


def image_matches(path: Path, expected: Image.Image) -> bool:
    if not path.is_file():
        return False
    actual = Image.open(path).convert("RGBA")
    return actual.size == expected.size and actual.tobytes() == expected.tobytes()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verifica PNGs sin escribir")
    args = parser.parse_args(argv)
    try:
        outputs = build_outputs()
    except (FileNotFoundError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    if args.check:
        stale = [path.name for path, image in outputs.items() if not image_matches(path, image)]
        if stale:
            print("DRIFT: " + ", ".join(stale), file=sys.stderr)
            print("  Regenerá con: python3 scripts/extract_rail_pbs_palette_sprites.py", file=sys.stderr)
            return 1
        print(f"OK: {len(outputs)} overlays PBS coinciden con PALETTE_CRASH={PALETTE_CRASH}")
        return 0

    TILES.mkdir(parents=True, exist_ok=True)
    for path, image in outputs.items():
        image.save(path)
    print(f"  rail PBS: {len(outputs)} overlays con PALETTE_CRASH={PALETTE_CRASH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
