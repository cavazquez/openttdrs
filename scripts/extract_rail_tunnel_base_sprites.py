#!/usr/bin/env python3
"""Extrae la base Action5 0x17 de portales ferroviarios de OpenTTD.

``SPR_RAILTYPE_TUNNEL_BASE`` vive en ``openttd.grf`` y aporta el suelo
transparente que acompaña a un portal ferroviario NewGRF. El NFO oficial
publica 16 vistas para Tropical y Temperate (8 normales + 8 nieve/desierto),
y ocho vistas para Arctic/Toyland porque esos climas no tienen una segunda
variante. Para conservar el índice de 16 slots que usa ``DrawTile_TunnelBridge``
se duplican de forma explícita las ocho vistas de estos dos últimos climas.

Se generan los PNG ``assets/opengfx/tiles/rail_tunnel_base_*`` y la tabla Rust
con las anclas NFO. El atlas se empaqueta después con ``gen_tile_atlas.py``.

Uso:
  python3 scripts/extract_rail_tunnel_base_sprites.py
  python3 scripts/extract_rail_tunnel_base_sprites.py --source-dir /ruta/a/openttd
  python3 scripts/extract_rail_tunnel_base_sprites.py --check
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
TILES = ROOT / "assets" / "opengfx" / "tiles"
OUT_RS = (
    ROOT
    / "crates"
    / "openttdrs-client"
    / "src"
    / "sprites"
    / "rail_tunnel_base_sprites_generated.rs"
)

SPR_RAILTYPE_TUNNEL_BASE = 6123
CLIMATES = ("temperate", "arctic", "tropical", "toyland")
SOURCE_LABELS = {
    "Tropical sprites.": "tropical",
    "Temperate grass + snow sprites.": "temperate",
    "Arctic grass sprites.": "arctic",
    "Toyland sprites.": "toyland",
}
EXPECTED_ROWS = {"temperate": 16, "tropical": 16, "arctic": 8, "toyland": 8}
ROW = re.compile(
    r"^\s*-1\s+sprites/tunnel_portals\.png\s+8bpp\s+"
    r"(?P<x>\d+)\s+(?P<y>\d+)\s+(?P<w>\d+)\s+(?P<h>\d+)\s+"
    r"(?P<xrel>-?\d+)\s+(?P<yrel>-?\d+)\s+normal\s*$"
)


def default_source_dir() -> Path | None:
    """Localiza el pin o la caché que prepara ``descargar_graficos.sh``."""
    configured = os.environ.get("OPENTTDRS_OPENTTD_EXTRA_DIR")
    candidates = [
        Path(configured) if configured else None,
        ROOT / "reference" / "openttd-upstream" / "media" / "baseset" / "openttd",
        ROOT / ".downloads" / "openttd" / "openttd-extra-15.3",
    ]
    return next(
        (
            candidate
            for candidate in candidates
            if candidate is not None
            and (candidate / "tunnel_portals.nfo").is_file()
            and (candidate / "tunnel_portals.png").is_file()
        ),
        None,
    )


def parse_rows(nfo: Path) -> dict[str, list[tuple[int, int, int, int, int, int]]]:
    """Lee cada bloque climático, sin depender de posiciones del PNG."""
    blocks: dict[str, list[tuple[int, int, int, int, int, int]]] = {
        climate: [] for climate in CLIMATES
    }
    current: str | None = None
    for line in nfo.read_text(encoding="utf-8", errors="replace").splitlines():
        stripped = line.strip()
        if stripped.startswith("//"):
            current = SOURCE_LABELS.get(stripped[2:].strip())
            continue
        if current is None:
            continue
        match = ROW.match(line)
        if match is None:
            continue
        blocks[current].append(
            tuple(int(match[name]) for name in ("x", "y", "w", "h", "xrel", "yrel"))
        )

    for climate, expected in EXPECTED_ROWS.items():
        actual = len(blocks[climate])
        if actual != expected:
            raise RuntimeError(
                f"{nfo}: bloque {climate} esperaba {expected} sprites Action5 0x17; "
                f"hallados {actual}"
            )
    return blocks


def normalized_rows(
    blocks: dict[str, list[tuple[int, int, int, int, int, int]]]
) -> list[list[tuple[int, int, int, int, int, int]]]:
    """Ordena LandscapeType 0..3 y completa los bloques de ocho vistas."""
    result: list[list[tuple[int, int, int, int, int, int]]] = []
    for climate in CLIMATES:
        rows = blocks[climate]
        result.append(rows if len(rows) == 16 else rows + rows)
    return result


def source_crop(source: Image.Image, rect: tuple[int, int, int, int, int, int]) -> Image.Image:
    """Recorta un sprite y vuelve transparente el índice 0 azul del GRF."""
    x, y, w, h, _xrel, _yrel = rect
    indexed = source.crop((x, y, x + w, y + h))
    rgba = indexed.convert("RGBA")
    if indexed.mode == "P":
        alpha = Image.frombytes(
            "L",
            indexed.size,
            bytes(0 if index == 0 else 255 for index in indexed.tobytes()),
        )
        rgba.putalpha(alpha)
    return rgba


def generated_rust(
    rows: list[list[tuple[int, int, int, int, int, int]]],
) -> str:
    lines = [
        "//! GENERADO por scripts/extract_rail_tunnel_base_sprites.py — no editar a mano.\n",
        "//!\n",
        "//! Fallback Action5 0x17 de `openttd.grf` (OpenTTD 15.3).\n\n",
        f"pub const SPR_RAILTYPE_TUNNEL_BASE: u32 = {SPR_RAILTYPE_TUNNEL_BASE};\n",
        "pub const RAIL_TUNNEL_BASE_CLIMATE_COUNT: usize = 4;\n",
        "pub const RAIL_TUNNEL_BASE_SPRITE_COUNT: usize = 16;\n\n",
        "/// `(w, h, xrel, yrel)` NFO por `LandscapeType` (0..3) y slot.\n",
        "pub static RAIL_TUNNEL_BASE_SPRITE_META: [[(f32, f32, f32, f32); RAIL_TUNNEL_BASE_SPRITE_COUNT];\n",
        "    RAIL_TUNNEL_BASE_CLIMATE_COUNT] = [\n",
    ]
    for climate_rows in rows:
        lines.append("    [\n")
        for _x, _y, w, h, xrel, yrel in climate_rows:
            lines.append(f"        ({w}.0, {h}.0, {xrel}.0, {yrel}.0),\n")
        lines.append("    ],\n")
    lines.extend(
        [
            "];\n\n",
            "/// ID global de un slot Action5 0x17.\n",
            "#[must_use]\n",
            "pub const fn rail_tunnel_base_sprite_id(climate: usize, slot: usize) -> Option<u32> {\n",
            "    if climate < RAIL_TUNNEL_BASE_CLIMATE_COUNT && slot < RAIL_TUNNEL_BASE_SPRITE_COUNT {\n",
            "        Some(SPR_RAILTYPE_TUNNEL_BASE + slot as u32)\n",
            "    } else {\n",
            "        None\n",
            "    }\n",
            "}\n\n",
            "/// Slot nativo: ocho vistas normales o ocho nieve/desierto; cada\n",
            "/// dirección ocupa dos sprites (rear, front).\n",
            "#[must_use]\n",
            "pub const fn rail_tunnel_base_slot(dir: u8, snow_or_desert: bool, front: bool) -> usize {\n",
            "    (if snow_or_desert { 8 } else { 0 }) + ((dir as usize & 3) * 2) + if front { 1 } else { 0 }\n",
            "}\n",
        ]
    )
    return "".join(lines)


def compare_or_write(path: Path, expected: bytes, check: bool) -> bool:
    actual = path.read_bytes() if path.is_file() else None
    if actual == expected:
        return False
    if check:
        print(f"DRIFT: {path.relative_to(ROOT)} no coincide con la fuente OpenTTD", file=sys.stderr)
        return True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)
    print(f"Escrito {path.relative_to(ROOT)}")
    return True


def image_differs(path: Path, expected: Image.Image) -> bool:
    if not path.is_file():
        return True
    with Image.open(path) as actual:
        actual_rgba = actual.convert("RGBA")
    return actual_rgba.size != expected.size or actual_rgba.tobytes() != expected.tobytes()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-dir", type=Path, help="directorio con tunnel_portals.nfo y tunnel_portals.png"
    )
    parser.add_argument("--check", action="store_true", help="verifica sin escribir")
    args = parser.parse_args(argv)

    source_dir = args.source_dir or default_source_dir()
    if source_dir is None:
        print(
            "SKIP: falta media/baseset/openttd/tunnel_portals.{nfo,png}; "
            "corré descargar_graficos.sh o prepará el pin OpenTTD",
            file=sys.stderr,
        )
        return 2
    nfo = source_dir / "tunnel_portals.nfo"
    png = source_dir / "tunnel_portals.png"
    if not nfo.is_file() or not png.is_file():
        print(f"ERROR: fuente incompleta en {source_dir}", file=sys.stderr)
        return 2

    try:
        blocks = parse_rows(nfo)
        rows = normalized_rows(blocks)
        with Image.open(png) as source:
            crops = [[source_crop(source, row) for row in climate_rows] for climate_rows in rows]
    except (OSError, RuntimeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1

    drift = compare_or_write(OUT_RS, generated_rust(rows).encode("utf-8"), args.check)

    # En CI se valida la tabla Rust desde el pin; si no hay PNG locales se
    # compara sólo metadata. Cuando la familia ya existe, --check compara los
    # 64 archivos para detectar una extracción parcial o un atlas obsoleto.
    check_tiles = not args.check or (
        TILES.is_dir() and any(TILES.glob("rail_tunnel_base_*.png"))
    )
    if check_tiles:
        for climate_index, climate in enumerate(CLIMATES):
            for slot, crop in enumerate(crops[climate_index]):
                path = TILES / f"rail_tunnel_base_{climate}_{slot:02}.png"
                if not image_differs(path, crop):
                    continue
                if args.check:
                    print(
                        f"DRIFT: {path.relative_to(ROOT)} no coincide con la fuente OpenTTD",
                        file=sys.stderr,
                    )
                    drift = True
                    continue
                path.parent.mkdir(parents=True, exist_ok=True)
                crop.save(path)
                print(f"Escrito {path.relative_to(ROOT)}")

    if args.check:
        if drift:
            return 1
        suffix = "" if check_tiles else "; PNG locales ausentes, se verificó metadata"
        print(f"OK: fallback Action5 rail tunnel (64 sprites + metadata) coincide{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
