#!/usr/bin/env python3
"""Genera el dibujo vanilla completo de las teselas de aeropuerto.

OpenTTD guarda en ``m5`` el ``StationGfx`` de aeropuerto (0..=73), no una
categoría abstracta como "terminal" o "torre". Cada valor referencia una
entrada de ``_station_display_datas_airport`` y, en muchos casos, una
secuencia ``TILE_SEQ_LINE`` o ``TILE_SEQ_GROUND`` adicional.

Este generador copia esa tabla de ``station_land.h`` en una representación
Rust con las dimensiones/offsets del NFO OpenGFX activo. También resuelve los
sprites Action5 necesarios para helipads y mitades de apron. Así 8bpp y 32bpp
usan siempre rects del mismo perfil gráfico, nunca una hoja del perfil opuesto.

Salidas:

* ``assets/opengfx/tiles/airport_*.png``;
* ``crates/openttdrs-client/src/sprites/airport_station_draw_data_generated.rs``.

Uso:
  python3 scripts/gen_airport_station_draw_data.py
  python3 scripts/gen_airport_station_draw_data.py --check
"""
from __future__ import annotations

import argparse
import re
import sys
from collections import defaultdict
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
    NFO_SPRITE_ROW_RE,
    SpriteRect,
    active_global_sprite_nfo,
    detect_graphics_mode,
    parse_global_sprite_rects,
)
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = (
    REPO
    / "crates"
    / "openttdrs-client"
    / "src"
    / "sprites"
    / "airport_station_draw_data_generated.rs"
)

# OpenTTD 15.3, src/table/sprites.h. Los Action5 no viven en el NFO global:
# se resuelven desde el bloque que los instala en el GRF extra.
SPR_OPENTTD_BASE = 4896
SPR_AIRPORTX_BASE = 5954


def line(
    gfx: int,
    label: str,
    sprite_id: int,
    dx: int,
    dy: int,
    dz: int,
    sx: int,
    sy: int,
    sz: int,
    company_coloured: bool = True,
) -> tuple[int, str, int, int, int, int, int, int, int, bool]:
    return (gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, company_coloured)


def ground(
    gfx: int,
    label: str,
    sprite_id: int,
    dx: int,
    dy: int,
    dz: int = 0,
) -> tuple[int, str, int, int, int, int]:
    # Todas las TILE_SEQ_GROUND airport vanilla usan PALETTE_MODIFIER_COLOUR.
    return (gfx, label, sprite_id, dx, dy, dz)


