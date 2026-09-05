#!/usr/bin/env python3
"""Genera sprites y metadata NFO de las estaciones ferroviarias vanilla.

`_station_display_datas_rail` (`table/station_land.h`) dibuja cada tesela con
una secuencia TILE_SEQ: plataformas rear/front (1069–1078), edificio pequeño
(1073/1074) y estructura de techo (1079–1082, mitades A/B por eje). Para que
empalmen como en upstream hay que dibujarlas con sus offsets NFO + el origen
TILE_SEQ remapeado, no centradas en la tesela.

OpenTTD suma ``RailTypeInfo::GetRailtypeSpriteOffset()`` a **cada** sprite de
la secuencia. Por eso los 18 sprites rail/elrail (1069–1086) tienen sus
familias monorail (+82, 1151–1168) y maglev (+164, 1233–1250). Los PNG de
estas dos últimas familias se recortan aquí desde el NFO del perfil activo;
no son copias del arte rail normal.

Salida: `crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs`.

Uso: python3 scripts/gen_rail_station_draw_data.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
    active_global_sprite_nfo,
    detect_graphics_mode,
    parse_global_sprite_rects,
    parse_sprite_offs,
    sprite_dims_from_assets,
)
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba
from pillow_compat import flattened_data

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs"

# (sprite_id, png) — IDs de `table/sprites.h` (SPR_RAIL_PLATFORM_* / SPR_RAIL_ROOF_*).
BASE_STATION_SPRITES = [
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
    (1083, "rail_roof_4.png"),  # SPR_RAIL_ROOF_GLASS_X_TILE_A
    (1084, "rail_roof_5.png"),  # SPR_RAIL_ROOF_GLASS_Y_TILE_A
    (1085, "rail_roof_6.png"),  # SPR_RAIL_ROOF_GLASS_X_TILE_B
    (1086, "rail_roof_7.png"),  # SPR_RAIL_ROOF_GLASS_Y_TILE_B
]

# Waypoints ogfx2_stations (cuerpo + toldos CC; ver gen_rail_waypoint_sprites.py).
WAYPOINT_SPRITES = [
    (4974, "rail_4974.png"),
    (4975, "rail_4975.png"),
    (4978, "rail_4978.png"),
    (4979, "rail_4979.png"),
    (4976, "rail_4976.png"),
    (4977, "rail_4977.png"),
    (4980, "rail_4980.png"),
    (4981, "rail_4981.png"),
]

# `RailTypeInfo::GetRailtypeSpriteOffset()` para rail/elrail, mono y maglev.
RAILTYPE_STATION_OFFSETS = (0, 82, 164)


def station_sprite_entries() -> list[tuple[int, str]]:
    """Entradas de metadata de las tres familias de estación y waypoints."""

    entries = [
        (sprite_id + offset, png if offset == 0 else f"rail_{sprite_id + offset}.png")
        for sprite_id, png in BASE_STATION_SPRITES
        for offset in RAILTYPE_STATION_OFFSETS
    ]
    entries.extend(WAYPOINT_SPRITES)
    return sorted(entries)


SPRITES = station_sprite_entries()
TYPED_STATION_SPRITE_IDS = tuple(
    sprite_id + offset
    for offset in RAILTYPE_STATION_OFFSETS[1:]
    for sprite_id, _png in BASE_STATION_SPRITES
)

# Metadata NFO de ogfx2_stations (casetas + toldos CC; paridad OpenTTD+OpenGFX2).
STATIONS_WAYPOINT_NFO_META: dict[int, tuple[float, float, float, float]] = {
    4974: (40.0, 29.0, -30.0, -9.0),
    4975: (40.0, 29.0, -8.0, -9.0),
    4978: (23.0, 14.0, -23.0, -5.0),
    4979: (23.0, 14.0, 2.0, -5.0),
    4976: (38.0, 28.0, -28.0, -8.0),
    4977: (38.0, 28.0, -8.0, -8.0),
    4980: (23.0, 14.0, -23.0, -5.0),
    4981: (23.0, 14.0, 2.0, -5.0),
}


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    """Abre una hoja NFO sin mezclar el colorkey 8bpp con el perfil 32bpp."""

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
                        for pixel in flattened_data(rgba)
                    ]
                )
            return rgba
        return image.convert("RGBA")
    if image.mode == "P":
        return indexed_dos_to_rgba(image)
    return dematte_legacy_colorkey(image)


class StationSpriteCropper:
    """Recorta IDs globales de estación desde el baseset gráfico activo."""

    def __init__(self, mode: str) -> None:
        nfo_path = active_global_sprite_nfo(REPO, mode)
        if nfo_path is None:
            raise FileNotFoundError(
                "No hay NFO OpenGFX base activo; ejecutá scripts/descargar_graficos.sh"
            )
        self.mode = mode
        self.sprites_dir = nfo_path.parent
        self.rects = parse_global_sprite_rects(nfo_path, mode)
        self.sheets: dict[str, Image.Image] = {}

    def crop(self, sprite_id: int, output_name: str) -> None:
        try:
            rect = self.rects[sprite_id]
        except KeyError as error:
            raise RuntimeError(f"sprite de estación {sprite_id} no está en el NFO activo") from error
        if rect.sheet not in self.sheets:
            source = self.sprites_dir / rect.sheet
            if not source.is_file():
                source = source.with_suffix(".pcx")
            if not source.is_file():
                raise FileNotFoundError(f"no existe hoja OpenGFX {source}")
            self.sheets[rect.sheet] = load_sheet(source, self.mode)
        TILES_DIR.mkdir(parents=True, exist_ok=True)
        self.sheets[rect.sheet].crop(
            (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h)
        ).save(TILES_DIR / output_name)


def extract_typed_station_sprites(mode: str) -> None:
    """Actualiza mono/maglev desde las filas exactas del NFO activo."""

    cropper = StationSpriteCropper(mode)
    for sprite_id in TYPED_STATION_SPRITE_IDS:
        cropper.crop(sprite_id, f"rail_{sprite_id}.png")


def main() -> None:
    nfo = parse_sprite_offs(REPO)
    prefer = detect_graphics_mode(REPO) or "8bpp"
    extract_typed_station_sprites(prefer)

    lines = [
        "// Generado por scripts/gen_rail_station_draw_data.py — NO EDITAR A MANO.",
        "//",
        "// Offsets NFO (sprite_id, w, h, xrel, yrel) de las piezas de estación de",
        "// tren (`_station_display_datas_rail`, `table/station_land.h`).",
        "",
        "/// Metadata NFO de estaciones rail/mono/maglev (1069–1250) y waypoints (4974–4981).",
        f"pub static RAIL_STATION_SPRITE_META: [(u32, f32, f32, f32, f32); {len(SPRITES)}] = [",
    ]
    for sid, png in SPRITES:
        if sid in STATIONS_WAYPOINT_NFO_META:
            w, h, xr, yr = STATIONS_WAYPOINT_NFO_META[sid]
            note = "ogfx2_stations"
        else:
            w, h, xr, yr, note = sprite_dims_from_assets(
                REPO, TILES_DIR, nfo, sid, png, prefer
            )
            if note == "sin_nfo" and sid == 4977 and (TILES_DIR / "rail_4976.png").is_file():
                w, h, xr, yr, note = sprite_dims_from_assets(
                    REPO, TILES_DIR, nfo, 4976, "rail_4976.png", prefer
                )
                note = f"{note}_from_4976"
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
