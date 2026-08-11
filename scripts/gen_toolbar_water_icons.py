#!/usr/bin/env python3
"""Extrae iconos del toolbar acuático (OpenTTD dock_gui / waterways).

Sprites base (depósito, muelle, boya) + GUI extra (canal, río, acueducto)
+ Action5 canals índice 64 (esclusa).

Salida: assets/opengfx/tiles/toolbar_water_*.png

Uso: python3 scripts/gen_toolbar_water_icons.py
"""
from __future__ import annotations

import re
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"


def active_sprite_sources() -> tuple[Path, Path, Path]:
    """Selecciona el set gráfico activo, incluido el OpenGFX clásico 8bpp."""
    opengfx = REPO / "assets" / "opengfx"
    if detect_graphics_mode(REPO) == "32bpp":
        sprites = opengfx / "opengfx2-32ez" / "sprites"
        return sprites, sprites / "ogfx21_base_32ez.nfo", sprites / "ogfx2e_extra_32ez.nfo"
    candidates = sorted(opengfx.glob("opengfx-*/sprites"), reverse=True)
    if not candidates:
        raise SystemExit("no hay OpenGFX 8bpp decodificado; corré descargar_graficos.sh --8bpp")
    sprites = candidates[0]
    return sprites, sprites / "ogfx1_base.nfo", sprites / "ogfxe_extra.nfo"


SPRITES, BASE_NFO, EXTRA_NFO = active_sprite_sources()

# SPR_IMG_* en table/sprites.h
BASE_ICONS = [
    (748, "depot"),  # SPR_IMG_SHIP_DEPOT
    (746, "dock"),  # SPR_IMG_SHIP_DOCK
    (693, "buoy"),  # SPR_IMG_BUOY
]

# SPR_OPENTTD_BASE + n (Action5 tipo 0x15)
EXTRA_ICONS = [
    (88, "canal"),  # SPR_IMG_BUILD_CANAL
    (136, "river"),  # SPR_IMG_BUILD_RIVER
    (145, "aqueduct"),  # SPR_IMG_AQUEDUCT
]

# SPR_IMG_BUILD_LOCK = SPR_CANALS_BASE + 64
LOCK_A5_SLOT = 64

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.png)\s+(8bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
A5_GUI_RE = re.compile(r"^\s*(\d+)\s+\*\s+\d+\s+05 95 FF ([0-9A-F]{2}) 00 FF ([0-9A-F]{2}) 00")
A5_CANALS_RE = re.compile(
    r"^\s*(\d+)\s+\*\s+\d+\s+05 (?:08|88) FF ([0-9A-F]{2}) 00(?: FF ([0-9A-F]{2}) 00)?"
)


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in rows:
            rows[sid] = (
                Path(m.group(2)).name,
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
            )
    return rows


def gui_offset_map(nfo: Path) -> dict[int, int]:
    out: dict[int, int] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = A5_GUI_RE.match(line)
        if not m:
            continue
        header_num = int(m.group(1))
        count = int(m.group(2), 16)
        # OpenGFX clásico usa `05 08 FF <count> 00` (offset implícito 0),
        # mientras que algunas variantes 32bpp escriben el offset explícito.
        offset = int(m.group(3), 16) if m.group(3) is not None else 0
        for i in range(count):
            out.setdefault(offset + i, header_num + 1 + i)
    return out


def canals_slot_map(nfo: Path) -> dict[int, re.Match[str]]:
    """Índice Action5 canals (0..64) → fila NFO 8bpp."""
    lines = nfo.read_text(errors="replace").splitlines()
    slots: dict[int, re.Match[str]] = {}
    i = 0
    while i < len(lines):
        m = A5_CANALS_RE.match(lines[i])
        if not m:
            i += 1
            continue
        count = int(m.group(2), 16)
        # El OpenGFX 8bpp clásico omite el offset cuando el bloque comienza
        # en cero; las variantes modernas lo escriben explícitamente.
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


def to_toolbar_canvas(img: Image.Image) -> Image.Image:
    w, h = img.size
    scale = min(2.0, 63.0 / w, 51.0 / h)
    sw, sh = max(1, round(w * scale)), max(1, round(h * scale))
    icon = img.resize((sw, sh), Image.NEAREST)
    canvas = Image.new("RGBA", (63, 51), (0, 0, 0, 0))
    canvas.alpha_composite(icon, ((63 - sw) // 2, (51 - sh) // 2))
    return canvas


def crop(rows: dict[int, tuple[str, int, int, int, int]], sid: int, out_name: str) -> None:
    sheet_name, x, y, w, h = rows[sid]
    sheet = Image.open(SPRITES / sheet_name)
    img = dematte_blue(sheet.crop((x, y, x + w, y + h)))
    to_toolbar_canvas(img).save(TILES / out_name)
    print(f"  {out_name} <- {sheet_name} sprite {sid} ({w}x{h})")


def crop_match(m: re.Match[str], out_name: str) -> None:
    sheet_name = Path(m.group(2)).name
    x, y, w, h = map(int, m.group(4, 5, 6, 7))
    sheet = Image.open(SPRITES / sheet_name)
    img = dematte_blue(sheet.crop((x, y, x + w, y + h)))
    to_toolbar_canvas(img).save(TILES / out_name)
    print(f"  {out_name} <- {sheet_name} canals[{LOCK_A5_SLOT}] ({w}x{h})")


def main() -> None:
    if not BASE_NFO.is_file() or not EXTRA_NFO.is_file():
        raise SystemExit(
            f"faltan NFO base/extra en {SPRITES} — ejecutá descargar_graficos.sh en el perfil activo"
        )
    TILES.mkdir(parents=True, exist_ok=True)

    base_rows = parse_rows(BASE_NFO)
    for sid, name in BASE_ICONS:
        if sid not in base_rows:
            raise SystemExit(f"sprite base {sid} ({name}) no encontrado")
        crop(base_rows, sid, f"toolbar_water_{name}.png")

    extra_rows = parse_rows(EXTRA_NFO)
    offsets = gui_offset_map(EXTRA_NFO)
    for off, name in EXTRA_ICONS:
        sid = offsets.get(off)
        if sid is None or sid not in extra_rows:
            raise SystemExit(f"icono extra GUI offset {off} ({name}) no encontrado")
        crop(extra_rows, sid, f"toolbar_water_{name}.png")

    slots = canals_slot_map(EXTRA_NFO)
    if LOCK_A5_SLOT not in slots:
        raise SystemExit(f"canals Action5 slot {LOCK_A5_SLOT} (esclusa) no encontrado")
    crop_match(slots[LOCK_A5_SLOT], "toolbar_water_lock.png")


if __name__ == "__main__":
    main()