# (StationGfx, etiqueta upstream, SpriteID de suelo, PALETTE_MODIFIER_COLOUR).
# Fuente: `_station_display_datas_airport` de station_land.h. Los cinco
# `TILE_SPRITE_NULL` se sustituyen dinámicamente por sus tablas de animación,
# pero conservan aquí su base de suelo exacta.
AIRPORT_STATION_BASES = (
    (0, "APT_APRON", 2634, False),
    (1, "APT_APRON_FENCE_NW", 2634, False),
    (2, "APT_APRON_FENCE_SW", 2634, False),
    (3, "APT_STAND", 2635, False),
    (4, "APT_APRON_W", 2636, False),
    (5, "APT_APRON_S", 2637, False),
    (6, "APT_APRON_VER_CROSSING_S", 2638, False),
    (7, "APT_APRON_HOR_CROSSING_W", 2639, False),
    (8, "APT_APRON_VER_CROSSING_N", 2640, False),
    (9, "APT_APRON_HOR_CROSSING_E", 2641, False),
    (10, "APT_APRON_E", 2642, False),
    (11, "APT_ARPON_N", 2643, False),
    (12, "APT_APRON_HOR", 2644, False),
    (13, "APT_APRON_N_FENCE_SW", 2643, False),
    (14, "APT_RUNWAY_1", 2645, False),
    (15, "APT_RUNWAY_2", 2646, False),
    (16, "APT_RUNWAY_3", 2647, False),
    (17, "APT_RUNWAY_4", 2648, False),
    (18, "APT_RUNWAY_END_FENCE_SE", 2649, False),
    (19, "APT_BUILDING_2", 2634, False),
    (20, "APT_TOWER_FENCE_SW", 3981, False),
    (21, "APT_ROUND_TERMINAL", 2634, False),
    (22, "APT_BUILDING_3", 2634, False),
    (23, "APT_BUILDING_1", 2634, False),
    (24, "APT_DEPOT_SE", 2634, False),
    (25, "APT_STAND_1", 2635, False),
    (26, "APT_STAND_PIER_NE", 2635, False),
    (27, "APT_PIER_NW_NE", 2634, False),
    (28, "APT_PIER", 2634, False),
    (29, "APT_EMPTY", 3981, False),
    (30, "APT_EMPTY_FENCE_NE", 3981, False),
    (31, "APT_RADAR_GRASS_FENCE_SW", 3981, False),
    (32, "APT_RADIO_TOWER_FENCE_NE", 3981, False),
    (33, "APT_SMALL_BUILDING_3", 2665, False),
    (34, "APT_SMALL_BUILDING_2", 2666, False),
    (35, "APT_SMALL_BUILDING_1", 2667, True),
    (36, "APT_GRASS_FENCE_SW", 2669, False),
    (37, "APT_GRASS_2", 2670, False),
    (38, "APT_GRASS_1", 2671, False),
    (39, "APT_GRASS_FENCE_NE_FLAG", 2672, False),
    (40, "APT_RUNWAY_SMALL_NEAR_END", 2673, False),
    (41, "APT_RUNWAY_SMALL_MIDDLE", 2674, False),
    (42, "APT_RUNWAY_SMALL_FAR_END", 2675, False),
    (43, "APT_SMALL_DEPOT_SE", 2634, False),
    (44, "APT_HELIPORT", 3981, False),
    (45, "APT_RUNWAY_END", 2649, False),
    (46, "APT_RUNWAY_5", 2646, False),
    (47, "APT_TOWER", 2634, False),
    (48, "APT_APRON_FENCE_NE", 2634, False),
    (49, "APT_RUNWAY_END_FENCE_NW", 2649, False),
    (50, "APT_RUNWAY_FENCE_NW", 2646, False),
    (51, "APT_RADAR_FENCE_SW", 2634, False),
    (52, "APT_RADAR_FENCE_NE", 2634, False),
    (53, "APT_HELIPAD_1", 2634, False),
    (54, "APT_HELIPAD_2_FENCE_NW", 2634, False),
    (55, "APT_HELIPAD_2", 2634, False),
    (56, "APT_APRON_FENCE_NE_SW", 2634, False),
    (57, "APT_RUNWAY_END_FENCE_NW_SW", 2649, False),
    (58, "APT_RUNWAY_END_FENCE_SE_SW", 2649, False),
    (59, "APT_RUNWAY_END_FENCE_NE_NW", 2649, False),
    (60, "APT_RUNWAY_END_FENCE_NE_SE", 2649, False),
    (61, "APT_HELIPAD_2_FENCE_NE_SE", 2634, False),
    (62, "APT_APRON_FENCE_SE_SW", 2634, False),
    (63, "APT_LOW_BUILDING_FENCE_N", 2634, False),
    (64, "APT_LOW_BUILDING_FENCE_NW", 2634, False),
    (65, "APT_APRON_FENCE_SE", 2634, False),
    (66, "APT_HELIPAD_3_FENCE_SE_SW", 2634, False),
    (67, "APT_HELIPAD_3_FENCE_NW_SW", 2634, False),
    (68, "APT_HELIPAD_3_FENCE_NW", 2634, False),
    (69, "APT_LOW_BUILDING", 2634, False),
    (70, "APT_APRON_FENCE_NE_SE", 2634, False),
    (71, "APT_APRON_HALF_EAST", 2634, False),
    (72, "APT_APRON_HALF_WEST", 2634, False),
    (73, "APT_GRASS_FENCE_NE_FLAG_2", 3981, False),
)

