#!/usr/bin/env python3
"""Extrae el set completo de orillas (SPR_SHORE_BASE, 18 sprites) de OpenGFX.

El GRF base conserva los ocho sprites originales de TTD (4062..4069), que
OpenTTD relocaliza a SPR_SHORE_BASE. El GRF *extra*
(`ogfx2e_extra_32ez.grf` / `ogfxe_extra.grf`) aporta por **Action5 tipo 0x0D**
los diez restantes:

- Un bloque de **10 sprites** ("missing shore sprites", `newgrf_act5.cpp`)
  que mapea en orden a SPR_SHORE_BASE + [0, 5, 7, 10, 11, 13, 14, 15, 16, 17]
  (STEEP_S, STEEP_W, WSE, STEEP_N, NWS, ENW, SEN, STEEP_E, EW, NS).

OpenGFX 8.0 usa ese esquema. Algunos sets históricos también incluyen un bloque
de **16 sprites** (A5BLOCK_FIXED), que reemplaza SPR_SHORE_BASE + 0..15; se
prefiere cuando está presente. Los bloques aparecen una vez por clima,
condicionados con Action7; el primero de cada tamaño es el clima templado.

Salidas:
- `assets/opengfx/tiles/shore_full_{slot:02d}.png` (slots 0..17)
- `crates/openttdrs-client/src/sprites/shore_draw_data_generated.rs` con
  `SHORE_META` (w/h/xrel/yrel del NFO) y `TILEH_TO_SHORE_SPRITE`
  (tabla completa `tileh_to_shoresprite` de `water_cmd.cpp`, incluido steep).

Uso: python3 scripts/gen_shore_full_set.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/shore_draw_data_generated.rs"

SHORE_SPRITE_COUNT = 18
# Orden del bloque de 10 ("missing shore sprites", newgrf_act5.cpp).
MISSING_BLOCK_SLOTS = [0, 5, 7, 10, 11, 13, 14, 15, 16, 17]
# ActivateOldShore (newgrf.cpp): sprite original base -> slot relocalizado.
ORIGINAL_SHORE_SLOTS = {
    4062: 4,
    4063: 1,
    4064: 2,
    4065: 8,
    4066: 6,
    4067: 12,
    4068: 3,
    4069: 9,
}
# `tileh_to_shoresprite` (water_cmd.cpp). Los índices 23, 27, 29 y 30
# representan las cuatro pendientes `SLOPE_STEEP_*`; no pueden degradarse a
# `tileh.min(14)` porque sus siluetas costeras son distintas.
TILEH_TO_SHORE_SPRITE = [
    0, 1, 2, 3, 4, 16, 6, 7, 8, 9, 17, 11, 12, 13, 14, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 10, 15, 0,
]

REAL_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
PSEUDO_RE = re.compile(r"^\s*(\d+)\s+\*\s+\d+\s+(.+?)\s*$")


def find_extra_nfo() -> Path:
    """Localiza (decodificando si hace falta) el NFO del GRF extra de OpenGFX."""
    for sprites_dir in (REPO / "assets" / "opengfx").glob("*/sprites"):
        for nfo in sprites_dir.glob("*extra*.nfo"):
            return nfo
        # Decodificar el GRF extra junto al base si todavía no se hizo.
        for grf in sprites_dir.parent.glob("*extra*.grf"):
            print(f"Decodificando {grf.name} con grfcodec...")
            subprocess.run(
                ["grfcodec", "-d", "-o", "png", grf.name, "sprites/"],
                cwd=grf.parent,
                check=True,
                stdout=subprocess.DEVNULL,
            )
            for nfo in sprites_dir.glob("*extra*.nfo"):
                return nfo
    sys.exit("No se encontró el GRF extra de OpenGFX (corré descargar_graficos.sh)")


def find_base_nfo(sprites_dir: Path) -> Path:
    """Encuentra el NFO del GRF base junto al GRF extra ya seleccionado."""
    for nfo in sorted(sprites_dir.glob("*base*.nfo")):
        return nfo
    sys.exit(f"No se encontró el NFO base de OpenGFX junto a {sprites_dir}")


def parse_real_sprites(nfo: Path) -> list[tuple[int, int, str, tuple[int, ...]]]:
    """Lee sprites reales como (línea, id, hoja, rect) desde un NFO."""
    reals: list[tuple[int, int, str, tuple[int, ...]]] = []
    for i, line in enumerate(nfo.read_text(errors="replace").splitlines()):
        m = REAL_RE.match(line)
        if m:
            rect = tuple(int(m.group(k)) for k in range(3, 9))
            reals.append((i, int(m.group(1)), Path(m.group(2)).name, rect))
    return reals


def parse_original_shores(base_nfo: Path) -> dict[int, tuple[Path, tuple[int, ...]]]:
    """Devuelve los ocho sprites originales relocalizados como OpenTTD 15.3."""
    by_sprite_id = {
        sprite_id: (base_nfo.parent / sheet, rect)
        for _line, sprite_id, sheet, rect in parse_real_sprites(base_nfo)
    }
    missing = sorted(set(ORIGINAL_SHORE_SLOTS) - set(by_sprite_id))
    if missing:
        sys.exit(f"Faltan sprites originales de orilla {missing} en {base_nfo.name}")
    return {
        slot: by_sprite_id[sprite_id]
        for sprite_id, slot in ORIGINAL_SHORE_SLOTS.items()
    }


def parse_shore_blocks(nfo: Path) -> dict[int, tuple[Path, tuple[int, ...]]]:
    """Devuelve slot -> (sheet, (x, y, w, h, xrel, yrel)) para clima templado."""
    sprites_dir = nfo.parent
    lines = nfo.read_text(errors="replace").splitlines()

    # Sprites reales en orden de archivo, con su índice de línea para poder
    # tomar "los N siguientes" a un pseudo-sprite Action5.
    reals = parse_real_sprites(nfo)

    def take_after(line_idx: int, n: int) -> list[tuple[str, tuple[int, ...]]]:
        out = [(sheet, rect) for li, _sid, sheet, rect in reals if li > line_idx][:n]
        if len(out) != n:
            sys.exit(f"Action5 0D en línea {line_idx + 1}: esperaba {n} sprites")
        return out

    slots: dict[int, tuple[Path, tuple[int, ...]]] = {}
    seen_counts: set[int] = set()
    for i, line in enumerate(lines):
        m = PSEUDO_RE.match(line)
        if not m:
            continue
        data = m.group(2).replace("\t", " ").split()
        if data[:2] != ["05", "0D"]:
            continue
        if data[2] == "FF":
            count = int(data[4] + data[3], 16)
        else:
            count = int(data[2], 16)
        # El primer bloque de cada tamaño es el de clima templado (Action7
        # posterior salta los demás climas).
        if count in seen_counts:
            continue
        seen_counts.add(count)
        if count == 10:
            for slot, (sheet, rect) in zip(MISSING_BLOCK_SLOTS, take_after(i, 10)):
                slots[slot] = (sprites_dir / sheet, rect)
        elif count == 16:
            for slot, (sheet, rect) in enumerate(take_after(i, 16)):
                slots[slot] = (sprites_dir / sheet, rect)
    # OpenGFX 8.0 deja los ocho sprites clásicos en el GRF base y sólo define
    # los diez restantes por Action5 en el GRF extra. Si existe el bloque fijo
    # de 16, ya habrá cubierto todos los slots y no hace falta el GRF base.
    if set(range(SHORE_SPRITE_COUNT)) - set(slots):
        for slot, sprite in parse_original_shores(find_base_nfo(sprites_dir)).items():
            slots.setdefault(slot, sprite)
    missing = sorted(set(range(SHORE_SPRITE_COUNT)) - set(slots))
    if missing:
        sys.exit(f"Faltan slots de orilla {missing} en {nfo.name}")
    return slots


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    img = Image.open(png_path)
    if img.mode == "P":
        if mode != "32bpp":
            return indexed_dos_to_rgba(img)
        pal = img.getpalette()
        transparent_rgb = tuple(pal[0:3]) if pal else None
        img_rgba = img.convert("RGBA")
        if transparent_rgb is not None:
            data = [
                (0, 0, 0, 0) if (r, g, b) == transparent_rgb else (r, g, b, a)
                for r, g, b, a in img_rgba.getdata()
            ]
            img_rgba.putdata(data)
        return img_rgba
    return img.convert("RGBA") if mode == "32bpp" else dematte_legacy_colorkey(img)


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    nfo = find_extra_nfo()
    slots = parse_shore_blocks(nfo)

    sheets: dict[Path, Image.Image] = {}
    meta: list[tuple[int, int, int, int]] = []
    for slot in range(SHORE_SPRITE_COUNT):
        sheet_path, (x, y, w, h, xr, yr) = slots[slot]
        if sheet_path not in sheets:
            sheets[sheet_path] = load_sheet(sheet_path, mode)
        crop = sheets[sheet_path].crop((x, y, x + w, y + h))
        crop.save(TILES_DIR / f"shore_full_{slot:02d}.png")
        meta.append((w, h, xr, yr))
    print(f"Recortados {SHORE_SPRITE_COUNT} sprites de orilla en {TILES_DIR}")

    # Formato estable (alineado como rustfmt) para no ensuciar git si los datos no cambian.
    meta_bodies = [f"({w}.0, {h}.0, {xr}.0, {yr}.0)," for w, h, xr, yr in meta]
    meta_width = max(len(b) for b in meta_bodies)
    lines = [
        "// Generado por scripts/gen_shore_full_set.py — NO EDITAR A MANO.",
        "//",
        "// Set completo de orillas (SPR_SHORE_BASE + 0..17) de OpenGFX. El GRF",
        "// extra aporta Action5 0x0D y el base los ocho sprites clásicos.",
        "// `SHORE_META` son los offsets NFO (w, h, xrel, yrel) y",
        "// `TILEH_TO_SHORE_SPRITE` es la tabla",
        "// `tileh_to_shoresprite` de `water_cmd.cpp` (tabla completa 0..31).",
        "",
        "/// Sprites del set de orillas (`SHORE_SPRITE_COUNT` en upstream).",
        f"pub const SHORE_SPRITE_COUNT: usize = {SHORE_SPRITE_COUNT};",
        "",
        "/// (w, h, xrel, yrel) NFO por slot de `SPR_SHORE_BASE`.",
        f"pub static SHORE_META: [(f32, f32, f32, f32); {SHORE_SPRITE_COUNT}] = [",
    ]
    for slot, body in enumerate(meta_bodies):
        lines.append(f"    {body:<{meta_width}} // slot {slot}")
    tileh = ", ".join(str(v) for v in TILEH_TO_SHORE_SPRITE)
    lines += [
        "];",
        "",
        "/// `tileh` (0..31) → slot de sprite de orilla (`tileh_to_shoresprite`).",
        f"pub static TILEH_TO_SHORE_SPRITE: [u8; 32] = [{tileh}];",
        "",
    ]
    text = "\n".join(lines)
    if OUT_RS.is_file() and OUT_RS.read_text(encoding="utf-8") == text:
        print(f"Sin cambios en {OUT_RS.relative_to(REPO)}")
    else:
        OUT_RS.write_text(text, encoding="utf-8")
        print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
