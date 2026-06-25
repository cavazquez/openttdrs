#!/usr/bin/env python3
"""Mapa grande de QA visual — superset del checklist SP3 en 64×48.

Salida: `crates/openttdrs-core/tests/fixtures/sp3_showcase.ottdmap`

Zonas (origen arriba-izquierda):

| Región        | Aprox.        | Contenido |
|---------------|---------------|-----------|
| Residencial   | x=2–22, y=2–12 | Ciudad con casas variadas + calles |
| Industrial    | x=24–48, y=2–28 | Minas, central, refinería, pozos animados, galería gfx |
| Transporte    | x=2–48, y=14–26 | Autopista, vía doble, estaciones, depósitos |
| Agua / costa  | x=50–63, y=2–28 | Lago + orillas |
| Pendientes    | x=50–63, y=30–45 | Vía/carretera en las 4 diagonales |
| Checklist SP3 | x=2–21, y=31–47 | Mismo layout que `sp3_visual_checklist` (20×17) |

Regenerar: `python3 scripts/gen_sp3_showcase_ottdmap.py`

Cargar:

    OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_showcase.ottdmap \\
      cargo run -p openttdrs-client
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_sp3_visual_checklist_ottdmap import (  # noqa: E402
    DEFAULT_H,
    MP_CLEAR,
    MP_HOUSE,
    MP_INDUSTRY,
    MP_RAILWAY,
    MP_ROAD,
    MP_STATION,
    MP_WATER,
    TileSpec,
    apply_ne_slope,
    apply_nw_slope,
    apply_se_slope,
    apply_sw_slope,
    build_map1,
    bus_stop_tile,
    house_completed,
    house_under_construction,
    industry_tile,
    industry_under_construction,
    put,
    road_depot_tile,
    road_tile,
    truck_stop_tile,
)

OUT = (
    Path(__file__).resolve().parents[1]
    / "crates/openttdrs-core/tests/fixtures/sp3_showcase.ottdmap"
)

W, H = 64, 48


def industry_done(gfx9: int, ind_id: int, m3hi: int = 0) -> TileSpec:
    """Industria terminada con id único en `m1` (bits bajos) para agrupar sim."""
    t = industry_tile(gfx9, ind_id)
    return replace(t, m1=0x80 | (ind_id & 0x7F), m3hi=m3hi & 0xFF)


def blit_layout(
    dst: dict[tuple[int, int], TileSpec],
    ox: int,
    oy: int,
    src: dict[tuple[int, int], TileSpec],
) -> None:
    for (x, y), spec in src.items():
        put(dst, ox + x, oy + y, spec)


def _skip_transport_over(t: TileSpec) -> bool:
    return t.tt in (MP_INDUSTRY, MP_HOUSE, MP_WATER, MP_STATION)


def road_h(dst: dict[tuple[int, int], TileSpec], y: int, x0: int, x1: int) -> None:
    for x in range(x0, x1 + 1):
        cur = dst.get((x, y), TileSpec())
        if _skip_transport_over(cur):
            continue
        bits = cur.m5 & 0xF0
        dst[(x, y)] = replace(cur, tt=MP_ROAD, m5=bits | 0x0A)


def road_v(dst: dict[tuple[int, int], TileSpec], x: int, y0: int, y1: int) -> None:
    for y in range(y0, y1 + 1):
        cur = dst.get((x, y), TileSpec())
        if _skip_transport_over(cur):
            continue
        bits = cur.m5 & 0xF0
        dst[(x, y)] = replace(cur, tt=MP_ROAD, m5=bits | 0x05)


def road_cross(dst: dict[tuple[int, int], TileSpec], x: int, y: int) -> None:
    cur = dst.get((x, y), TileSpec())
    if _skip_transport_over(cur):
        return
    put(dst, x, y, TileSpec(tt=MP_ROAD, m5=0x0F))


def rail_h(dst: dict[tuple[int, int], TileSpec], y: int, x0: int, x1: int) -> None:
    for x in range(x0, x1 + 1):
        cur = dst.get((x, y), TileSpec())
        if _skip_transport_over(cur):
            continue
        dst[(x, y)] = replace(cur, tt=MP_RAILWAY, m5=(cur.m5 & 0xC0) | 0x02)


def place_coal_mine(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    """Layout vanilla mina de carbón (gfx 0–6)."""
    layout = [
        (0, 0, 0),
        (1, 0, 1),
        (0, 1, 2),
        (1, 1, 3),
        (0, 2, 4),
        (1, 2, 5),
        (2, 2, 6),
    ]
    for dx, dy, gfx in layout:
        m3hi = (dx + dy) & 3 if gfx == 1 else 0
        put(dst, ox + dx, oy + dy, industry_done(gfx, ind_id, m3hi))


def place_power_station(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    for dx, gfx in enumerate(range(7, 11)):
        put(dst, ox + dx, oy, industry_done(gfx, ind_id))


def place_sawmill(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    for dx, gfx in enumerate(range(11, 16)):
        put(dst, ox + dx, oy, industry_done(gfx, ind_id))


def place_oil_refinery(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    for dx, gfx in enumerate(range(16, 24)):
        put(dst, ox + dx, oy, industry_done(gfx, ind_id))


def place_factory_block(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    for dx, gfx in enumerate(range(39, 43)):
        put(dst, ox + dx, oy, industry_done(gfx, ind_id))


def place_copper_mine(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int, ind_id: int) -> None:
    put(dst, ox, oy, industry_done(47, ind_id))
    put(dst, ox + 1, oy, industry_done(48, ind_id, m3hi=1))
    put(dst, ox, oy + 1, industry_done(49, ind_id))


def place_oil_wells(dst: dict[tuple[int, int], TileSpec], ox: int, oy: int) -> None:
    """Pozos animados (gfx 30–32) como industrias separadas."""
    for i, gfx in enumerate((29, 30, 31, 32)):
        put(dst, ox + i * 2, oy, industry_done(gfx, 10 + i, m3hi=i & 3))


def fill_water_lake(dst: dict[tuple[int, int], TileSpec], x0: int, y0: int, x1: int, y1: int) -> None:
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(dst, x, y, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0))


def coast_strip(dst: dict[tuple[int, int], TileSpec], x: int, y0: int, y1: int) -> None:
    for y in range(y0, y1 + 1):
        put(dst, x, y, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0x10))


def build_residential(dst: dict[tuple[int, int], TileSpec]) -> None:
    house_ids = (
        0, 16, 20, 24, 32, 39, 44, 70, 78, 82, 66, 109, 107, 128, 0, 16, 20, 24, 32, 39
    )
    idx = 0
    for row in range(2, 13, 2):
        for col in range(2, 23, 2):
            hid = house_ids[idx % len(house_ids)]
            idx += 1
            if (col + row) % 8 == 0:
                put(dst, col, row, house_under_construction((col // 2) % 4, hid))
            else:
                put(dst, col, row, house_completed(hid))
    road_h(dst, 8, 2, 22)
    road_v(dst, 12, 2, 12)
    road_cross(dst, 12, 8)


def build_industrial_park(dst: dict[tuple[int, int], TileSpec]) -> None:
    place_coal_mine(dst, 26, 3, 1)
    place_power_station(dst, 34, 3, 2)
    place_sawmill(dst, 42, 3, 3)
    place_oil_refinery(dst, 26, 11, 4)
    place_oil_wells(dst, 38, 13)
    place_factory_block(dst, 26, 22, 6)
    place_copper_mine(dst, 36, 22, 7)
    # Galería gfx recientes (120–130) + casos límite
    gallery = (0, 42, 116, 119, 120, 125, 130, 256)
    for i, gfx in enumerate(gallery):
        put(dst, 26 + i * 2, 27, industry_done(gfx, 20 + i))
    # Obra industrial (mina gfx0)
    for x, stage in zip((42, 44, 46), (0, 1, 2), strict=True):
        put(dst, x, 27, industry_under_construction(0, stage, 30))


def build_transport(dst: dict[tuple[int, int], TileSpec]) -> None:
    road_h(dst, 15, 2, 48)
    road_v(dst, 25, 2, 28)
    road_cross(dst, 25, 15)
    for x, direction in zip((4, 8, 12, 16, 20), (0, 1, 2, 3, 0), strict=True):
        put(dst, x, 14, bus_stop_tile(direction))
    put(dst, 22, 14, truck_stop_tile(1))
    rail_h(dst, 17, 2, 48)
    rail_h(dst, 19, 2, 48)
    put(dst, 30, 17, TileSpec(tt=MP_RAILWAY, m5=0x07))
    put(dst, 32, 17, TileSpec(tt=MP_RAILWAY, m5=0x03))
    put(
        dst,
        34,
        17,
        TileSpec(tt=MP_RAILWAY, m5=(1 << 6) | 0x02, m3=0xC0, m3hi=0x80),
    )
    put(dst, 36, 17, TileSpec(tt=MP_STATION, m5=0x01, m6=0))
    put(dst, 40, 17, TileSpec(tt=MP_STATION, m5=0x02, m6=0))
    for depot_x, direction, exit_xy, stub in [
        (4, 0, (3, 22), 0x02),
        (10, 1, (10, 23), 0x01),
        (16, 2, (17, 22), 0x08),
        (22, 3, (22, 21), 0x04),
    ]:
        put(dst, depot_x, 22, road_depot_tile(direction))
        put(dst, exit_xy[0], exit_xy[1], road_tile(stub))
    road_h(dst, 23, 2, 48)


def build_water(dst: dict[tuple[int, int], TileSpec]) -> None:
    fill_water_lake(dst, 52, 4, 62, 24)
    coast_strip(dst, 51, 4, 24)
    put(dst, 50, 14, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0x10))


def build_slopes(dst: dict[tuple[int, int], TileSpec]) -> None:
    for tx, ty, slope_fn, m5 in [
        (52, 32, apply_ne_slope, 0x02),
        (56, 32, apply_se_slope, 0x03),
        (60, 32, apply_nw_slope, 0x03),
        (52, 36, apply_sw_slope, 0x07),
        (56, 36, apply_ne_slope, 0x05),
        (60, 36, apply_se_slope, 0x0A),
    ]:
        slope_fn(dst, tx, ty)
        cur = dst.get((tx, ty), TileSpec())
        if m5 in (0x05, 0x0A, 0x0F):
            dst[(tx, ty)] = replace(cur, tt=MP_ROAD, m5=m5)
        else:
            dst[(tx, ty)] = replace(cur, tt=MP_RAILWAY, m5=m5)


def build_checklist_embed(dst: dict[tuple[int, int], TileSpec]) -> None:
    """Incrusta el checklist SP3 completo en la esquina inferior izquierda."""
    checklist: dict[tuple[int, int], TileSpec] = {}
    _build_checklist_tiles(checklist)
    blit_layout(dst, 2, 31, checklist)


def _build_checklist_tiles(tiles: dict[tuple[int, int], TileSpec]) -> None:
    """Copia del layout en `gen_sp3_visual_checklist_ottdmap.main` (sin I/O)."""
    import gen_sp3_visual_checklist_ottdmap as cl

    HOUSE_X = (1, 5, 9, 13, 17)
    for x, hid in zip(HOUSE_X, (0, 44, 88, 107, 128), strict=True):
        put(tiles, x, 0, cl.house_completed(hid))
    for x, stage in zip(HOUSE_X[:4], (0, 1, 2, 3), strict=True):
        put(tiles, x, 1, cl.house_under_construction(stage, house_id=0))
    put(tiles, HOUSE_X[4], 1, cl.house_completed(0))
    for x, hid in zip(HOUSE_X, (16, 20, 24, 32, 39), strict=True):
        put(tiles, x, 2, cl.house_completed(hid))
    for x, hid in zip(HOUSE_X, (70, 78, 82, 66, 109), strict=True):
        put(tiles, x, 6, cl.house_completed(hid))
    for depot_x, direction, exit_xy, road_m5 in [
        (3, 0, (2, 6), 0x02),
        (6, 1, (6, 7), 0x01),
        (10, 2, (11, 6), 0x08),
        (14, 3, (14, 5), 0x04),
    ]:
        put(tiles, depot_x, 6, cl.road_depot_tile(direction))
        put(tiles, exit_xy[0], exit_xy[1], cl.road_tile(road_m5))
    for x, stage in zip(HOUSE_X[:4], (0, 1, 2, 3), strict=True):
        put(tiles, x, 8, cl.house_under_construction(stage, house_id=16))
    put(tiles, HOUSE_X[4], 8, cl.house_completed(16))
    put(tiles, 1, 3, TileSpec(tt=MP_ROAD, m5=0x05))
    put(tiles, 3, 3, TileSpec(tt=MP_ROAD, m5=0x0A))
    put(tiles, 5, 3, TileSpec(tt=MP_ROAD, m5=0x07))
    put(tiles, 7, 3, TileSpec(tt=MP_ROAD, m5=0x0F))
    put(tiles, 9, 3, TileSpec(tt=MP_ROAD, m5=0x40))
    put(tiles, 11, 3, TileSpec(tt=MP_ROAD, m5=0x41))
    put(tiles, 15, 3, TileSpec(tt=MP_ROAD, m5=0x0A, m3=0x0A))
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
    for tx, ty, slope_fn, m5 in [
        (1, 7, cl.apply_ne_slope, 0x05),
        (4, 7, cl.apply_se_slope, 0x0A),
        (7, 7, cl.apply_sw_slope, 0x03),
        (10, 7, cl.apply_nw_slope, 0x0F),
    ]:
        slope_fn(tiles, tx, ty)
        cur = tiles.get((tx, ty), TileSpec())
        tiles[(tx, ty)] = replace(cur, tt=MP_ROAD, m5=m5)
    cl.apply_ne_slope(tiles, 13, 7)
    cur = tiles.get((13, 7), TileSpec())
    tiles[(13, 7)] = replace(cur, tt=MP_ROAD, m5=0x05, m3=0x05)
    cl.apply_ne_slope(tiles, 16, 7)
    cur = tiles.get((16, 7), TileSpec())
    put(tiles, 16, 7, replace(cur, tt=MP_STATION, m5=0x01, m6=0))
    IND_OBRA_X = (1, 5, 9, 13)
    for x, stage in zip(IND_OBRA_X[:3], (0, 1, 2), strict=True):
        put(tiles, x, 4, cl.industry_under_construction(0, stage))
    put(tiles, IND_OBRA_X[3], 4, cl.industry_tile(0))
    put(tiles, 17, 4, cl.house_completed(20))
    IND_X = (1, 3, 5, 7, 9, 11)
    for x, gfx in zip(IND_X, (0, 42, 116, 119, 120, 256), strict=True):
        put(tiles, x, 10, cl.industry_tile(gfx))
    for x, direction in zip((1, 3, 5, 7), (0, 1, 2, 3), strict=True):
        put(tiles, x, 9, cl.bus_stop_tile(direction))
    put(tiles, 9, 9, cl.truck_stop_tile(1))
    put(tiles, 11, 9, TileSpec(tt=MP_STATION, m5=0x01, m6=0))
    put(tiles, 13, 9, cl.house_completed(0))
    cl.apply_ne_slope(tiles, 15, 9)
    cur = tiles.get((15, 9), TileSpec())
    put(tiles, 15, 9, replace(cur, tt=MP_STATION, m5=0, m6=3 << 3, m3=0x08))
    put(tiles, 2, 11, TileSpec(tt=MP_WATER, height=cl.DEFAULT_H, m5=0x00))
    put(tiles, 3, 11, TileSpec(tt=MP_WATER, height=cl.DEFAULT_H, m5=0x00))
    put(tiles, 5, 11, TileSpec(tt=MP_WATER, height=cl.DEFAULT_H, m5=0x10))
    for tx, ty, slope_fn, m5 in [
        (9, 11, cl.apply_ne_slope, 0x02),
        (12, 11, cl.apply_se_slope, 0x03),
        (15, 11, cl.apply_sw_slope, 0x03),
        (18, 11, cl.apply_nw_slope, 0x03),
    ]:
        slope_fn(tiles, tx, ty)
        cur = tiles.get((tx, ty), TileSpec())
        tiles[(tx, ty)] = replace(cur, tt=MP_RAILWAY, m5=m5)
    for tx, ty, slope_fn in [
        (1, 13, cl.apply_ne_slope),
        (4, 13, cl.apply_se_slope),
        (7, 13, cl.apply_sw_slope),
        (10, 13, cl.apply_nw_slope),
    ]:
        slope_fn(tiles, tx, ty)
        cur = tiles.get((tx, ty), TileSpec())
        put(tiles, tx, ty, replace(cur, tt=MP_RAILWAY, m5=0x07))
    for tx, ty, slope_fn in [
        (1, 15, cl.apply_ne_slope),
        (4, 15, cl.apply_se_slope),
        (7, 15, cl.apply_sw_slope),
        (10, 15, cl.apply_nw_slope),
    ]:
        slope_fn(tiles, tx, ty)
        cur = tiles.get((tx, ty), TileSpec())
        put(tiles, tx, ty, replace(cur, tt=MP_RAILWAY, m5=0x03))


def main() -> None:
    tiles: dict[tuple[int, int], TileSpec] = {}
    build_residential(tiles)
    build_industrial_park(tiles)
    build_transport(tiles)
    build_water(tiles)
    build_slopes(tiles)
    build_checklist_embed(tiles)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    data = build_map1(W, H, tiles)
    OUT.write_bytes(data)
    industry_n = sum(1 for t in tiles.values() if t.tt == MP_INDUSTRY)
    station_n = sum(1 for t in tiles.values() if t.tt == MP_STATION)
    print(f"Escrito {OUT} ({len(data)} bytes, {W}×{H})")
    print(f"  teselas industria={industry_n} estación={station_n}")
    print(
        "Cargar: OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_showcase.ottdmap "
        "cargo run -p openttdrs-client"
    )


if __name__ == "__main__":
    main()