# Todas las `TILE_SEQ_LINE` de `_station_display_datas_airport`. 31, 51 y
# 52 almacenan el frame 0 del radar: el renderer sustituye ese sprite por el
# frame actual de m7 conservando el mismo ancla/bounding box.
AIRPORT_STATION_OVERLAYS = (
    line(19, "APT_BUILDING_2", 2650, 2, 0, 0, 11, 16, 40),
    line(20, "APT_TOWER_FENCE_SW", 2651, 3, 3, 0, 10, 10, 60),
    line(20, "APT_TOWER_FENCE_SW", 2663, 15, 0, 0, 1, 16, 6),
    line(21, "APT_ROUND_TERMINAL", 2652, 0, 1, 0, 14, 14, 30),
    line(22, "APT_BUILDING_3", 2653, 3, 3, 0, 10, 11, 35),
    line(23, "APT_BUILDING_1", 2654, 0, 3, 0, 16, 11, 40),
    line(24, "APT_DEPOT_SE", 2655, 14, 0, 0, 2, 17, 28),
    line(24, "APT_DEPOT_SE", 2656, 0, 0, 0, 2, 17, 28),
    line(25, "APT_STAND_1", 2659, 7, 11, 0, 3, 3, 14),
    line(25, "APT_STAND_1", 2664, 0, 0, 0, 16, 1, 6),
    line(26, "APT_STAND_PIER_NE", 2660, 2, 7, 0, 3, 3, 14),
    line(27, "APT_PIER_NW_NE", 2661, 3, 2, 0, 3, 3, 14),
    line(28, "APT_PIER", 2662, 0, 8, 0, 14, 3, 14),
    line(31, "APT_RADAR_GRASS_FENCE_SW", 2680, 7, 7, 0, 2, 2, 8, False),
    line(31, "APT_RADAR_GRASS_FENCE_SW", 2663, 15, 0, 0, 1, 16, 6),
    line(32, "APT_RADIO_TOWER_FENCE_NE", 2601, 7, 7, 0, 2, 2, 70, False),
    line(32, "APT_RADIO_TOWER_FENCE_NE", 2663, 0, 0, 0, 1, 16, 6),
    line(35, "APT_SMALL_BUILDING_1", 2668, 0, 0, 0, 15, 15, 30),
    line(39, "APT_GRASS_FENCE_NE_FLAG", 2663, 0, 0, 0, 1, 16, 6),
    line(39, "APT_GRASS_FENCE_NE_FLAG", 2676, 4, 11, 0, 1, 1, 20),
    line(43, "APT_SMALL_DEPOT_SE", 2657, 14, 0, 0, 2, 17, 28),
    line(43, "APT_SMALL_DEPOT_SE", 2658, 0, 0, 0, 2, 17, 28),
    line(44, "APT_HELIPORT", 2633, 0, 0, 0, 16, 16, 60),
    line(47, "APT_TOWER", 2651, 3, 3, 0, 10, 10, 60),
    line(51, "APT_RADAR_FENCE_SW", 2680, 7, 7, 0, 2, 2, 8, False),
    line(51, "APT_RADAR_FENCE_SW", 2663, 15, 0, 0, 1, 16, 6),
    line(52, "APT_RADAR_FENCE_NE", 2680, 7, 7, 0, 2, 2, 8, False),
    line(52, "APT_RADAR_FENCE_NE", 2663, 0, 0, 0, 1, 16, 6),
    line(53, "APT_HELIPAD_1", 4982, 10, 6, 0, 0, 0, 0, False),
    line(53, "APT_HELIPAD_1", 2663, 15, 0, 0, 1, 16, 6),
    line(54, "APT_HELIPAD_2_FENCE_NW", 4982, 10, 6, 0, 0, 0, 0, False),
    line(54, "APT_HELIPAD_2_FENCE_NW", 2664, 0, 0, 0, 16, 1, 6),
    line(55, "APT_HELIPAD_2", 4982, 10, 6, 0, 0, 0, 0, False),
    line(61, "APT_HELIPAD_2_FENCE_NE_SE", 4982, 10, 6, 0, 0, 0, 0, False),
    line(61, "APT_HELIPAD_2_FENCE_NE_SE", 2663, 0, 0, 0, 1, 16, 6),
    line(61, "APT_HELIPAD_2_FENCE_NE_SE", 2664, 0, 15, 0, 16, 1, 6),
    line(63, "APT_LOW_BUILDING_FENCE_N", 2664, 0, 0, 0, 16, 1, 6),
    line(63, "APT_LOW_BUILDING_FENCE_N", 2663, 0, 0, 0, 1, 16, 6),
    line(63, "APT_LOW_BUILDING_FENCE_N", 2095, 3, 3, 0, 10, 10, 60),
    line(64, "APT_LOW_BUILDING_FENCE_NW", 2664, 0, 0, 0, 16, 1, 6),
    line(64, "APT_LOW_BUILDING_FENCE_NW", 2095, 3, 3, 0, 10, 10, 60),
    line(66, "APT_HELIPAD_3_FENCE_SE_SW", 5966, 0, 1, 2, 0, 0, 0, False),
    line(66, "APT_HELIPAD_3_FENCE_SE_SW", 2663, 15, 0, 0, 1, 16, 6),
    line(66, "APT_HELIPAD_3_FENCE_SE_SW", 2664, 0, 15, 0, 16, 1, 6),
    line(67, "APT_HELIPAD_3_FENCE_NW_SW", 5966, 0, 1, 2, 0, 0, 0, False),
    line(67, "APT_HELIPAD_3_FENCE_NW_SW", 2663, 15, 0, 0, 1, 16, 6),
    line(67, "APT_HELIPAD_3_FENCE_NW_SW", 2664, 0, 0, 0, 16, 1, 6),
    line(68, "APT_HELIPAD_3_FENCE_NW", 5966, 0, 1, 2, 0, 0, 0, False),
    line(68, "APT_HELIPAD_3_FENCE_NW", 2664, 0, 0, 0, 16, 1, 6),
    line(69, "APT_LOW_BUILDING", 2095, 3, 3, 0, 10, 10, 60),
    line(71, "APT_APRON_HALF_EAST", 5968, 0, 0, 0, 0, 0, 0, False),
    line(72, "APT_APRON_HALF_WEST", 5967, 0, 0, 0, 0, 0, 0, False),
    line(73, "APT_GRASS_FENCE_NE_FLAG_2", 2663, 0, 0, 0, 1, 16, 6),
    line(73, "APT_GRASS_FENCE_NE_FLAG_2", 2676, 4, 11, 0, 1, 1, 20),
)

