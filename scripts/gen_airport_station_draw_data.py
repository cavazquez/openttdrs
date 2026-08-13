#!/usr/bin/env python3
"""Extrae y describe las capas vanilla de los piers de aeropuerto.

`DrawTile_Station` no representa `APT_PIER_NW_NE` (27) ni `APT_PIER` (28)
con el concourse: ambos parten de ``SPR_AIRPORT_APRON`` y suman una capa
``TILE_SEQ_LINE`` con ancla, bounding-box y paleta de compañía propios.

Este generador recorta las dos capas desde el NFO *global* del perfil OpenGFX
activo y emite la geometría que necesita el renderer. Elegir la alternativa
normal 8/32bpp del mismo SpriteID evita mezclar el rect de la hoja indexada
con el PNG HighDef.

Fuente semántica: `src/table/station_land.h` de OpenTTD:

* `APT_PIER_NW_NE`: `(3, 2, 0, 3, 3, 14, SPR_AIRPORT_JETWAY_3)`;
* `APT_PIER`: `(0, 8, 0, 14, 3, 14, SPR_AIRPORT_PASSENGER_TUNNEL)`.

Salidas:

* `assets/opengfx/tiles/airport_jetway_3.png`
* `assets/opengfx/tiles/airport_passenger_tunnel.png`
* `crates/openttdrs-client/src/sprites/airport_station_draw_data_generated.rs`

Uso:
  python3 scripts/gen_airport_station_draw_data.py
  python3 scripts/gen_airport_station_draw_data.py --check
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
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

# (StationGfx, etiqueta OpenTTD, SpriteID, dx, dy, dz, sx, sy, sz, png).
# Las cajas son las de `TILE_SEQ_LINE`, no las dimensiones del recorte PNG.
AIRPORT_STATION_OVERLAYS = (
    (27, "APT_PIER_NW_NE", 2661, 3, 2, 0, 3, 3, 14, "airport_jetway_3.png"),
    (28, "APT_PIER", 2662, 0, 8, 0, 14, 3, 14, "airport_passenger_tunnel.png"),
)


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    """Abre una hoja NFO con la conversión de transparencia del perfil activo."""

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


class AirportStationSpriteCropper:
    """Recorta IDs globales de aeropuerto desde el baseset gráfico activo."""

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

    def rect(self, sprite_id: int) -> SpriteRect:
        try:
            return self.rects[sprite_id]
        except KeyError as error:
            raise RuntimeError(
                f"sprite de aeropuerto {sprite_id} no está en el NFO activo"
            ) from error

    def crop(self, sprite_id: int, output_name: str) -> SpriteRect:
        rect = self.rect(sprite_id)
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
        return rect


def airport_station_layers(mode: str, *, write_tiles: bool) -> list[tuple]:
    """Devuelve las capas con metadata NFO y opcionalmente actualiza sus PNG."""

    cropper = AirportStationSpriteCropper(mode)
    layers = []
    for spec in AIRPORT_STATION_OVERLAYS:
        gfx, label, sprite_id, dx, dy, dz, sx, sy, sz, png = spec
        rect = cropper.crop(sprite_id, png) if write_tiles else cropper.rect(sprite_id)
        layers.append((
            gfx,
            label,
            sprite_id,
            dx,
            dy,
            dz,
            sx,
            sy,
            sz,
            png,
            rect.w,
            rect.h,
            rect.xrel,
            rect.yrel,
        ))
    return layers


def render_output(layers: list[tuple], mode: str) -> str:
    """Renderiza la tabla Rust sin depender de los PNG ya existentes."""

    by_gfx = {layer[0]: layer for layer in layers}
    lines = [
        "// Generado por scripts/gen_airport_station_draw_data.py — NO EDITAR A MANO.",
        "// Fuente: OpenTTD table/station_land.h (APT_PIER_NW_NE/APT_PIER) + NFO OpenGFX.",
        f"// Modo gráfico detectado: {mode}.",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
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
        "    pub path: &'static str,",
        "}",
        "",
    ]
    for gfx, const_name in ((27, "AIRPORT_PIER_NW_NE_LAYERS"), (28, "AIRPORT_PIER_LAYERS")):
        (
            _gfx,
            label,
            sprite_id,
            dx,
            dy,
            dz,
            sx,
            sy,
            sz,
            png,
            width,
            height,
            xrel,
            yrel,
        ) = by_gfx[gfx]
        lines.extend(
            [
                f"/// {label} (StationGfx {gfx}); capa `TILE_SEQ_LINE` coloreada por compañía.",
                f"pub static {const_name}: [AirportStationLayer; 1] = [",
                "    AirportStationLayer { "
                f"sprite_id: {sprite_id}, dx: {dx}.0, dy: {dy}.0, dz: {dz}.0, "
                f"sx: {sx}, sy: {sy}, sz: {sz}, z: 0.050, "
                f"w: {width}.0, h: {height}.0, x_offs: {xrel}.0, y_offs: {yrel}.0, "
                f'path: "assets/opengfx/tiles/{png}" }},',
                "];",
                "",
            ]
        )
    lines.extend(
        [
            "/// Capas ordenables de los `StationGfx` airport vanilla cubiertos.",
            "#[must_use]",
            "pub const fn airport_station_layers_for_gfx(gfx: u8) -> &'static [AirportStationLayer] {",
            "    match gfx {",
            "        27 => &AIRPORT_PIER_NW_NE_LAYERS,",
            "        28 => &AIRPORT_PIER_LAYERS,",
            "        _ => &[],",
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
    layers = airport_station_layers(mode, write_tiles=not args.check)
    expected = render_output(layers, mode)
    if args.check:
        current = OUT_RS.read_text(encoding="utf-8") if OUT_RS.is_file() else None
        if current == expected:
            print(f"OK {OUT_RS.relative_to(REPO)}")
            return 0
        print(f"DRIFT {OUT_RS.relative_to(REPO)}", file=sys.stderr)
        return 1
    OUT_RS.write_text(expected, encoding="utf-8")
    print(f"Recortadas {len(layers)} capas de aeropuerto en {TILES_DIR.relative_to(REPO)}")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
