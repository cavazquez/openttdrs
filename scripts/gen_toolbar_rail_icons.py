#!/usr/bin/env python3
"""Extrae los iconos del toolbar de construcción ferroviaria de OpenGFX.

Iconos del set base (`_nested_build_rail_widgets`, `rail_gui.cpp`): dinamita
(703), quitar (714), vías NS/X/EW/Y (1251–1254), señales (1291), depósito
(1294), estación (1298), túnel (2430) y puente (2594) — sprite IDs de
`table/sprites.h`, recortados del NFO base.

Los tres restantes son del GRF extra (`SPR_OPENTTD_BASE + n`): autorail (+53),
convertir vía (+55) y waypoint (+76). En `ogfx2e_extra` van en bloques
Action 5 tipo `95` (con offset): autorail/convertir en `05 95 FF 0B 00 FF 2C 00`
(offsets 0x2C..0x36, sprites consecutivos tras la cabecera) y waypoint en el
bloque con offset 0x4A.

Salida: assets/opengfx/tiles/toolbar_rail_<nombre>.png

Uso: python3 scripts/gen_toolbar_rail_icons.py
"""
from __future__ import annotations

import re
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "assets" / "opengfx" / "opengfx2-32ez" / "sprites"
TILES = REPO / "assets" / "opengfx" / "tiles"

BASE_NFO = SPRITES / "ogfx21_base_32ez.nfo"
EXTRA_NFO = SPRITES / "ogfx2e_extra_32ez.nfo"

# (sprite_id base, nombre de salida)
BASE_ICONS = [
    (703, "demolish"),
    (714, "remove"),
    (1251, "rail_ns"),
    (1252, "rail_x"),
    (1253, "rail_ew"),
    (1254, "rail_y"),
    (1291, "signals"),
    (1294, "depot"),
    (1298, "station"),
    (2430, "tunnel"),
    (2594, "bridge"),
]

# SPR_OPENTTD_BASE + n para los iconos del GRF extra (Action 5 tipo 0x15).
EXTRA_ICONS = [
    (53, "autorail"),
    (55, "convert"),
    (76, "waypoint"),
]

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.png)\s+(8bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
A5_GUI_RE = re.compile(r"^\s*(\d+)\s+\*\s+\d+\s+05 95 FF ([0-9A-F]{2}) 00 FF ([0-9A-F]{2}) 00")


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in rows:  # primera fila 8bpp por sprite
            rows[sid] = (
                Path(m.group(2)).name,
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
            )
    return rows


def gui_offset_map(nfo: Path) -> dict[int, int]:
    """offset GUI (n de SPR_OPENTTD_BASE+n) → spritenum NFO."""
    out: dict[int, int] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = A5_GUI_RE.match(line)
        if not m:
            continue
        header_num = int(m.group(1))
        count = int(m.group(2), 16)
        offset = int(m.group(3), 16)
        for i in range(count):
            # Los sprites del bloque siguen consecutivos a la cabecera.
            out.setdefault(offset + i, header_num + 1 + i)
    return out


def crop(rows: dict[int, tuple[str, int, int, int, int]], sid: int, out_name: str) -> None:
    sheet_name, x, y, w, h = rows[sid]
    sheet = Image.open(SPRITES / sheet_name)
    img = sheet.crop((x, y, x + w, y + h)).convert("RGBA")
    # Azul puro = índice transparente del 8bpp.
    px = img.load()
    for j in range(img.height):
        for i in range(img.width):
            if px[i, j][:3] == (0, 0, 255):
                px[i, j] = (0, 0, 0, 0)
    # Lienzo uniforme 63×51 (proporción del icono del botón, 42×34) con el
    # sprite a ×2 (o menos si no entra), centrado y sin deformar.
    scale = min(2.0, 63.0 / w, 51.0 / h)
    sw, sh = max(1, round(w * scale)), max(1, round(h * scale))
    icon = img.resize((sw, sh), Image.NEAREST)
    canvas = Image.new("RGBA", (63, 51), (0, 0, 0, 0))
    canvas.alpha_composite(icon, ((63 - sw) // 2, (51 - sh) // 2))
    canvas.save(TILES / out_name)
    print(f"  {out_name} <- {sheet_name} sprite {sid} ({w}x{h})")


def main() -> None:
    base_rows = parse_rows(BASE_NFO)
    for sid, name in BASE_ICONS:
        crop(base_rows, sid, f"toolbar_rail_{name}.png")

    extra_rows = parse_rows(EXTRA_NFO)
    offsets = gui_offset_map(EXTRA_NFO)
    for off, name in EXTRA_ICONS:
        sid = offsets.get(off)
        if sid is None or sid not in extra_rows:
            raise SystemExit(f"icono extra GUI offset {off:#x} ({name}) no encontrado")
        crop(extra_rows, sid, f"toolbar_rail_{name}.png")


if __name__ == "__main__":
    main()