# Todas las TILE_SEQ_GROUND, que OpenTTD pinta en el pase de suelo y no como
# sprites sortable. El orden dentro de cada entrada coincide con station_land.h.
AIRPORT_STATION_GROUND_LAYERS = (
    ground(1, "APT_APRON_FENCE_NW", 2664, 0, 0),
    ground(2, "APT_APRON_FENCE_SW", 2663, 15, 0),
    ground(13, "APT_APRON_N_FENCE_SW", 2663, 15, 0),
    ground(14, "APT_RUNWAY_1", 2664, 0, 15),
    ground(15, "APT_RUNWAY_2", 2664, 0, 15),
    ground(16, "APT_RUNWAY_3", 2664, 0, 15),
    ground(17, "APT_RUNWAY_4", 2664, 0, 15),
    ground(18, "APT_RUNWAY_END_FENCE_SE", 2664, 0, 15),
    ground(30, "APT_EMPTY_FENCE_NE", 2663, 0, 0),
    ground(36, "APT_GRASS_FENCE_SW", 2663, 15, 0),
    ground(40, "APT_RUNWAY_SMALL_NEAR_END", 2664, 0, 15),
    ground(41, "APT_RUNWAY_SMALL_MIDDLE", 2664, 0, 15),
    ground(42, "APT_RUNWAY_SMALL_FAR_END", 2664, 0, 15),
    ground(45, "APT_RUNWAY_END", 2663, 0, 0),
    ground(48, "APT_APRON_FENCE_NE", 2663, 0, 0),
    ground(49, "APT_RUNWAY_END_FENCE_NW", 2664, 0, 0),
    ground(50, "APT_RUNWAY_FENCE_NW", 2664, 0, 0),
    ground(56, "APT_APRON_FENCE_NE_SW", 2663, 0, 0),
    ground(56, "APT_APRON_FENCE_NE_SW", 2663, 15, 0),
    ground(57, "APT_RUNWAY_END_FENCE_NW_SW", 2664, 0, 0),
    ground(57, "APT_RUNWAY_END_FENCE_NW_SW", 2663, 15, 0),
    ground(58, "APT_RUNWAY_END_FENCE_SE_SW", 2663, 15, 0),
    ground(58, "APT_RUNWAY_END_FENCE_SE_SW", 2664, 0, 15),
    ground(59, "APT_RUNWAY_END_FENCE_NE_NW", 2664, 0, 0),
    ground(59, "APT_RUNWAY_END_FENCE_NE_NW", 2663, 0, 0),
    ground(60, "APT_RUNWAY_END_FENCE_NE_SE", 2663, 0, 0),
    ground(60, "APT_RUNWAY_END_FENCE_NE_SE", 2664, 0, 15),
    ground(62, "APT_APRON_FENCE_SE_SW", 2663, 15, 0),
    ground(62, "APT_APRON_FENCE_SE_SW", 2664, 0, 15),
    ground(65, "APT_APRON_FENCE_SE", 2664, 0, 15),
    ground(70, "APT_APRON_FENCE_NE_SE", 2663, 0, 0),
    ground(70, "APT_APRON_FENCE_NE_SE", 2664, 0, 15),
)

# SpriteID global -> nombre de tile. Mantener los nombres preexistentes evita
# que una regeneración invalide rutas ya usadas por la UI o mapas demo.
GLOBAL_SPRITES = (
    (2095, "airport_helidepot_office.png"),
    (2601, "airport_transmitter.png"),
    (2633, "airport_heliport.png"),
    (2634, "airport_apron.png"),
    (2635, "airport_stand.png"),
    *tuple((2636 + i, f"airport_taxiway_{i}.png") for i in range(9)),
    *tuple((2645 + i, f"airport_runway_{i}.png") for i in range(5)),
    (2650, "airport_terminal_a.png"),
    (2651, "airport_tower.png"),
    (2652, "airport_concourse.png"),
    (2653, "airport_terminal_b.png"),
    (2654, "airport_terminal_c.png"),
    (2655, "airport_hangar_front.png"),
    (2656, "airport_hangar_rear.png"),
    (2657, "airport_airfield_hangar_front.png"),
    (2658, "airport_airfield_hangar_rear.png"),
    (2659, "airport_jetway_1.png"),
    (2660, "airport_jetway_2.png"),
    (2661, "airport_jetway_3.png"),
    (2662, "airport_passenger_tunnel.png"),
    (2663, "airport_fence_y.png"),
    (2664, "airport_fence_x.png"),
    (2665, "airport_airfield_terminal_a.png"),
    (2666, "airport_airfield_terminal_b.png"),
    (2667, "airport_airfield_terminal_c_ground.png"),
    (2668, "airport_airfield_terminal_c_build.png"),
    (2669, "airport_airfield_apron_a.png"),
    (2670, "airport_airfield_apron_b.png"),
    (2671, "airport_airfield_apron_c.png"),
    (2672, "airport_airfield_apron_d.png"),
    (2673, "airport_airfield_runway_near.png"),
    (2674, "airport_airfield_runway_middle.png"),
    (2675, "airport_airfield_runway_far.png"),
    *tuple((2676 + i, f"airport_wind_{i}.png") for i in range(4)),
    *tuple((2680 + i, f"airport_radar_{i:02}.png") for i in range(12)),
    (3981, "grass.png"),
)

