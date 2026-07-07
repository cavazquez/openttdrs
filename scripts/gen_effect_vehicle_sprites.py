#!/usr/bin/env python3
"""Extrae sprites de EffectVehicle (humo tren, chispas, explosión, avería).

OpenTTD `table/sprites.h` + `effectvehicle.cpp`:
  - SPR_DIESEL_SMOKE_0..5     = 3073..3078
  - SPR_STEAM_SMOKE_0..4      = 3079..3083
  - SPR_ELECTRIC_SPARK_0..5   = 3084..3089
  - SPR_EXPLOSION_LARGE_0..F  = 3709..3724
  - SPR_BREAKDOWN_SMOKE_0..3  = 3737..3740

Uso: python3 scripts/gen_effect_vehicle_sprites.py
"""
from __future__ import annotations

from gen_field_draw_data import REPO, TILES_DIR, Cropper
from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

OUT_RS = REPO / "crates/openttdrs-client/src/sprites/effect_vehicle_draw_data_generated.rs"

# (png_prefix, first_sprite_id, frame_count, rust_const_prefix)
GROUPS: list[tuple[str, int, int, str]] = [
    ("diesel_smoke", 3073, 6, "DIESEL_SMOKE"),
    ("steam_smoke", 3079, 5, "STEAM_SMOKE"),
    ("electric_spark", 3084, 6, "ELECTRIC_SPARK"),
    ("explosion_large", 3709, 16, "EXPLOSION_LARGE"),
    ("breakdown_smoke", 3737, 4, "BREAKDOWN_SMOKE"),
]


def emit_group(
    lines: list[str],
    nfo,
    mode: str,
    png_prefix: str,
    first_id: int,
    count: int,
    const: str,
) -> None:
    lines += [
        f"pub const {const}_FRAMES: usize = {count};",
        "",
        f"/// (w, h, xrel, yrel) de `{png_prefix}_{{i}}.png`.",
        f"pub static {const}_META: [(f32, f32, f32, f32); {count}] = [",
    ]
    for i in range(count):
        sid = first_id + i
        png = f"{png_prefix}_{i}.png"
        w, h, xr, yr, _ = sprite_dims_from_assets(REPO, TILES_DIR, nfo, sid, png, mode)
        lines.append(f"    ({w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f}),")
    lines += ["];", ""]


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    cropper = Cropper(mode)
    for png_prefix, first_id, count, _ in GROUPS:
        for i in range(count):
            cropper.crop(first_id + i, f"{png_prefix}_{i}.png")
        print(f"Recortados {count} frames → {png_prefix}_*.png")

    nfo = parse_sprite_offs(REPO)
    lines = [
        "// Generado por scripts/gen_effect_vehicle_sprites.py — NO EDITAR A MANO.",
        "//",
        "// EffectVehicle: humo vapor/diésel, chispas eléctricas, explosión, avería.",
        "",
    ]
    for png_prefix, first_id, count, const in GROUPS:
        emit_group(lines, nfo, mode, png_prefix, first_id, count, const)

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
