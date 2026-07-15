#!/usr/bin/env python3
"""Extrae el set completo de orillas (SPR_SHORE_BASE, 18 sprites) del GRF extra.

OpenGFX no entrega la costa completa en el GRF base (ahí solo están los 8
sprites originales de TTD, 4062..4069). El set completo llega por **Action5
tipo 0x0D** en el GRF *extra* (`ogfx2e_extra_32ez.grf` / `ogfxe_extra.grf`):

- Un bloque de **10 sprites** ("missing shore sprites", `newgrf_act5.cpp`)
  que mapea en orden a SPR_SHORE_BASE + [0, 5, 7, 10, 11, 13, 14, 15, 16, 17]
  (STEEP_S, STEEP_W, WSE, STEEP_N, NWS, ENW, SEN, STEEP_E, EW, NS).
- Un bloque de **16 sprites** (A5BLOCK_FIXED) que reemplaza
  SPR_SHORE_BASE + 0..15 con el set redibujado.

Ambos bloques aparecen una vez por clima, condicionados con Action7 sobre la
variable de clima; el primero de cada tamaño es el de clima templado (valor 0).
Cargados en orden de archivo (10 primero, 16 después) el resultado templado es:
slots 0..15 del bloque de 16 y slots 16/17 (EW/NS) del bloque de 10.

Salidas:
- `assets/opengfx/tiles/shore_full_{slot:02d}.png` (slots 0..17)
- `crates/openttdrs-client/src/sprites/shore_draw_data_generated.rs` con
  `SHORE_META` (w/h/xrel/yrel del NFO) y `TILEH_TO_SHORE_SPRITE`
  (tabla `tileh_to_shoresprite` de `water_cmd.cpp`, pendientes 0..14).

Uso: python3 scripts/gen_shore_full_set.py
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/shore_draw_data_generated.rs"

SHORE_SPRITE_COUNT = 18
# Orden del bloque de 10 ("missing shore sprites", newgrf_act5.cpp).
MISSING_BLOCK_SLOTS = [0, 5, 7, 10, 11, 13, 14, 15, 16, 17]
# `tileh_to_shoresprite` (water_cmd.cpp), entradas 0..14 (sin empinadas).
TILEH_TO_SHORE_SPRITE = [0, 1, 2, 3, 4, 16, 6, 7, 8, 9, 17, 11, 12, 13, 14]

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


def parse_shore_blocks(nfo: Path) -> dict[int, tuple[Path, tuple[int, ...]]]:
    """Devuelve slot -> (sheet, (x, y, w, h, xrel, yrel)) para clima templado."""
    sprites_dir = nfo.parent
    lines = nfo.read_text(errors="replace").splitlines()

    # Sprites reales en orden de archivo, con su índice de línea para poder
    # tomar "los N siguientes" a un pseudo-sprite Action5.
    reals: list[tuple[int, str, tuple[int, ...]]] = []  # (line_idx, sheet, rect)
    for i, line in enumerate(lines):
        m = REAL_RE.match(line)
        if m:
            rect = tuple(int(m.group(k)) for k in range(3, 9))
            reals.append((i, Path(m.group(2)).name, rect))

    def take_after(line_idx: int, n: int) -> list[tuple[str, tuple[int, ...]]]:
        out = [(sheet, rect) for li, sheet, rect in reals if li > line_idx][:n]
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
    missing = sorted(set(range(SHORE_SPRITE_COUNT)) - set(slots))
    if missing:
        sys.exit(f"Faltan slots de orilla {missing} en {nfo.name}")
    return slots


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    img = Image.open(png_path)
    if img.mode == "P":
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
    img_rgba = img.convert("RGBA")
    if mode != "32bpp":
        data = [
            (0, 0, 0, 0) if (r, g, b) == (0, 0, 255) else (r, g, b, a)
            for r, g, b, a in img_rgba.getdata()
        ]
        img_rgba.putdata(data)
    return img_rgba


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
        "// Set completo de orillas (SPR_SHORE_BASE + 0..17) del GRF extra de",
        "// OpenGFX (Action5 tipo 0x0D, clima templado). `SHORE_META` son los",
        "// offsets NFO (w, h, xrel, yrel) y `TILEH_TO_SHORE_SPRITE` es la tabla",
        "// `tileh_to_shoresprite` de `water_cmd.cpp` (pendientes 0..14).",
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
        "/// `tileh` (0..14) → slot de sprite de orilla (`tileh_to_shoresprite`).",
        f"pub static TILEH_TO_SHORE_SPRITE: [u8; 15] = [{tileh}];",
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