# Action5 0x95 (GUI) y 0x10 (airportx). Los ids son virtuales runtime y no
# los IDs físicos consecutivos que escribe grfcodec en el NFO extra.
ACTION5_SPRITES = (
    (4982, "airport_helipad.png"),
    (5966, "airport_new_helipad.png"),
    (5967, "airport_grass_right.png"),
    (5968, "airport_grass_left.png"),
)

# Secuencias que no aparecen por completo en la tabla estática: `m7` elige
# cualquier frame durante una partida cargada. Mantenerlas declaradas aparte
# hace que la regresión sintética pueda reducir el fixture sin perder el
# contrato de producción.
DYNAMIC_SPRITE_IDS = (*range(2676, 2680), *range(2680, 2692))

ACTION5_RE = re.compile(
    r"^\s*\d+\s+\*\s+\d+\s+05\s+(?P<kind>[0-9A-F]{2})\s+FF\s+"
    r"(?P<count_lo>[0-9A-F]{2})\s+(?P<count_hi>[0-9A-F]{2})"
    r"(?:\s+FF\s+(?P<offset_lo>[0-9A-F]{2})\s+(?P<offset_hi>[0-9A-F]{2}))?",
    re.IGNORECASE,
)


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    """Abre una hoja NFO con la transparencia correcta del perfil activo."""

    image = Image.open(png_path)
    if mode == "32bpp":
        if image.mode == "P":
            palette = image.getpalette()
            transparent_rgb = tuple(palette[:3]) if palette else None
            rgba = image.convert("RGBA")
            if transparent_rgb is not None:
                rgba.putdata(
                    [
                        (0, 0, 0, 0) if pixel[:3] == transparent_rgb else pixel
                        for pixel in rgba.getdata()
                    ]
                )
            return rgba
        return image.convert("RGBA")
    if image.mode == "P":
        return indexed_dos_to_rgba(image)
    return dematte_legacy_colorkey(image)


def active_extra_sprite_nfo(mode: str) -> Path | None:
    root = REPO / "assets" / "opengfx"
    if mode == "32bpp":
        candidates = sorted(root.glob("opengfx2-*/sprites/ogfx2e_extra_32ez.nfo"), reverse=True)
    else:
        candidates = sorted(root.glob("opengfx-*/sprites/ogfxe_extra.nfo"), reverse=True)
    return next((path for path in candidates if path.is_file()), None)


def selected_extra_rects(nfo_path: Path, mode: str) -> dict[int, SpriteRect]:
    """Rect de cada ID físico del NFO extra en la variante activa normal."""

    wanted_bpp = "32bpp" if mode == "32bpp" else "8bpp"
    rects: dict[int, SpriteRect] = {}
    current_id: int | None = None
    for raw in nfo_path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = NFO_SPRITE_ROW_RE.match(raw)
        if match is None:
            continue
        if match["sprite_id"] is not None:
            current_id = int(match["sprite_id"])
        if current_id is None or match["zoom"] != "normal" or match["bpp"] != wanted_bpp:
            continue
        rects.setdefault(
            current_id,
            SpriteRect(
                int(match["x"]),
                int(match["y"]),
                int(match["w"]),
                int(match["h"]),
                int(match["xrel"]),
                int(match["yrel"]),
                Path(match["sheet"]).name,
            ),
        )
    return rects


def action5_rects(nfo_path: Path, mode: str) -> dict[int, SpriteRect]:
    """Resuelve Action5 0x10/0x95 a SpriteID runtime de OpenTTD.

    Una cabecera Action5 enumera ``count`` sprites físicos inmediatamente
    posteriores. El NFO clásico reinicia su numeración por GRF, de modo que el
    contrato fiable es la cabecera y no el número físico.
    """

    lines = nfo_path.read_text(encoding="utf-8", errors="replace").splitlines()
    physical_rects = selected_extra_rects(nfo_path, mode)
    resolved: dict[int, SpriteRect] = {}
    for index, raw in enumerate(lines):
        action = ACTION5_RE.match(raw)
        if action is None:
            continue
        kind = int(action["kind"], 16)
        if kind == 0x10:
            runtime_base = SPR_AIRPORTX_BASE
        elif kind == 0x95:
            runtime_base = SPR_OPENTTD_BASE
        else:
            continue
        count = int(action["count_lo"], 16) | (int(action["count_hi"], 16) << 8)
        offset = 0
        if action["offset_lo"] is not None:
            offset = int(action["offset_lo"], 16) | (int(action["offset_hi"], 16) << 8)

        physical_ids: list[int] = []
        for following in lines[index + 1 :]:
            next_action = ACTION5_RE.match(following)
            if next_action is not None:
                break
            row = NFO_SPRITE_ROW_RE.match(following)
            if row is None or row["sprite_id"] is None:
                continue
            physical_ids.append(int(row["sprite_id"]))
            if len(physical_ids) == count:
                break
        if len(physical_ids) != count:
            continue
        for slot, physical_id in enumerate(physical_ids):
            rect = physical_rects.get(physical_id)
            if rect is not None:
                resolved[runtime_base + offset + slot] = rect
    return resolved


