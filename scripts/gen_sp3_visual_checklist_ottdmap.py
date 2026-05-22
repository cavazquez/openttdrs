#!/usr/bin/env python3
"""Genera el fixture MAP1 para capturas manuales SP3.0 / SP3.1.

Salida: `crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap`

Cada escena va separada por **al menos 1 tesela de hierba** para distinguirlas en capturas.

Layout (20×12, origen arriba-izquierda):

```
y=3   · RY · RX · RT · cruce · cruce nivel X/Y · · tranvía X ·
y=5   · vía Y · vía X · T · cruce · señales · nieve ·
y=7   · carretera NE · SE · SW · NW · tranvía en pendiente NE (SP3.1) ·
y=9   · casa · camión · bus · tren · industria ·
y=11  · hierba · mar Clear · costa · hierba ·
```

Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`
"""

from __future__ import annotations

import struct
from dataclasses import dataclass, replace
from pathlib import Path

MP_CLEAR = 0
MP_RAILWAY = 1
MP_ROAD = 2
MP_STATION = 5
MP_WATER = 6
MP_HOUSE = 3
MP_INDUSTRY = 8

FORMAT_VERSION = 1
FLAG_HAS_M2_HI = 1
# MAP1 v1: magic(4) + width(4) + height(4) + format_version(2) + flags(2) = 16 bytes antes del bloque denso.
DENSE_OFFSET = 16

DEFAULT_H = 4


@dataclass
class TileSpec:
    tt: int = MP_CLEAR
    height: int = DEFAULT_H
    m1: int = 0
    m2: int = 0
    m2_hi: int = 0
    m3: int = 0
    m3hi: int = 0
    m5: int = 0
    m6: int = 0
    m7: int = 0
    m8: int = 0


def mapt_byte(tile_type: int) -> int:
    return (tile_type & 0xF) << 4


def set_height(tiles: dict[tuple[int, int], TileSpec], x: int, y: int, h: int) -> None:
    cur = tiles.get((x, y), TileSpec())
    tiles[(x, y)] = replace(cur, height=h & 0xFF)


def apply_ne_slope(tiles: dict[tuple[int, int], TileSpec], tx: int, ty: int, base: int = DEFAULT_H) -> None:
    """`tileh` 12 (SLOPE_NE) en la tesela (tx, ty)."""
    set_height(tiles, tx, ty, base + 1)
    set_height(tiles, tx, ty + 1, base + 1)
    set_height(tiles, tx + 1, ty, base)
    set_height(tiles, tx + 1, ty + 1, base)


def apply_se_slope(tiles: dict[tuple[int, int], TileSpec], tx: int, ty: int, base: int = DEFAULT_H) -> None:
    """`tileh` 6 (SLOPE_SE): sur + este elevados."""
    set_height(tiles, tx, ty, base)
    set_height(tiles, tx, ty + 1, base + 1)
    set_height(tiles, tx + 1, ty, base)
    set_height(tiles, tx + 1, ty + 1, base + 1)


def apply_sw_slope(tiles: dict[tuple[int, int], TileSpec], tx: int, ty: int, base: int = DEFAULT_H) -> None:
    """`tileh` 3 (SLOPE_SW): oeste + sur elevados."""
    set_height(tiles, tx, ty, base)
    set_height(tiles, tx, ty + 1, base)
    set_height(tiles, tx + 1, ty, base + 1)
    set_height(tiles, tx + 1, ty + 1, base + 1)


def apply_nw_slope(tiles: dict[tuple[int, int], TileSpec], tx: int, ty: int, base: int = DEFAULT_H) -> None:
    """`tileh` 9 (SLOPE_NW)."""
    set_height(tiles, tx, ty, base + 1)
    set_height(tiles, tx, ty + 1, base)
    set_height(tiles, tx + 1, ty, base + 1)
    set_height(tiles, tx + 1, ty + 1, base)


def put(tiles: dict[tuple[int, int], TileSpec], x: int, y: int, spec: TileSpec) -> None:
    tiles[(x, y)] = spec


