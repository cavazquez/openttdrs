#!/usr/bin/env python3
"""Genera vehicle_gfx_data_generated.rs desde sprites OpenGFX (bus, camión, tren).

OpenTTD road vehicles: `sprite = direction + _roadveh_images[spritenum]`.
Trenes: `GetDefaultTrainSprite(image_index, direction)` (`train_sprites.h`).
Índice 0..7 = `Direction` (`N`, `NE`, `E`, `SE`, `S`, `SW`, `W`, `NW`).
"""
from __future__ import annotations

import sys
from pathlib import Path

from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

# OpenTTD Direction 0..7
DIR_NAMES = ("n", "ne", "e", "se", "s", "sw", "w", "nw")
FALLBACK = (20.0, 16.0, -14.0, -7.0)

GFX_SETS: tuple[tuple[str, tuple[tuple[int, str], ...]], ...] = (
    (
        "BUS_VEHICLE_LAYERS",
        (
            (3092, "vehicle_bus_n.png"),
            (3093, "vehicle_bus_ne.png"),
            (3094, "vehicle_bus_e.png"),
            (3095, "vehicle_bus_se.png"),
            (3096, "vehicle_bus_s.png"),
            (3097, "vehicle_bus_sw.png"),
            (3098, "vehicle_bus_w.png"),
            (3099, "vehicle_bus_nw.png"),
        ),
    ),
    (
        "BUS_VEHICLE_LAYERS_LOADED",
        (
            (3180, "vehicle_bus_n_loaded.png"),
            (3181, "vehicle_bus_ne_loaded.png"),
            (3182, "vehicle_bus_e_loaded.png"),
            (3183, "vehicle_bus_se_loaded.png"),
            (3184, "vehicle_bus_s_loaded.png"),
            (3185, "vehicle_bus_sw_loaded.png"),
            (3186, "vehicle_bus_w_loaded.png"),
            (3187, "vehicle_bus_nw_loaded.png"),
        ),
    ),
    (
        "TRUCK_VEHICLE_LAYERS",
        (
            (3100, "vehicle_truck_n.png"),
            (3101, "vehicle_truck_ne.png"),
            (3102, "vehicle_truck_e.png"),
            (3103, "vehicle_truck_se.png"),
            (3104, "vehicle_truck_s.png"),
            (3105, "vehicle_truck_sw.png"),
            (3106, "vehicle_truck_w.png"),
            (3107, "vehicle_truck_nw.png"),
        ),
    ),
    (
        "TRUCK_VEHICLE_LAYERS_LOADED",
        (
            (3188, "vehicle_truck_n_loaded.png"),
            (3189, "vehicle_truck_ne_loaded.png"),
            (3190, "vehicle_truck_e_loaded.png"),
            (3191, "vehicle_truck_se_loaded.png"),
            (3192, "vehicle_truck_s_loaded.png"),
            (3193, "vehicle_truck_sw_loaded.png"),
            (3194, "vehicle_truck_w_loaded.png"),
            (3195, "vehicle_truck_nw_loaded.png"),
        ),
    ),
    (
        "SHIP_VEHICLE_LAYERS",
        (
            (3677, "vehicle_ship_mps_n.png"),
            (3678, "vehicle_ship_mps_ne.png"),
            (3679, "vehicle_ship_mps_e.png"),
            (3680, "vehicle_ship_mps_se.png"),
            (3681, "vehicle_ship_mps_s.png"),
            (3682, "vehicle_ship_mps_sw.png"),
            (3683, "vehicle_ship_mps_w.png"),
            (3684, "vehicle_ship_mps_nw.png"),
        ),
    ),
    (
        "SHIP_VEHICLE_LAYERS_OIL",
        (
            (3669, "vehicle_ship_oil_n.png"),
            (3670, "vehicle_ship_oil_ne.png"),
            (3671, "vehicle_ship_oil_e.png"),
            (3672, "vehicle_ship_oil_se.png"),
            (3673, "vehicle_ship_oil_s.png"),
            (3674, "vehicle_ship_oil_sw.png"),
            (3675, "vehicle_ship_oil_w.png"),
            (3676, "vehicle_ship_oil_nw.png"),
        ),
    ),
    (
        "SHIP_VEHICLE_LAYERS_COAL",
        (
            (3685, "vehicle_ship_coal_n.png"),
            (3686, "vehicle_ship_coal_ne.png"),
            (3687, "vehicle_ship_coal_e.png"),
            (3688, "vehicle_ship_coal_se.png"),
            (3689, "vehicle_ship_coal_s.png"),
            (3690, "vehicle_ship_coal_sw.png"),
            (3691, "vehicle_ship_coal_w.png"),
            (3692, "vehicle_ship_coal_nw.png"),
        ),
    ),
    (
        "SHIP_VEHICLE_LAYERS_FERRY",
        (
            (3693, "vehicle_ship_ferry_n.png"),
            (3694, "vehicle_ship_ferry_ne.png"),
            (3695, "vehicle_ship_ferry_e.png"),
            (3696, "vehicle_ship_ferry_se.png"),
            (3697, "vehicle_ship_ferry_s.png"),
            (3698, "vehicle_ship_ferry_sw.png"),
            (3699, "vehicle_ship_ferry_w.png"),
            (3700, "vehicle_ship_ferry_nw.png"),
        ),
    ),
    (
        "AIRCRAFT_VEHICLE_LAYERS",
        (
            (3765, "vehicle_aircraft_dakota_n.png"),
            (3766, "vehicle_aircraft_dakota_ne.png"),
            (3767, "vehicle_aircraft_dakota_e.png"),
            (3768, "vehicle_aircraft_dakota_se.png"),
            (3769, "vehicle_aircraft_dakota_s.png"),
            (3770, "vehicle_aircraft_dakota_sw.png"),
            (3771, "vehicle_aircraft_dakota_w.png"),
            (3772, "vehicle_aircraft_dakota_nw.png"),
        ),
    ),
    (
        "AIRCRAFT_VEHICLE_LAYERS_FOKKER",
        (
            (3773, "vehicle_aircraft_fokker_n.png"),
            (3774, "vehicle_aircraft_fokker_ne.png"),
            (3775, "vehicle_aircraft_fokker_e.png"),
            (3776, "vehicle_aircraft_fokker_se.png"),
            (3777, "vehicle_aircraft_fokker_s.png"),
            (3778, "vehicle_aircraft_fokker_sw.png"),
            (3779, "vehicle_aircraft_fokker_w.png"),
            (3780, "vehicle_aircraft_fokker_nw.png"),
        ),
    ),
    (
        # Tricario (image_index 9): helicóptero OpenGFX sprites 3813..3820.
        "AIRCRAFT_VEHICLE_LAYERS_TRICARIO",
        (
            (3813, "vehicle_aircraft_tricario_n.png"),
            (3814, "vehicle_aircraft_tricario_ne.png"),
            (3815, "vehicle_aircraft_tricario_e.png"),
            (3816, "vehicle_aircraft_tricario_se.png"),
            (3817, "vehicle_aircraft_tricario_s.png"),
            (3818, "vehicle_aircraft_tricario_sw.png"),
            (3819, "vehicle_aircraft_tricario_w.png"),
            (3820, "vehicle_aircraft_tricario_nw.png"),
        ),
    ),
    (
        "TRAIN_VEHICLE_LAYERS",
        (
            (2921, "vehicle_train_n.png"),
            (2922, "vehicle_train_ne.png"),
            (2923, "vehicle_train_e.png"),
            (2924, "vehicle_train_se.png"),
            (2925, "vehicle_train_s.png"),
            (2926, "vehicle_train_sw.png"),
            (2927, "vehicle_train_w.png"),
            (2928, "vehicle_train_nw.png"),
        ),
    ),
    (
        "TRAIN_VEHICLE_LAYERS_T0",
        (
            (2905, "vehicle_train_t0_n.png"),
            (2906, "vehicle_train_t0_ne.png"),
            (2907, "vehicle_train_t0_e.png"),
            (2908, "vehicle_train_t0_se.png"),
            (2909, "vehicle_train_t0_s.png"),
            (2910, "vehicle_train_t0_sw.png"),
            (2911, "vehicle_train_t0_w.png"),
            (2912, "vehicle_train_t0_nw.png"),
        ),
    ),
    (
        "TRAIN_VEHICLE_LAYERS_T1",
        (
            (2913, "vehicle_train_t1_n.png"),
            (2914, "vehicle_train_t1_ne.png"),
            (2915, "vehicle_train_t1_e.png"),
            (2916, "vehicle_train_t1_se.png"),
            (2917, "vehicle_train_t1_s.png"),
            (2918, "vehicle_train_t1_sw.png"),
            (2919, "vehicle_train_t1_w.png"),
            (2920, "vehicle_train_t1_nw.png"),
        ),
    ),
    (
        "TRAIN_VEHICLE_LAYERS_TDIESEL",
        (
            (2949, "vehicle_train_td_n.png"),
            (2950, "vehicle_train_td_ne.png"),
            (2951, "vehicle_train_td_e.png"),
            (2952, "vehicle_train_td_se.png"),
            (2953, "vehicle_train_td_s.png"),
            (2954, "vehicle_train_td_sw.png"),
            (2955, "vehicle_train_td_w.png"),
            (2956, "vehicle_train_td_nw.png"),
        ),
    ),
    (
        "TRAIN_VEHICLE_LAYERS_TELECTRIC",
        (
            (2965, "vehicle_train_te_n.png"),
            (2966, "vehicle_train_te_ne.png"),
            (2967, "vehicle_train_te_e.png"),
            (2968, "vehicle_train_te_se.png"),
            (2969, "vehicle_train_te_s.png"),
            (2970, "vehicle_train_te_sw.png"),
            (2971, "vehicle_train_te_w.png"),
            (2972, "vehicle_train_te_nw.png"),
        ),
    ),
)