class AirportStationSpriteCropper:
    """Recorta IDs globales y Action5 del perfil OpenGFX activo."""

    def __init__(self, mode: str) -> None:
        global_nfo = active_global_sprite_nfo(REPO, mode)
        if global_nfo is None:
            raise FileNotFoundError(
                "No hay NFO OpenGFX base activo; ejecutá scripts/descargar_graficos.sh"
            )
        self.mode = mode
        self.global_dir = global_nfo.parent
        self.global_rects = parse_global_sprite_rects(global_nfo, mode)
        self.extra_dir: Path | None = None
        self.extra_rects: dict[int, SpriteRect] = {}
        extra_nfo = active_extra_sprite_nfo(mode)
        if extra_nfo is not None:
            self.extra_dir = extra_nfo.parent
            self.extra_rects = action5_rects(extra_nfo, mode)
        self.sheets: dict[Path, Image.Image] = {}

    def source(self, sprite_id: int) -> tuple[Path, SpriteRect]:
        if sprite_id in self.extra_rects and self.extra_dir is not None:
            return self.extra_dir, self.extra_rects[sprite_id]
        try:
            return self.global_dir, self.global_rects[sprite_id]
        except KeyError as error:
            hint = ""
            if sprite_id in dict(ACTION5_SPRITES):
                hint = "; falta el Action5 del GRF extra para el perfil activo"
            raise RuntimeError(
                f"sprite de aeropuerto {sprite_id} no está disponible{hint}"
            ) from error

    def rect(self, sprite_id: int) -> SpriteRect:
        return self.source(sprite_id)[1]

    def crop(self, sprite_id: int, output_name: str) -> SpriteRect:
        source_dir, rect = self.source(sprite_id)
        source = source_dir / rect.sheet
        if not source.is_file():
            source = source.with_suffix(".pcx")
        if not source.is_file():
            raise FileNotFoundError(f"no existe hoja OpenGFX {source}")
        if source not in self.sheets:
            self.sheets[source] = load_sheet(source, self.mode)
        TILES_DIR.mkdir(parents=True, exist_ok=True)
        self.sheets[source].crop((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h)).save(
            TILES_DIR / output_name
        )
        return rect


def required_sprite_ids() -> tuple[int, ...]:
    ids = {sprite_id for _gfx, _label, sprite_id, _cc in AIRPORT_STATION_BASES}
    ids.update(sprite_id for _gfx, _label, sprite_id, *_rest in AIRPORT_STATION_OVERLAYS)
    ids.update(sprite_id for _gfx, _label, sprite_id, *_rest in AIRPORT_STATION_GROUND_LAYERS)
    # Las tablas dinámicas usan cuatro banderas y doce frames de radar,
    # aunque las entradas estáticas sólo contienen el primer frame de cada
    # secuencia. El renderer escoge el frame vivo desde m7.
    ids.update(DYNAMIC_SPRITE_IDS)
    return tuple(sorted(ids))


def sprite_names() -> dict[int, str]:
    names = dict(GLOBAL_SPRITES)
    names.update(ACTION5_SPRITES)
    unknown = sorted(set(required_sprite_ids()) - set(names))
    if unknown:
        raise RuntimeError(f"faltan nombres de tile para sprites airport {unknown}")
    return names


def airport_station_sprites(mode: str, *, write_tiles: bool) -> dict[int, tuple[str, SpriteRect]]:
    cropper = AirportStationSpriteCropper(mode)
    names = sprite_names()
    out: dict[int, tuple[str, SpriteRect]] = {}
    for sprite_id in required_sprite_ids():
        name = names[sprite_id]
        rect = cropper.crop(sprite_id, name) if write_tiles else cropper.rect(sprite_id)
        out[sprite_id] = (name, rect)
    return out


def airport_station_layers(mode: str, *, write_tiles: bool) -> list[tuple]:
    """Compatibilidad: capas lineales enriquecidas con rect NFO."""

    sprites = airport_station_sprites(mode, write_tiles=write_tiles)
    out = []
    for gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, cc in AIRPORT_STATION_OVERLAYS:
        name, rect = sprites[sprite_id]
        out.append((gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, cc, name, rect))
    return out


