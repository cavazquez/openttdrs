#!/usr/bin/env python3
"""Genera el fixture MAP1 para capturas manuales SP3.0 (checklist visual denso).

Salida: `crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap`

Layout (12×8, origen arriba-izquierda):

```
y=2  hierba | carretera Y | X | T | cruce | cruce nivel X | cruce nivel Y | tranvía m3
y=3  hierba | vía Y | X | T | cruce vía | …
y=5  hierba | estación | … | industria (gfx 0) | …
y=7  hierba | agua Clear | agua Coast | hierba | …
```

Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from pathlib import Path

# Nibble alto MAPT (OpenTTD TileType)
MP_CLEAR = 0
MP_RAILWAY = 1
MP_ROAD = 2
MP_STATION = 5
MP_WATER = 6
MP_HOUSE = 3
MP_INDUSTRY = 8

FORMAT_VERSION = 1
FLAG_HAS_M2_HI = 1


@dataclass
class TileSpec:
    tt: int = MP_CLEAR
    height: int = 4
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
    n = width * height
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
    w, h = 12, 8
    tiles: dict[tuple[int, int], TileSpec] = {}

    # --- Fila carretera (y=2) ---
    road_y = TileSpec(tt=MP_ROAD, m5=0x05)  # ROAD_Y
    road_x = TileSpec(tt=MP_ROAD, m5=0x0A)  # ROAD_X
    road_t = TileSpec(tt=MP_ROAD, m5=0x07)  # T (NW+NE+SE)
    road_cross = TileSpec(tt=MP_ROAD, m5=0x0F)
    crossing_x = TileSpec(tt=MP_ROAD, m5=0x40)  # Cruce nivel, eje carretera X
    crossing_y = TileSpec(tt=MP_ROAD, m5=0x41)  # Cruce nivel, eje carretera Y
    road_tram = TileSpec(tt=MP_ROAD, m5=0x03, m3=0x0A)  # NW+NE + tranvía ROAD_X

    for x, spec in enumerate(
        [road_y, road_x, road_t, road_cross, crossing_x, crossing_y, road_tram],
        start=1,
    ):
        tiles[(x, 2)] = spec

    # --- Fila vía (y=3) ---
    rail_y = TileSpec(tt=MP_RAILWAY, m5=0x02)  # TRACK_BIT_Y
    rail_x = TileSpec(tt=MP_RAILWAY, m5=0x01)  # TRACK_BIT_X
    rail_t = TileSpec(tt=MP_RAILWAY, m5=0x07)  # X+Y+UPPER (T)
    rail_cross = TileSpec(tt=MP_RAILWAY, m5=0x03)  # X+Y

    for x, spec in enumerate([rail_y, rail_x, rail_t, rail_cross], start=1):
        tiles[(x, 3)] = spec

    # Señales en vía Y
    tiles[(8, 3)] = TileSpec(
        tt=MP_RAILWAY,
        m5=(1 << 6) | 0x02,
        m3=0xC0,
        m3hi=0x80,
    )
    # Vía nieve
    tiles[(9, 3)] = TileSpec(tt=MP_RAILWAY, m5=0x02, m3=0x0C)
    # Estación tren 1×1 junto al cruce de vía (lejos de la costa y=7)
    tiles[(4, 4)] = TileSpec(tt=MP_STATION, m5=0x01, m6=0)  # Rail, eje Y

    # --- Casa + estación camión + industria (y=5) ---
    tiles[(0, 5)] = TileSpec(tt=MP_HOUSE, m8=0)  # Tall Office (HouseID 0, etapa 3 vía hash)
    tiles[(1, 5)] = TileSpec(tt=MP_STATION, m5=0x02, m6=2 << 3)  # Truck
    tiles[(4, 5)] = TileSpec(tt=MP_INDUSTRY, m5=0, m6=0, m1=1)  # gfx 0 = coal mine

    # --- Costa (y=7): hierba | mar Clear | costa | hierba ---
    tiles[(2, 7)] = TileSpec(tt=MP_WATER, height=1, m5=0x00)  # Clear
    tiles[(3, 7)] = TileSpec(tt=MP_WATER, height=1, m5=0x10)  # Coast

    out = (
        Path(__file__).resolve().parents[1]
        / "crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap"
    )
    out.parent.mkdir(parents=True, exist_ok=True)
    data = build_map1(w, h, tiles)
    out.write_bytes(data)
    print(f"Escrito {out} ({len(data)} bytes, {w}×{h})")
    print("Cargar: OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client")


if __name__ == "__main__":
    main()
