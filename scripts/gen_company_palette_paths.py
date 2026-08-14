#!/usr/bin/env python3
"""Genera la lista de PNG con paleta de compañía para el cliente.

Lee paths de los `.rs` generados (vehículos, paradas, depósitos) y emite
`company_palette_paths_generated.rs`.

Uso: python3 scripts/gen_company_palette_paths.py
"""
from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "crates/openttdrs-client/src/sprites"
OUT = SPRITES / "company_palette_paths_generated.rs"

PATH_RE = re.compile(r'path:\s*"assets/opengfx/tiles/([^"]+)"')


def paths_from_file(path: Path) -> list[str]:
    if not path.is_file():
        return []
    return sorted(set(PATH_RE.findall(path.read_text(encoding="utf-8", errors="replace"))))


def main() -> None:
    sources = [
        SPRITES / "vehicle_gfx_data_generated.rs",
        SPRITES / "road_stop_gfx_data_generated.rs",
        SPRITES / "road_depot_gfx_data_generated.rs",
        SPRITES / "rail_depot_gfx_data_generated.rs",
    ]
    paths: set[str] = set()
    for src in sources:
        paths.update(paths_from_file(src))
    # Depósitos de vía (paths en rail.rs, no en generated depot file).
    rail_rs = (SPRITES / "rail.rs").read_text(encoding="utf-8", errors="replace")
    paths.update(PATH_RE.findall(rail_rs))
    # Excluir suelos neutros (sin PALETTE_MODIFIER_COLOUR en upstream).
    paths.discard("road_depot_ground.png")
    # Los suelos de bahías bus/camión sí llevan `PALETTE_MODIFIER_COLOUR`
    # (`_station_display_datas_{bus,truck}` en station_land.h), aunque no
    # aparezcan en la tabla BUILD generada. Sin ellos el edificio se colorea
    # pero el andén queda siempre azul oscuro.
    paths.update(
        {
            "bus_stop_ne_ground.png",
            "bus_stop_se_ground.png",
            "bus_stop_sw_ground.png",
            "bus_stop_nw_ground.png",
            "truck_stop_ground_0.png",
            "truck_stop_ground_1.png",
            "truck_stop_ground_2.png",
            "truck_stop_ground_3.png",
        }
    )
    # `DrawShipDepotSprite` usa PALETTE_MODIFIER_COLOUR para las seis capas
    # vanilla del depósito naval; no están descriptas por una tabla Rust.
    paths.update(
        {
            # Capas airport vanilla que llevan `PALETTE_MODIFIER_COLOUR`.
            # La tabla de StationGfx incluye terminales, hangares, helipads,
            # cercas, piers y las cuatro variantes de la manga de viento.
            # No incluir los suelos, radar ni transmisor: son neutros en
            # `station_land.h` y recolorearlos sería otra discrepancia.
            "airport_terminal_a.png",
            "airport_tower.png",
            "airport_concourse.png",
            "airport_terminal_b.png",
            "airport_terminal_c.png",
            "airport_hangar_front.png",
            "airport_hangar_rear.png",
            "airport_airfield_hangar_front.png",
            "airport_airfield_hangar_rear.png",
            "airport_jetway_1.png",
            "airport_jetway_2.png",
            "airport_jetway_3.png",
            "airport_passenger_tunnel.png",
            "airport_fence_x.png",
            "airport_fence_y.png",
            "airport_airfield_terminal_c_ground.png",
            "airport_airfield_terminal_c_build.png",
            "airport_heliport.png",
            "airport_helidepot_office.png",
            "airport_wind_0.png",
            "airport_wind_1.png",
            "airport_wind_2.png",
            "airport_wind_3.png",
            "ship_depot_se_front.png",
            "ship_depot_sw_front.png",
            "ship_depot_nw.png",
            "ship_depot_ne.png",
            "ship_depot_se_rear.png",
            "ship_depot_sw_rear.png",
        }
    )
    ordered = sorted(paths)
    lines = [
        "// Generado por scripts/gen_company_palette_paths.py — NO EDITAR A MANO.",
        "",
        f"pub static COMPANY_PALETTE_STATIC_PATHS: [&str; {len(ordered)}] = [",
    ]
    for p in ordered:
        lines.append(f'    "{p}",')
    lines += ["];", ""]
    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT.relative_to(REPO)} ({len(ordered)} paths)")


if __name__ == "__main__":
    main()