def airport_station_ground_layers(mode: str, *, write_tiles: bool) -> list[tuple]:
    sprites = airport_station_sprites(mode, write_tiles=write_tiles)
    out = []
    for gfx, label, sprite_id, dx, dy, dz in AIRPORT_STATION_GROUND_LAYERS:
        name, rect = sprites[sprite_id]
        out.append((gfx, label, sprite_id, dx, dy, dz, name, rect))
    return out


def airport_station_data(mode: str, *, write_tiles: bool) -> tuple[dict[int, tuple[str, SpriteRect]], list[tuple], list[tuple]]:
    sprites = airport_station_sprites(mode, write_tiles=write_tiles)
    layers = []
    for gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, cc in AIRPORT_STATION_OVERLAYS:
        name, rect = sprites[sprite_id]
        layers.append((gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, cc, name, rect))
    ground_layers = []
    for gfx, label, sprite_id, dx, dy, dz in AIRPORT_STATION_GROUND_LAYERS:
        name, rect = sprites[sprite_id]
        ground_layers.append((gfx, label, sprite_id, dx, dy, dz, name, rect))
    return sprites, layers, ground_layers


def f(value: int | float) -> str:
    return f"{float(value):.1f}"


def render_output(
    sprites: dict[int, tuple[str, SpriteRect]],
    layers: list[tuple],
    ground_layers: list[tuple],
    mode: str,
) -> str:
    """Renderiza la tabla Rust sin depender de PNGs ya recortados."""

    layers_by_gfx: dict[int, list[tuple]] = defaultdict(list)
    for layer in layers:
        layers_by_gfx[layer[0]].append(layer)
    ground_by_gfx: dict[int, list[tuple]] = defaultdict(list)
    for layer in ground_layers:
        ground_by_gfx[layer[0]].append(layer)

    lines = [
        "// Generado por scripts/gen_airport_station_draw_data.py — NO EDITAR A MANO.",
        "// Fuente: OpenTTD table/station_land.h (los 74 StationGfx vanilla) + NFO OpenGFX.",
        "// Los IDs Action5 se resuelven desde el GRF extra del mismo perfil gráfico.",
        f"// Modo gráfico detectado: {mode}.",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq)]",
        "pub struct AirportStationSprite {",
        "    pub sprite_id: u32,",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub path: &'static str,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct AirportStationBase {",
        "    pub sprite_id: u32,",
        "    pub company_coloured: bool,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq)]",
        "pub struct AirportStationLayer {",
        "    pub sprite_id: u32,",
        "    pub dx: f32,",
        "    pub dy: f32,",
        "    pub dz: f32,",
        "    pub sx: i32,",
        "    pub sy: i32,",
        "    pub sz: i32,",
        "    pub z: f32,",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub company_coloured: bool,",
        "    pub path: &'static str,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq)]",
        "pub struct AirportStationGroundLayer {",
        "    pub sprite_id: u32,",
        "    pub dx: f32,",
        "    pub dy: f32,",
        "    pub dz: f32,",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub company_coloured: bool,",
        "    pub path: &'static str,",
        "}",
        "",
        f"pub static AIRPORT_STATION_SPRITES: [AirportStationSprite; {len(sprites)}] = [",
    ]
    for sprite_id, (name, rect) in sorted(sprites.items()):
        lines.append(
            "    AirportStationSprite { "
            f"sprite_id: {sprite_id}, w: {f(rect.w)}, h: {f(rect.h)}, "
            f"x_offs: {f(rect.xrel)}, y_offs: {f(rect.yrel)}, "
            f'path: "assets/opengfx/tiles/{name}" }},'
        )
    lines.extend(["];", ""])

    for gfx, label, sprite_id, company_coloured in AIRPORT_STATION_BASES:
        lines.append(
            f"/// {label} (`StationGfx {gfx}`), base `DrawGroundSprite` de OpenTTD."
        )
        lines.append(
            f"pub const AIRPORT_GFX_{gfx}_BASE: AirportStationBase = "
            f"AirportStationBase {{ sprite_id: {sprite_id}, company_coloured: {'true' if company_coloured else 'false'} }};"
        )
    lines.append("")

    for gfx in sorted(layers_by_gfx):
        grouped = layers_by_gfx[gfx]
        lines.extend(
            [
                f"/// Capas `TILE_SEQ_LINE` de StationGfx {gfx}, en orden upstream.",
                f"pub static AIRPORT_GFX_{gfx}_LAYERS: [AirportStationLayer; {len(grouped)}] = [",
            ]
        )
        for (
            _gfx,
            _label,
            sprite_id,
            dx,
            dy,
            dz,
            sx,
            sy,
            sz,
            company_coloured,
            name,
            rect,
        ) in grouped:
            lines.append(
                "    AirportStationLayer { "
                f"sprite_id: {sprite_id}, dx: {f(dx)}, dy: {f(dy)}, dz: {f(dz)}, "
                f"sx: {sx}, sy: {sy}, sz: {sz}, z: 0.050, "
                f"w: {f(rect.w)}, h: {f(rect.h)}, x_offs: {f(rect.xrel)}, y_offs: {f(rect.yrel)}, "
                f"company_coloured: {'true' if company_coloured else 'false'}, "
                f'path: "assets/opengfx/tiles/{name}" }},'
            )
        lines.extend(["];", ""])

    for gfx in sorted(ground_by_gfx):
        grouped = ground_by_gfx[gfx]
        lines.extend(
            [
                f"/// Capas `TILE_SEQ_GROUND` de StationGfx {gfx}, en orden upstream.",
                f"pub static AIRPORT_GFX_{gfx}_GROUND_LAYERS: [AirportStationGroundLayer; {len(grouped)}] = [",
            ]
        )
        for _gfx, _label, sprite_id, dx, dy, dz, name, rect in grouped:
            lines.append(
                "    AirportStationGroundLayer { "
                f"sprite_id: {sprite_id}, dx: {f(dx)}, dy: {f(dy)}, dz: {f(dz)}, "
                f"w: {f(rect.w)}, h: {f(rect.h)}, x_offs: {f(rect.xrel)}, y_offs: {f(rect.yrel)}, "
                f"company_coloured: true, path: \"assets/opengfx/tiles/{name}\" }},"
            )
        lines.extend(["];", ""])

    lines.extend(
        [
            "/// Metadato NFO de un SpriteID airport usado por el renderer.",
            "#[must_use]",
            "pub fn airport_station_sprite_for_id(sprite_id: u32) -> Option<&'static AirportStationSprite> {",
            "    match sprite_id {",
            *[
                f"        {sprite_id} => Some(&AIRPORT_STATION_SPRITES[{index}]),"
                for index, sprite_id in enumerate(sorted(sprites))
            ],
            "        _ => None,",
            "    }",
            "}",
            "",
            "/// Base de suelo exacta para un `StationGfx` airport vanilla (0..=73).",
            "#[must_use]",
            "pub const fn airport_station_base_for_gfx(gfx: u8) -> Option<AirportStationBase> {",
            "    match gfx {",
            *[f"        {gfx} => Some(AIRPORT_GFX_{gfx}_BASE)," for gfx, _label, _id, _cc in AIRPORT_STATION_BASES],
            "        _ => None,",
            "    }",
            "}",
            "",
            "/// Capas ordenables `TILE_SEQ_LINE` del `StationGfx` airport.",
            "#[must_use]",
            "pub const fn airport_station_layers_for_gfx(gfx: u8) -> &'static [AirportStationLayer] {",
            "    match gfx {",
            *[f"        {gfx} => &AIRPORT_GFX_{gfx}_LAYERS," for gfx in sorted(layers_by_gfx)],
            "        _ => &[],",
            "    }",
            "}",
            "",
            "/// Capas `TILE_SEQ_GROUND` del `StationGfx` airport.",
            "#[must_use]",
            "pub const fn airport_station_ground_layers_for_gfx(gfx: u8) -> &'static [AirportStationGroundLayer] {",
            "    match gfx {",
            *[f"        {gfx} => &AIRPORT_GFX_{gfx}_GROUND_LAYERS," for gfx in sorted(ground_by_gfx)],
            "        _ => &[],",
            "    }",
            "}",
            "",
            "/// Alias histórico: SpriteID de la base `DrawGroundSprite`.",
            "#[must_use]",
            "pub const fn airport_station_ground_sprite_id_for_gfx(gfx: u8) -> Option<u32> {",
            "    match gfx {",
            *[f"        {gfx} => Some({sprite_id})," for gfx, _label, sprite_id, _cc in AIRPORT_STATION_BASES],
            "        _ => None,",
            "    }",
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="falla si la tabla generada difiere")
    args = parser.parse_args(argv)
    mode = detect_graphics_mode(REPO) or "8bpp"
    sprites, layers, ground_layers = airport_station_data(mode, write_tiles=not args.check)
    expected = render_output(sprites, layers, ground_layers, mode)
    if args.check:
        current = OUT_RS.read_text(encoding="utf-8") if OUT_RS.is_file() else None
        if current == expected:
            print(f"OK {OUT_RS.relative_to(REPO)}")
            return 0
        print(f"DRIFT {OUT_RS.relative_to(REPO)}", file=sys.stderr)
        return 1
    OUT_RS.write_text(expected, encoding="utf-8")
    print(f"Recortados {len(sprites)} sprites airport en {TILES_DIR.relative_to(REPO)}")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
