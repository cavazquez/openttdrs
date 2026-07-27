#!/usr/bin/env python3
"""Genera los frames de paleta del agua desde los índices NFO originales.

OpenTTD anima por separado los índices 245–249 (dark water, 5 fases) y
250–254 (glitter water, 15 fases). Los PNG RGBA no sirven para reconstruir
esos slots: varios índices glitter comparten el mismo RGB en la paleta base.
Por eso este generador recorta directamente los sheets 8bpp indexados que
describe el NFO de OpenGFX y recién entonces hornea cada combinación.

Salidas:
- ``water_anim_d{d:02}_g{g:02}.png`` (5 × 15)
- ``shore_full_{slot:02}_anim_d{d:02}_g{g:02}.png`` (18 × 5 × 15)
- ``water_palette_generated.rs`` con las constantes validadas por CI.

Uso: python3 scripts/gen_water_anim_frames.py
"""
from __future__ import annotations

import re
from pathlib import Path

from PIL import Image

from gen_shore_full_set import find_extra_nfo, parse_shore_blocks

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = (
    REPO
    / "crates"
    / "openttdrs-client"
    / "src"
    / "sprites"
    / "water_palette_generated.rs"
)

# ``_extra_palette_values`` de table/palettes.h (clima templado).
DARK_WATER = [
    (32, 68, 112),
    (36, 72, 116),
    (40, 76, 120),
    (44, 80, 124),
    (48, 84, 128),
]
GLITTER_WATER = [
    (216, 244, 252),
    (172, 208, 224),
    (132, 172, 196),
    (100, 132, 168),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (72, 100, 144),
    (100, 132, 168),
    (132, 172, 196),
    (172, 208, 224),
]
DARK_FRAME_COUNT = 5
GLITTER_FRAME_COUNT = 15
ANIMATED_INDICES = set(range(245, 255))

REAL_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def indexed_crop(sheet_path: Path, rect: tuple[int, ...]) -> Image.Image:
    """Recorta un sprite ``P`` sin colapsar índices con el mismo color."""
    x, y, w, h, _xrel, _yrel = rect
    sheet = Image.open(sheet_path)
    if sheet.mode != "P":
        raise SystemExit(f"{sheet_path} no es un sheet indexado (modo {sheet.mode})")
    return sheet.crop((x, y, x + w, y + h))


def find_flat_water_crop() -> Image.Image:
    """Localiza SPR_FLAT_WATER_TILE (4061) en el NFO base 8bpp."""
    roots = sorted(
        nfo
        for nfo in (REPO / "assets" / "opengfx").glob("*/sprites/*base*.nfo")
        if not nfo.parents[1].name.startswith(".")
    )
    for nfo in roots:
        for line in nfo.read_text(errors="replace").splitlines():
            match = REAL_RE.match(line)
            if not match or int(match.group(1)) != 4061 or match.group(3) != "8bpp":
                continue
            raw_sheet = Path(match.group(2))
            sheet_path = raw_sheet if raw_sheet.is_absolute() else nfo.parent / raw_sheet.name
            rect = tuple(int(match.group(i)) for i in range(4, 10))
            if sheet_path.is_file():
                return indexed_crop(sheet_path, rect)
    raise SystemExit("No se encontró SPR_FLAT_WATER_TILE 4061 indexado en el NFO base")


def indexed_sources() -> list[tuple[str, Image.Image]]:
    """Agua plana y set completo de orillas, aún en índices de paleta."""
    sources = [("water", find_flat_water_crop())]
    shore_slots = parse_shore_blocks(find_extra_nfo())
    for slot in range(18):
        sheet, rect = shore_slots[slot]
        sources.append((f"shore_full_{slot:02}", indexed_crop(sheet, rect)))
    return sources


def validate_palette_sources(sources: list[tuple[str, Image.Image]]) -> None:
    """Falla si OpenGFX deja de cubrir los diez índices animables."""
    water_indices = set(sources[0][1].get_flattened_data()) & ANIMATED_INDICES
    if water_indices != ANIMATED_INDICES:
        missing = sorted(ANIMATED_INDICES - water_indices)
        raise SystemExit(f"SPR_FLAT_WATER_TILE no cubre índices animables: faltan {missing}")
    all_indices: set[int] = set()
    for _name, image in sources:
        all_indices.update(set(image.get_flattened_data()) & ANIMATED_INDICES)
    if all_indices != ANIMATED_INDICES:
        missing = sorted(ANIMATED_INDICES - all_indices)
        raise SystemExit(f"Set agua/orillas incompleto: faltan índices {missing}")


def render_frame(base: Image.Image, dark_frame: int, glitter_frame: int) -> Image.Image:
    """Convierte un recorte indexado en RGBA para las dos fases indicadas."""
    rgba = base.convert("RGBA")
    palette = base.getpalette()
    transparent_rgb = tuple(palette[:3]) if palette else None
    src = list(base.get_flattened_data())
    dst = list(rgba.get_flattened_data())
    for i, palette_index in enumerate(src):
        if palette_index == 0:
            dst[i] = (0, 0, 0, 0)
        elif 245 <= palette_index <= 249:
            slot = palette_index - 245
            dst[i] = (*DARK_WATER[(slot + dark_frame) % DARK_FRAME_COUNT], 255)
        elif 250 <= palette_index <= 254:
            slot = palette_index - 250
            index = (glitter_frame + 3 * slot) % GLITTER_FRAME_COUNT
            dst[i] = (*GLITTER_WATER[index], 255)
        elif transparent_rgb is not None and dst[i][:3] == transparent_rgb:
            dst[i] = (0, 0, 0, 0)
    rgba.putdata(dst)
    return rgba


def generated_rust() -> str:
    dark = ",\n    ".join(f"[{r}, {g}, {b}]" for r, g, b in DARK_WATER)
    glitter = ",\n    ".join(f"[{r}, {g}, {b}]" for r, g, b in GLITTER_WATER)
    return f"""// Generado por scripts/gen_water_anim_frames.py — NO EDITAR A MANO.
// Fuente: OpenTTD table/palettes.h + índices NFO OpenGFX validados 245..254.

pub const DARK_WATER_FRAME_COUNT: usize = {DARK_FRAME_COUNT};
pub const GLITTER_WATER_FRAME_COUNT: usize = {GLITTER_FRAME_COUNT};
pub const WATER_PALETTE_FRAME_COUNT: usize = {DARK_FRAME_COUNT * GLITTER_FRAME_COUNT};

#[allow(dead_code)]
pub const DARK_WATER_RGB: [[u8; 3]; DARK_WATER_FRAME_COUNT] = [
    {dark},
];
#[allow(dead_code)]
pub const GLITTER_WATER_RGB: [[u8; 3]; GLITTER_WATER_FRAME_COUNT] = [
    {glitter},
];
"""


def main() -> None:
    sources = indexed_sources()
    validate_palette_sources(sources)
    total = 0
    for stem, base in sources:
        for dark_frame in range(DARK_FRAME_COUNT):
            for glitter_frame in range(GLITTER_FRAME_COUNT):
                render_frame(base, dark_frame, glitter_frame).save(
                    TILES_DIR
                    / f"{stem}_anim_d{dark_frame:02}_g{glitter_frame:02}.png"
                )
                total += 1
    rust = generated_rust()
    if not OUT_RS.is_file() or OUT_RS.read_text(encoding="utf-8") != rust:
        OUT_RS.write_text(rust, encoding="utf-8")
    print(f"Generados {total} frames indexados de agua/orilla en {TILES_DIR}")
    print(f"Validada cobertura de índices NFO 245..254; escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
