#!/usr/bin/env python3
"""Extrae tiles in-world de esclusas (Action5 canals / SPR_LOCK_*).

Compone agua plana + piezas rear/front de OpenGFX (SPR_CANALS_BASE+4..27)
en PNGs esperados por `WorldAssets::load`:

  water_lock_{ns,ew}_{lower,middle,upper}.png

Uso: python3 scripts/gen_water_lock_tiles.py
Luego: python3 scripts/gen_tile_atlas.py
"""
from __future__ import annotations

import re
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"


def active_sprite_sources() -> tuple[Path, Path]:
    """Selecciona el GRF extra del perfil activo (OpenGFX 8bpp o 32bpp)."""
    opengfx = REPO / "assets" / "opengfx"
    if detect_graphics_mode(REPO) == "32bpp":
        sprites = opengfx / "opengfx2-32ez" / "sprites"
        return sprites, sprites / "ogfx2e_extra_32ez.nfo"
    candidates = sorted(opengfx.glob("opengfx-*/sprites"), reverse=True)
    if not candidates:
        raise SystemExit("no hay OpenGFX 8bpp decodificado; corré descargar_graficos.sh --8bpp")
    sprites = candidates[0]
    return sprites, sprites / "ogfxe_extra.nfo"


SPRITES, EXTRA_NFO = active_sprite_sources()

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.png)\s+(8bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
A5_CANALS_RE = re.compile(
    r"^\s*(\d+)\s+\*\s+\d+\s+05 (?:08|88) FF ([0-9A-F]{2}) 00(?: FF ([0-9A-F]{2}) 00)?"
)

# (out_name, rear_slot, front_slot) — offsets desde SPR_CANALS_BASE
# ns ≈ eje Y (Y_UP), ew ≈ eje X (X_UP); lower=bottom, middle=center, upper=top.
LOCK_TILES = [
    ("water_lock_ns_lower.png", 12, 16),
    ("water_lock_ns_middle.png", 4, 8),
    ("water_lock_ns_upper.png", 20, 24),
    ("water_lock_ew_lower.png", 14, 18),
    ("water_lock_ew_middle.png", 6, 10),
    ("water_lock_ew_upper.png", 22, 26),
]

CANVAS_W = 64
CANVAS_H = 48


def canals_slot_map(nfo: Path) -> dict[int, re.Match[str]]:
    lines = nfo.read_text(errors="replace").splitlines()
    slots: dict[int, re.Match[str]] = {}
    i = 0
    while i < len(lines):
        m = A5_CANALS_RE.match(lines[i])
        if not m:
            i += 1
            continue
        count = int(m.group(2), 16)
        # El GRF clásico omite el offset cuando el bloque empieza en 0.
        offset = int(m.group(3), 16) if m.group(3) is not None else 0
        j = i + 1
        got = 0
        while j < len(lines) and got < count:
            rm = ROW_RE.match(lines[j])
            if rm:
                slots[offset + got] = rm
                got += 1
            elif A5_CANALS_RE.match(lines[j]) or re.search(r"\*\s+\d+\s+05 ", lines[j]):
                break
            j += 1
        i = j
    return slots


def dematte_blue(img: Image.Image) -> Image.Image:
    img = img.convert("RGBA")
    px = img.load()
    for j in range(img.height):
        for i in range(img.width):
            r, g, b, _a = px[i, j]
            if (r, g, b) in ((0, 0, 255), (255, 0, 255)):
                px[i, j] = (0, 0, 0, 0)
    return img


def crop_slot(slots: dict[int, re.Match[str]], slot: int) -> tuple[Image.Image, int, int]:
    m = slots[slot]
    sheet_name = Path(m.group(2)).name
    x, y, w, h = map(int, m.group(4, 5, 6, 7))
    xofs, yofs = int(m.group(8)), int(m.group(9))
    sheet = Image.open(SPRITES / sheet_name)
    return dematte_blue(sheet.crop((x, y, x + w, y + h))), xofs, yofs


def compose_lock(
    water: Image.Image,
    slots: dict[int, re.Match[str]],
    rear: int,
    front: int,
) -> Image.Image:
    canvas = Image.new("RGBA", (CANVAS_W, CANVAS_H), (0, 0, 0, 0))
    water_y = CANVAS_H - water.height
    canvas.alpha_composite(water, (0, water_y))
    # Origen típico de child sprites sobre ground isometrico 64×31.
    origin_x = 31
    origin_y = water_y
    for slot in (rear, front):
        img, xofs, yofs = crop_slot(slots, slot)
        canvas.alpha_composite(img, (origin_x + xofs, origin_y + yofs))
    return canvas


def main() -> None:
    if not EXTRA_NFO.is_file():
        raise SystemExit(
            f"falta {EXTRA_NFO} — ejecutá descargar_graficos.sh en el perfil activo"
        )
    water_path = TILES / "water_flat.png"
    if not water_path.is_file():
        raise SystemExit(f"falta {water_path}")
    water = Image.open(water_path).convert("RGBA")

    slots = canals_slot_map(EXTRA_NFO)
    needed = {s for _, a, b in LOCK_TILES for s in (a, b)}
    missing = sorted(s for s in needed if s not in slots)
    if missing:
        raise SystemExit(f"slots canals ausentes: {missing}")

    TILES.mkdir(parents=True, exist_ok=True)
    for name, rear, front in LOCK_TILES:
        out = compose_lock(water, slots, rear, front)
        out.save(TILES / name)
        print(f"  {name} <- canals[{rear}]+[{front}] ({out.width}x{out.height})")


if __name__ == "__main__":
    main()
