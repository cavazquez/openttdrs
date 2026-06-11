#!/usr/bin/env python3
"""Genera metadata NFO (w/h/xrel/yrel) de los sprites del puente de madera.

`DrawBridgeMiddle` (`tunnelbridge_cmd.cpp`) dibuja cada vano con tres sprites
de `_bridge_sprite_table_wood_middle` (`table/bridge_land.h`): rear (suelo +
barandilla trasera), front (barandilla frontal, desplazada +12 unidades de
mundo hacia la cámara) y pillar (columna por nivel de altura). Para que los
segmentos empalmen como en upstream hay que dibujarlos con sus offsets NFO,
no centrados en la tesela.

IDs (`table/sprites.h`): rear rail Y/X = 2545/2546, rear road Y/X = 2547/2548,
front Y/X = 2549/2550, pillar Y/X = 2551/2552.

Salida: `crates/openttdrs-client/src/sprites/bridge_draw_data_generated.rs`.

Uso: python3 scripts/gen_bridge_draw_data.py
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
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/bridge_draw_data_generated.rs"

# (sprite_id, png) en el orden de los arrays generados: índice 0 = eje X, 1 = eje Y.
REAR_ROAD = [(2548, "bridge_wood_road_x.png"), (2547, "bridge_wood_road_y.png")]
REAR_RAIL = [(2546, "bridge_wood_rail_x.png"), (2545, "bridge_wood_rail_y.png")]
FRONT = [(2550, "bridge_wood_x_front.png"), (2549, "bridge_wood_y_front.png")]
PILLAR = [(2552, "bridge_wood_x_pillar.png"), (2551, "bridge_wood_y_pillar.png")]


def main() -> None:
    nfo = parse_sprite_offs(REPO)
    prefer = detect_graphics_mode(REPO)

    def meta_line(sid: int, png: str) -> str:
        w, h, xr, yr, note = sprite_dims_from_assets(
            REPO, TILES_DIR, nfo, sid, png, prefer
        )
        if note in ("sin_nfo", "macro"):
            raise SystemExit(f"sin metadata NFO para sprite {sid} ({png})")
        return f"    ({w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f}), // {sid} {png} [{note}]"

    def array(name: str, doc: str, pairs: list[tuple[int, str]]) -> list[str]:
        lines = [f"/// {doc}", f"pub static {name}: [(f32, f32, f32, f32); 2] = ["]
        lines += [meta_line(sid, png) for sid, png in pairs]
        lines.append("];")
        return lines

    out = [
        "// Generado por scripts/gen_bridge_draw_data.py — NO EDITAR A MANO.",
        "//",
        "// Offsets NFO (w, h, xrel, yrel) de `_bridge_sprite_table_wood_middle`",
        "// (`table/bridge_land.h`): rear (suelo + barandilla trasera), front y",
        "// pillar, indexados por eje del puente (0 = X, 1 = Y).",
        "",
    ]
    out += array(
        "BRIDGE_WOOD_REAR_ROAD_META",
        "Rear carretera (2548/2547) por eje.",
        REAR_ROAD,
    )
    out.append("")
    out += array(
        "BRIDGE_WOOD_REAR_RAIL_META",
        "Rear ferrocarril (2546/2545) por eje.",
        REAR_RAIL,
    )
    out.append("")
    out += array("BRIDGE_WOOD_FRONT_META", "Barandilla frontal (2550/2549) por eje.", FRONT)
    out.append("")
    out += array("BRIDGE_WOOD_PILLAR_META", "Pilar (2552/2551) por eje.", PILLAR)
    out.append("")
    OUT_RS.write_text("\n".join(out), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
