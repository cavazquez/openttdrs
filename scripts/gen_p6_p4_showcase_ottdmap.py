#!/usr/bin/env python3
"""Mapa de QA para paridad P6 (obra) y P4 (agua, paleta, cimientos).

Salida: `crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap`

Layout (36×24, origen arriba-izquierda):

| Zona | Aprox. | Qué ver |
|------|--------|---------|
| P6 estático | y=2, x=1–13 | Misma pieza (gfx 39) en etapas 0, 1, 2 y terminada |
| P6 sim | y=5–7 | Aserradero completo `m1=0` — avanza con la sim (5 Hz) |
| P6 sim | y=8–11 | Fábrica completa `m1=0` — idem |
| P4 agua | x=22–34, y=4–11 | Lago + plataforma petrolera (gfx 24–28) + costa gfx 29 |
| P4 paleta | y=12, x=1–16 | Cuatro gfx 40 terminadas, `m2` distinto → colores distintos |
| P4 pendiente | y=15–17 | Aserradero y fábrica en pendiente NE (cimientos) |
| P6 estático | y=18, x=1–10 | Aserradero gfx 11 en etapas 0–2 y terminada (sin sim) |

Regenerar:

    python3 scripts/gen_p6_p4_showcase_ottdmap.py

Cargar (despausar sim para ver obra P6 en vivo):

    OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap \\
      cargo run -p openttdrs-client
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_sp3_visual_checklist_ottdmap import (  # noqa: E402
    DEFAULT_H,
    MP_INDUSTRY,
    MP_WATER,
    TileSpec,
    apply_ne_slope,
    build_map1,
    industry_tile,
    industry_under_construction,
    put,
)

OUT = (
    Path(__file__).resolve().parents[1]
    / "crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap"
)

W, H = 36, 24


def industry_live_start(gfx9: int, industry_index: int) -> TileSpec:
    """Obra recién colocada (`m1=0`); el tile loop P6 la termina en sim."""
    return industry_under_construction(gfx9, 0, industry_index)


# OpenTTD `_tile_table_sawmill_0` (build_industry.h): 3×3 parcial, no 1×5.
SAWMILL_LAYOUT: list[tuple[int, int, int]] = [
    (1, 0, 14),
    (1, 1, 12),
    (1, 2, 11),
    (2, 0, 14),
    (2, 1, 13),
    (0, 0, 15),
    (0, 1, 15),
    (0, 2, 12),
]

# OpenTTD `_tile_table_factory_0` (build_industry.h): bloque 4×4, no 1×4.
FACTORY_LAYOUT: list[tuple[int, int, int]] = [
    (0, 0, 39),
    (0, 1, 40),
    (1, 0, 41),
    (1, 1, 42),
    (0, 2, 39),
    (0, 3, 40),
    (1, 2, 41),
    (1, 3, 42),
    (2, 1, 39),
    (2, 2, 40),
    (3, 1, 41),
    (3, 2, 42),
]


def place_industry_layout_live(
    dst: dict[tuple[int, int], TileSpec],
    ox: int,
    oy: int,
    ind_id: int,
    layout: list[tuple[int, int, int]],
) -> None:
    for dx, dy, gfx in layout:
        put(dst, ox + dx, oy + dy, industry_live_start(gfx, ind_id))


def place_sawmill_live(
    dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int
) -> None:
    place_industry_layout_live(dst, ox, oy, ind_id, SAWMILL_LAYOUT)


def place_factory_live(
    dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int
) -> None:
    place_industry_layout_live(dst, ox, oy, ind_id, FACTORY_LAYOUT)


# OpenTTD `_tile_table_oil_rig_0` (build_industry.h): 2×3, no 1×5.
OIL_RIG_LAYOUT: list[tuple[int, int, int]] = [
    (0, 0, 24),
    (0, 1, 24),
    (0, 2, 25),
    (1, 0, 26),
    (1, 1, 27),
    (1, 2, 28),
]


def place_oil_rig(
    dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int
) -> None:
    """Plataforma petrolera vanilla (gfx 24–28) sobre agua."""
    for dx, dy, gfx in OIL_RIG_LAYOUT:
        put(dst, ox + dx, oy + dy, industry_tile(gfx, ind_id))


def fill_water(
    dst: dict[tuple[int, int], TileSpec], x0: int, y0: int, x1: int, y1: int
) -> None:
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(dst, x, y, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0))


def build_p6_static_stages(dst: dict[tuple[int, int], TileSpec]) -> None:
    """Una tesela gfx 39 en etapas 0–2 y terminada (referencia instantánea)."""
    gfx = 39
    ind = 1
    for x, stage in [(1, 0), (4, 1), (7, 2), (10, 3)]:
        if stage == 3:
            put(dst, x, 2, industry_tile(gfx, ind))
        else:
            put(dst, x, 2, industry_under_construction(gfx, stage, ind))


def build_p6_sim_zones(dst: dict[tuple[int, int], TileSpec]) -> None:
    place_sawmill_live(dst, 1, 5, 2)
    place_factory_live(dst, 1, 8, 3)


def build_p4_water(dst: dict[tuple[int, int], TileSpec]) -> None:
    fill_water(dst, 24, 4, 34, 10)
    # Costa: gfx 29 con hierba plana (`0xf54`) junto al lago → suelo de agua P4.
    put(dst, 22, 6, industry_tile(29, 4))
    put(dst, 23, 6, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0))
    # Plataforma 2×3 (6 teselas) centrada en el lago.
    place_oil_rig(dst, 27, 5, 6)


def build_p4_palette_row(dst: dict[tuple[int, int], TileSpec]) -> None:
    """Gfx 40 con paleta `random_colour` distinta vía `m2` (1, 2, 3, 4)."""
    for i, ind_id in enumerate((1, 2, 3, 4)):
        put(dst, 1 + i * 4, 12, industry_tile(40, ind_id))


def build_p6_static_sawmill_stages(dst: dict[tuple[int, int], TileSpec]) -> None:
    """Gfx 11 (cobertizo) en etapas 0–2 y terminada — referencia sin sim."""
    gfx = 11
    ind = 10
    for x, stage in [(1, 0), (4, 1), (7, 2), (10, 3)]:
        if stage == 3:
            put(dst, x, 18, industry_tile(gfx, ind))
        else:
            put(dst, x, 18, industry_under_construction(gfx, stage, ind))


def build_p4_slopes(dst: dict[tuple[int, int], TileSpec]) -> None:
    # Aserradero en pendiente NE (cimientos nivelados).
    apply_ne_slope(dst, 2, 15)
    put(dst, 2, 15, industry_tile(11, 7))
    # Fábrica en obra sobre pendiente NE.
    apply_ne_slope(dst, 8, 15)
    put(dst, 8, 15, industry_live_start(39, 8))
    # Terminada en pendiente para comparar.
    apply_ne_slope(dst, 14, 15)
    put(dst, 14, 15, industry_tile(40, 9))


def main() -> None:
    tiles: dict[tuple[int, int], TileSpec] = {}
    build_p6_static_stages(tiles)
    build_p6_sim_zones(tiles)
    build_p4_water(tiles)
    build_p4_palette_row(tiles)
    build_p4_slopes(tiles)
    build_p6_static_sawmill_stages(tiles)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    data = build_map1(W, H, tiles)
    OUT.write_bytes(data)
    industry_n = sum(1 for t in tiles.values() if t.tt == MP_INDUSTRY)
    water_n = sum(1 for t in tiles.values() if t.tt == MP_WATER)
    print(f"Escrito {OUT} ({len(data)} bytes, {W}×{H})")
    print(f"  industria={industry_n} agua={water_n}")
    print(
        "Cargar: OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap "
        "cargo run -p openttdrs-client"
    )
    print("  Despausa la sim (HUD) para ver P6: aserradero y fábrica en y=5–11 avanzan solos.")


if __name__ == "__main__":
    main()
