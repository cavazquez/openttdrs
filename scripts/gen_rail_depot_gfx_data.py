#!/usr/bin/env python3
"""Genera metadata de depósitos ferroviarios vanilla para rail/mono/maglev.

Los sprites de los depósitos no pertenecen al bloque de vía 1005..1038:
OpenTTD los elige mediante ``RailTypeInfo::GetRailtypeSpriteOffset()``.
Por eso hay que exportar tanto los PNG de monorriel/maglev como sus offsets
NFO propios; reutilizar las capas de riel normal hace que los edificios y sus
puertas no coincidan con el medio de transporte real.

Entrada: ``assets/opengfx/`` descargado por ``scripts/descargar_graficos.sh``.
Salida: ``crates/openttdrs-client/src/sprites/rail_depot_gfx_data_generated.rs``.

También (re)extrae los PNG en ``assets/opengfx/tiles/``. Así la tabla Rust y
los recortes proceden siempre del mismo NFO y no pueden divergir al alternar
entre OpenGFX 8bpp y OpenGFX2 32bpp.
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba
from pillow_compat import flattened_data


REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates" / "openttdrs-client" / "src" / "sprites" / "rail_depot_gfx_data_generated.rs"

# ``_depot_gfx_NE..NW`` de OpenTTD ``src/table/track_land.h``. Cada fila es
# (nombre PNG base, sprite rail normal, dx, dy, sx, sy). ``dz`` siempre es 0 y
# ``sz`` es 23 por ``TILE_SEQ_LINE``.
LAYOUT: tuple[tuple[tuple[str, int, int, int, int, int], ...], ...] = (
    (("ne", 1067, 2, 13, 13, 1),),
    (("se_1", 1063, 2, 2, 1, 13), ("se_2", 1064, 13, 2, 1, 13)),
    (("sw_1", 1065, 2, 2, 13, 1), ("sw_2", 1066, 2, 13, 13, 1)),
    (("nw", 1068, 13, 2, 1, 13),),
)

# ``RailTypeInfo::GetRailtypeSpriteOffset()``. Electric conserva los sprites
# de riel normal, de modo que la tabla visual sólo necesita tres variantes.
VARIANTS: tuple[tuple[str, int, str], ...] = (
    ("rail", 0, ""),
    ("monorail", 82, "mono_"),
    ("maglev", 164, "maglev_"),
)


def nfo_path_for_mode(mode: str | None) -> Path:
    opengfx = REPO / "assets" / "opengfx"
    if mode == "32bpp":
        path = opengfx / "opengfx2-32ez" / "sprites" / "ogfx21_base_32ez.nfo"
        if path.is_file():
            return path
    for base in sorted(opengfx.glob("opengfx-*"), reverse=True):
        path = base / "sprites" / "ogfx1_base.nfo"
        if path.is_file():
            return path
    fallback = opengfx / ".signal-src-8bpp" / "sprites" / "ogfx1_base.nfo"
    if fallback.is_file():
        return fallback
    raise FileNotFoundError(
        "No hay NFO base de OpenGFX; ejecutá ./scripts/descargar_graficos.sh"
    )


def parse_rows(path: Path) -> dict[int, tuple[str, int, int, int, int, int, int]]:
    """Lee ID -> (sheet, x, y, w, h, xrel, yrel) del NFO elegido.

    Los IDs de los depósitos tienen una sola entrada 8bpp incluso en el
    paquete 32bpp; usar el NFO del mismo paquete que produjo los PNG evita
    mezclar offsets de la cache 8bpp auxiliar.
    """
    import re

    pattern = re.compile(
        r"^\s*(\d+)\s+(\S+)\s+(?:8bpp|32bpp)\s+"
        r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    rows: dict[int, tuple[str, int, int, int, int, int, int]] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = pattern.match(line)
        if match is None:
            continue
        sid = int(match.group(1))
        rows[sid] = (
            Path(match.group(2)).name,
            *(int(match.group(i)) for i in range(3, 9)),
        )
    return rows


def png_name(prefix: str, name: str) -> str:
    return f"rail_depot_{prefix}{name}.png"


def load_sheet(path: Path, mode: str | None) -> Image.Image:
    """Abre una hoja respetando el color transparente del paquete OpenGFX."""
    image = Image.open(path)
    if image.mode == "P":
        if mode != "32bpp":
            return indexed_dos_to_rgba(image)
        palette = image.getpalette()
        transparent = tuple(palette[:3]) if palette else None
        rgba = image.convert("RGBA")
        if transparent is not None:
            rgba.putdata(
                [
                    (0, 0, 0, 0) if pixel[:3] == transparent else pixel
                    for pixel in flattened_data(rgba)
                ]
            )
        return rgba
    return image.convert("RGBA") if mode == "32bpp" else dematte_legacy_colorkey(image)


def extract_png(
    file_name: str,
    row: tuple[str, int, int, int, int, int, int],
    nfo_path: Path,
    mode: str | None,
    sheets: dict[str, Image.Image],
) -> None:
    sheet_name, x, y, width, height, _xrel, _yrel = row
    if sheet_name not in sheets:
        source = nfo_path.parent / sheet_name
        alternative = source.with_suffix(".pcx")
        if not source.is_file() and alternative.is_file():
            source = alternative
        if not source.is_file():
            raise RuntimeError(f"No existe la hoja {source.relative_to(REPO)}")
        sheets[sheet_name] = load_sheet(source, mode)
    crop = sheets[sheet_name].crop((x, y, x + width, y + height))
    TILES.mkdir(parents=True, exist_ok=True)
    crop.save(TILES / file_name)


def write_variant(
    variant_name: str,
    sprite_offset: int,
    prefix: str,
    rows: dict[int, tuple[str, int, int, int, int, int, int]],
    nfo_path: Path,
    mode: str | None,
    sheets: dict[str, Image.Image],
) -> list[str]:
    lines: list[str] = ["    ["]
    for dir_index, direction in enumerate(LAYOUT):
        lines.append("        &[")
        for layer_index, (name, normal_sid, dx, dy, sx, sy) in enumerate(direction):
            sid = normal_sid + sprite_offset
            try:
                row = rows[sid]
            except KeyError as error:
                raise RuntimeError(
                    f"El NFO no contiene sprite {sid} ({variant_name}/{name})"
                ) from error
            file_name = png_name(prefix, name)
            extract_png(file_name, row, nfo_path, mode, sheets)
            _sheet, _x, _y, width, height, xrel, yrel = row
            # Las capas se emiten en el orden de ``DrawRailTileSeq``.
            z = 0.05 + layer_index * 0.01
            lines.append(
                "            RailDepotLayerGfx { "
                f"sprite_id: {sid}, dx: {dx}.0, dy: {dy}.0, dz: 0.0, "
                f"sx: {sx}, sy: {sy}, z: {z:.2f}, w: {width}.0, h: {height}.0, "
                f"x_offs: {xrel}.0, y_offs: {yrel}.0, "
                f'path: "assets/opengfx/tiles/{file_name}" }},'
            )
        lines.append("        ],")
    lines.append("    ],")
    return lines


def main() -> int:
    mode = detect_graphics_mode(REPO)
    nfo_path = nfo_path_for_mode(mode)
    rows = parse_rows(nfo_path)
    sheets: dict[str, Image.Image] = {}
    variant_rows: list[str] = []
    for variant_name, sprite_offset, prefix in VARIANTS:
        variant_rows.extend(
            write_variant(variant_name, sprite_offset, prefix, rows, nfo_path, mode, sheets)
        )

    mode_comment = mode or "auto"
    lines = [
        "// @generated by scripts/gen_rail_depot_gfx_data.py — no editar a mano.",
        "// Fuente: OpenTTD src/table/track_land.h + OpenGFX NFO.",
        f"// Modo gráfico detectado: {mode_comment}.",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct RailDepotLayerGfx {",
        "    pub sprite_id: u32,",
        "    pub dx: f32,",
        "    pub dy: f32,",
        "    pub dz: f32,",
        "    pub sx: i32,",
        "    pub sy: i32,",
        "    pub z: f32,",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub path: &'static str,",
        "}",
        "",
        "/// Índice: rail/electric, monorail, maglev; luego dirección NE/SE/SW/NW.",
        "pub const RAIL_DEPOT_BUILD_LAYERS_BY_TYPE: [[&[RailDepotLayerGfx]; 4]; 3] = [",
        *variant_rows,
        "];",
        "",
    ]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    layer_count = sum(len(direction) for direction in LAYOUT) * len(VARIANTS)
    print(
        f"Escrito {OUT_RS.relative_to(REPO)} ({layer_count} capas; NFO {nfo_path.relative_to(REPO)})"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (FileNotFoundError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