def build_stxy_footer(tile_types: list[int], dim_x: int, dim_y: int) -> bytes:
    coords: list[tuple[int, int]] = []
    for i, mapt in enumerate(tile_types):
        if ((mapt >> 4) & 0xF) == MP_STATION:
            coords.append((i % dim_x, i // dim_x))
    parts = [b"STXY", struct.pack("<I", len(coords))]
    for x, y in coords:
        parts.append(struct.pack("<HH", x & 0xFFFF, y & 0xFFFF))
    return b"".join(parts)


def build_map1(
    width: int,
    height: int,
    tiles: dict[tuple[int, int], TileSpec],
) -> bytes:
    default = TileSpec()
    mapt: list[int] = []
    heights: list[int] = []
    m1: list[int] = []
    m2: list[int] = []
    m2_hi: list[int] = []
    m3: list[int] = []
    m3hi: list[int] = []
    m5: list[int] = []
    m6: list[int] = []
    m7: list[int] = []
    m8: list[int] = []

    for y in range(height):
        for x in range(width):
            t = tiles.get((x, y), default)
            mapt.append(mapt_byte(t.tt))
            heights.append(t.height & 0xFF)
            m1.append(t.m1 & 0xFF)
            m2.append(t.m2 & 0xFF)
            m2_hi.append(t.m2_hi & 0xFF)
            m3.append(t.m3 & 0xFF)
            m3hi.append(t.m3hi & 0xFF)
            m5.append(t.m5 & 0xFF)
            m6.append(t.m6 & 0xFF)
            m7.append(t.m7 & 0xFF)
            m8.append(t.m8 & 0xFFFF)

    body = bytearray()
    body.extend(b"MAP1")
    body.extend(struct.pack("<IIHH", width, height, FORMAT_VERSION, FLAG_HAS_M2_HI))
    body.extend(mapt)
    body.extend(heights)
    body.extend(m1)
    body.extend(m2)
    body.extend(m2_hi)
    body.extend(m3)
    body.extend(m3hi)
    body.extend(m5)
    body.extend(m6)
    body.extend(m7)
    for v in m8:
        body.extend(struct.pack("<H", v))
    body.extend(build_stxy_footer(mapt, width, height))
    return bytes(body)


def main() -> None:
    w, h = 20, 12
    tiles: dict[tuple[int, int], TileSpec] = {}

    # --- Fila carretera plana (y=3), paso 2 en x ---
    put(tiles, 1, 3, TileSpec(tt=MP_ROAD, m5=0x05))  # ROAD_Y
    put(tiles, 3, 3, TileSpec(tt=MP_ROAD, m5=0x0A))  # ROAD_X
    put(tiles, 5, 3, TileSpec(tt=MP_ROAD, m5=0x07))  # T
    put(tiles, 7, 3, TileSpec(tt=MP_ROAD, m5=0x0F))  # cruce
    put(tiles, 9, 3, TileSpec(tt=MP_ROAD, m5=0x40))  # cruce nivel eje X
    put(tiles, 11, 3, TileSpec(tt=MP_ROAD, m5=0x41))  # cruce nivel eje Y
    # Tranvía: misma máscara en m5 (carretera) y m3 (vía tranvía) — eje X (0x0A).
    put(tiles, 15, 3, TileSpec(tt=MP_ROAD, m5=0x0A, m3=0x0A))

    # --- Fila vía plana (y=5) ---
    put(tiles, 1, 5, TileSpec(tt=MP_RAILWAY, m5=0x02))
    put(tiles, 3, 5, TileSpec(tt=MP_RAILWAY, m5=0x01))
    put(tiles, 5, 5, TileSpec(tt=MP_RAILWAY, m5=0x07))
    put(tiles, 7, 5, TileSpec(tt=MP_RAILWAY, m5=0x03))
    put(
        tiles,
        9,
        5,
        TileSpec(tt=MP_RAILWAY, m5=(1 << 6) | 0x02, m3=0xC0, m3hi=0x80),
    )
    put(tiles, 11, 5, TileSpec(tt=MP_RAILWAY, m5=0x02, m3=0x0C))

    # --- SP3.1: carretera en pendiente (y=7), paso 3 en x (alturas antes del tipo) ---
    for tx, ty, slope_fn, m5 in [
        (1, 7, apply_ne_slope, 0x05),
        (4, 7, apply_se_slope, 0x0A),
        (7, 7, apply_sw_slope, 0x03),
        (10, 7, apply_nw_slope, 0x0F),
    ]:
        slope_fn(tiles, tx, ty)
        cur = tiles.get((tx, ty), TileSpec())
        tiles[(tx, ty)] = replace(cur, tt=MP_ROAD, m5=m5)

    # Tranvía en pendiente NE (mismo índice road_flat_11 / tram_flat_11; m5 y m3 alineados).
    apply_ne_slope(tiles, 13, 7)
    cur = tiles.get((13, 7), TileSpec())
    tiles[(13, 7)] = replace(cur, tt=MP_ROAD, m5=0x05, m3=0x05)

    # --- Objetos (y=9), paso 2 en x ---
    put(tiles, 1, 9, TileSpec(tt=MP_HOUSE, m8=0))
    put(tiles, 3, 9, TileSpec(tt=MP_STATION, m5=0x02, m6=2 << 3))  # Truck SE
    put(tiles, 5, 9, TileSpec(tt=MP_STATION, m5=0x00, m6=3 << 3))  # Bus NE
    put(tiles, 7, 9, TileSpec(tt=MP_STATION, m5=0x01, m6=0))  # Rail eje Y
    put(tiles, 9, 9, TileSpec(tt=MP_INDUSTRY, m5=0, m6=0, m1=1))

    # --- Costa (y=11) ---
    put(tiles, 3, 11, TileSpec(tt=MP_WATER, height=1, m5=0x00))
    put(tiles, 5, 11, TileSpec(tt=MP_WATER, height=1, m5=0x10))

    out = (
        Path(__file__).resolve().parents[1]
        / "crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap"
    )
    out.parent.mkdir(parents=True, exist_ok=True)
    data = build_map1(w, h, tiles)
    out.write_bytes(data)
    print(f"Escrito {out} ({len(data)} bytes, {w}×{h})")
    print(
        "Cargar: OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap "
        "cargo run -p openttdrs-client"
    )


if __name__ == "__main__":
    main()
