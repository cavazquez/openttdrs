#!/usr/bin/env python3
"""Genera metadata NFO (w/h/xrel/yrel) de los sprites de estación de tren.

`_station_display_datas_rail` (`table/station_land.h`) dibuja cada tesela con
una secuencia TILE_SEQ: plataformas rear/front (1069–1078), edificio pequeño
(1073/1074) y estructura de techo (1079–1082, mitades A/B por eje). Para que
empalmen como en upstream hay que dibujarlas con sus offsets NFO + el origen
TILE_SEQ remapeado, no centradas en la tesela.

Salida: `crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs`.

Uso: python3 scripts/gen_rail_station_draw_data.py
"""
from __future__ import annotations

from pathlib import Path

from nfo_sprite_meta import (
    detect_graphics_mode,
    parse_sprite_offs,
    sprite_dims_from_assets,
)

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs"

# (sprite_id, png) — IDs de `table/sprites.h` (SPR_RAIL_PLATFORM_* / SPR_RAIL_ROOF_*).
SPRITES = [
    (1069, "rail_platform_y_front.png"),
    (1070, "rail_platform_x_rear.png"),
    (1071, "rail_platform_y_rear.png"),
    (1072, "rail_platform_x_front.png"),
    (1073, "rail_platform_building_x.png"),
    (1074, "rail_platform_building_y.png"),
    (1075, "rail_platform_pillars_y_front.png"),
    (1076, "rail_platform_pillars_x_rear.png"),
    (1077, "rail_platform_pillars_y_rear.png"),
    (1078, "rail_platform_pillars_x_front.png"),
    (1079, "rail_roof_0.png"),  # SPR_RAIL_ROOF_STRUCTURE_X_TILE_A
    (1080, "rail_roof_1.png"),  # SPR_RAIL_ROOF_STRUCTURE_Y_TILE_A
    (1081, "rail_roof_2.png"),  # SPR_RAIL_ROOF_STRUCTURE_X_TILE_B
    (1082, "rail_roof_3.png"),  # SPR_RAIL_ROOF_STRUCTURE_Y_TILE_B
]


def main() -> None:
    nfo = parse_sprite_offs(REPO)
    prefer = detect_graphics_mode(REPO)

    lines = [
        "// Generado por scripts/gen_rail_station_draw_data.py — NO EDITAR A MANO.",
        "//",
        "// Offsets NFO (sprite_id, w, h, xrel, yrel) de las piezas de estación de",
        "// tren (`_station_display_datas_rail`, `table/station_land.h`).",
        "",
        "/// Metadata NFO de plataformas, edificios y techos (1069–1082).",
        f"pub static RAIL_STATION_SPRITE_META: [(u32, f32, f32, f32, f32); {len(SPRITES)}] = [",
    ]
    for sid, png in SPRITES:
        w, h, xr, yr, note = sprite_dims_from_assets(
            REPO, TILES_DIR, nfo, sid, png, prefer
        )
        if note in ("sin_nfo", "macro"):
            raise SystemExit(f"sin metadata NFO para sprite {sid} ({png})")
        lines.append(
            f"    ({sid}, {w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f}), // {png} [{note}]"
        )
    lines += ["];", ""]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
