#!/usr/bin/env python3
"""Recorta iconos UI de toolbar (OpenGFX) a assets/opengfx/tiles/.

Elevar/bajar: sprites 694–695 en el NFO base.
Nivelar: sprite 4964 (`SPR_IMG_LEVEL_LAND` = `SPR_OPENTTD_BASE + 68`) en el GRF extra.
Ajustes / audio: sprites 751 (`SPR_IMG_SETTINGS`) y 713 (`SPR_IMG_MUSIC`).

Útil sin volver a correr descargar_graficos.sh entero.
Requiere assets/opengfx/opengfx2-32ez/sprites/ (base + extra ya extraídos).

Uso:
  python3 scripts/crop_ui_terraform_icons.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Instala Pillow: pip install Pillow", file=sys.stderr)
    raise SystemExit(1) from None

ROOT = Path(__file__).resolve().parents[1]
TILES_DIR = ROOT / "assets/opengfx/tiles"
SPRITES_DIR = ROOT / "assets/opengfx/opengfx2-32ez/sprites"

NFO_LINE = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez|ogfx2e_extra_32ez)\d+\.(?:32\.png|png|pcx)))\s+"
    r"(?:8bpp|32bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)

# (sprite_id, output_name, nfo_filename)
SPRITES = [
    (694, "ui_terraform_up.png", "ogfx21_base_32ez.nfo"),
    (695, "ui_terraform_down.png", "ogfx21_base_32ez.nfo"),
    (4964, "ui_terraform_level.png", "ogfx2e_extra_32ez.nfo"),
    (751, "ui_settings.png", "ogfx21_base_32ez.nfo"),
    (713, "ui_sound.png", "ogfx21_base_32ez.nfo"),
]


def dematte_cc_blue(img: Image.Image) -> Image.Image:
    data = []
    for r, g, b, a in img.getdata():
        if a > 0 and r == 0 and g == 0 and b == 255:
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    out = img.copy()
    out.putdata(data)
    return out


def load_sprite_rects(nfo_path: Path) -> dict[int, tuple[int, int, int, int, str]]:
    rects: dict[int, tuple[int, int, int, int, str]] = {}
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = NFO_LINE.match(line)
        if m:
            sid = int(m.group(1))
            sheet = Path(m.group(2)).name
            rects[sid] = (
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
                sheet,
            )
    return rects


def load_sheets(sprites_dir: Path) -> dict[str, Image.Image]:
    sheets: dict[str, Image.Image] = {}
    for prefix in ("ogfx21_base_32ez", "ogfx2e_extra_32ez", "ogfx1_base"):
        for p in sorted(sprites_dir.glob(f"{prefix}*.png")):
            if p.stat().st_size == 0:
                continue
            sheets[p.name] = Image.open(p).convert("RGBA")
    return sheets


def main() -> int:
    if not SPRITES_DIR.is_dir():
        print(
            "No se encontró sprites OpenGFX. Ejecutá: ./scripts/descargar_graficos.sh --32bpp",
            file=sys.stderr,
        )
        return 1

    sheets = load_sheets(SPRITES_DIR)
    TILES_DIR.mkdir(parents=True, exist_ok=True)

    for sid, out_name, nfo_name in SPRITES:
        nfo_path = SPRITES_DIR / nfo_name
        if not nfo_path.is_file():
            print(f"  (omitido {out_name}: no existe {nfo_path})")
            continue
        rects = load_sprite_rects(nfo_path)
        if sid not in rects:
            print(f"  (omitido {out_name}: sprite {sid} no en {nfo_name})")
            continue
        x, y, w, h, sheet = rects[sid]
        sheet_key = sheet
        if sheet_key not in sheets:
            alt = Path(sheet).with_suffix(".pcx").name
            sheet_key = alt if alt in sheets else sheet
        if sheet_key not in sheets:
            print(f"  (omitido {out_name}: sheet {sheet} no encontrado)")
            continue
        crop = dematte_cc_blue(sheets[sheet_key].crop((x, y, x + w, y + h)))
        out = TILES_DIR / out_name
        crop.save(out)
        print(f"  {out_name} ({w}×{h}) ← sprite {sid} [{nfo_name}]")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