KIRBY_FALLBACK = (
    "vehicle_train_n.png",
    "vehicle_train_ne.png",
    "vehicle_train_e.png",
    "vehicle_train_se.png",
    "vehicle_train_s.png",
    "vehicle_train_sw.png",
    "vehicle_train_w.png",
    "vehicle_train_nw.png",
)


def emit_layers(
    repo: Path,
    tiles_dir: Path,
    nfo: dict,
    prefer_bpp: str | None,
    set_name: str,
    entries: tuple[tuple[int, str], ...],
) -> tuple[list[str], list[str], int]:
    rows: list[str] = []
    missing: list[str] = []
    nfo_ok = 0
    rows.append(f"pub const {set_name}: [VehicleLayerGfx; 8] = [")
    for (dir_name, (sid, png)) in zip(DIR_NAMES, entries, strict=True):
        w, h, xr, yr, note = sprite_dims_from_assets(
            repo,
            tiles_dir,
            nfo,
            sid,
            png,
            prefer_bpp,
            fallback=FALLBACK,
        )
        if note.startswith("nfo"):
            nfo_ok += 1
        if not (tiles_dir / png).is_file():
            missing.append(png)
            png = KIRBY_FALLBACK[DIR_NAMES.index(dir_name)]
        rows.append(f"    // {dir_name.upper()} (sprite {sid})")
        rows.append(
            f"    VehicleLayerGfx {{ w: {w:.1f}, h: {h:.1f}, "
            f"x_offs: {xr:.1f}, y_offs: {yr:.1f}, "
            f'path: "assets/opengfx/tiles/{png}" }},'
        )
    rows.append("];")
    rows.append("")
    return rows, missing, nfo_ok


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "vehicle_gfx_data_generated.rs"
    )
    nfo = parse_sprite_offs(repo)
    prefer_bpp = detect_graphics_mode(repo)

    lines = [
        "// @generated by scripts/gen_vehicle_gfx_data.py — no editar a mano.",
        "// Fuente: OpenGFX bus/camión MPS + Kirby Paul Tank (sprites 2921..2928).",
        f"// Modo gráfico detectado: {prefer_bpp or 'desconocido'}.",
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct VehicleLayerGfx {",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub path: &'static str,",
        "}",
        "",
        "/// Índice = `Vehicle::render_direction()` (`OpenTTD` `Direction` 0..7).",
    ]

    all_missing: list[str] = []
    total_nfo = 0
    for set_name, entries in GFX_SETS:
        block, missing, nfo_ok = emit_layers(
            repo, tiles_dir, nfo, prefer_bpp, set_name, entries
        )
        lines.extend(block)
        all_missing.extend(missing)
        total_nfo += nfo_ok

    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {out_path} (nfo={total_nfo}/{8 * len(GFX_SETS)})")
    if all_missing:
        print(f"PNG ausentes (fallback Kirby): {sorted(set(all_missing))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
