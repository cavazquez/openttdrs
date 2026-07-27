#!/usr/bin/env python3
"""Recorta el catálogo SPR_IMG_* de toolbar a assets/opengfx/tiles/.

Elevar/bajar: sprites 694–695 en el NFO base.
Nivelar: sprite 4964 (`SPR_IMG_LEVEL_LAND` = `SPR_OPENTTD_BASE + 68`) en el GRF extra.
Ajustes / audio: sprites 751 (`SPR_IMG_SETTINGS`) y 713 (`SPR_IMG_MUSIC`).

Útil sin volver a correr descargar_graficos.sh entero. Detecta automáticamente
la fuente 8bpp o 32bpp ya extraída por el pipeline.

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
SPRITES_DIR_32 = ROOT / "assets/opengfx/opengfx2-32ez/sprites"
SPRITES_DIR_8 = next(
    iter(sorted((ROOT / "assets/opengfx").glob("opengfx-*/sprites"))), None
)
SPRITES_DIR = SPRITES_DIR_32 if SPRITES_DIR_32.is_dir() else SPRITES_DIR_8

NFO_LINE = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez|ogfx2e_extra_32ez)\d+\.(?:32\.png|png|pcx)))\s+"
    r"(?:8bpp|32bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)

# (sprite_id, output_name, nfo_filename)
SPRITES = [
    (143, "window_close.png", "ogfx21_base_32ez.nfo"),
    (694, "ui_terraform_up.png", "ogfx21_base_32ez.nfo"),
    (695, "ui_terraform_down.png", "ogfx21_base_32ez.nfo"),
    (4964, "ui_terraform_level.png", "ogfx2e_extra_32ez.nfo"),
    (751, "ui_settings.png", "ogfx21_base_32ez.nfo"),
    (713, "ui_sound.png", "ogfx21_base_32ez.nfo"),
    (726, "toolbar_pause.png", "ogfx21_base_32ez.nfo"),
    (724, "toolbar_save.png", "ogfx21_base_32ez.nfo"),
    (708, "toolbar_smallmap.png", "ogfx21_base_32ez.nfo"),
    (4077, "toolbar_town.png", "ogfx21_base_32ez.nfo"),
    (679, "toolbar_subsidies.png", "ogfx21_base_32ez.nfo"),
    (1299, "toolbar_stations.png", "ogfx21_base_32ez.nfo"),
    (737, "toolbar_finances.png", "ogfx21_base_32ez.nfo"),
    (743, "toolbar_companies.png", "ogfx21_base_32ez.nfo"),
    (745, "toolbar_graphs.png", "ogfx21_base_32ez.nfo"),
    (684, "toolbar_league.png", "ogfx21_base_32ez.nfo"),
    (741, "toolbar_industry.png", "ogfx21_base_32ez.nfo"),
    (742, "toolbar_trees.png", "ogfx21_base_32ez.nfo"),
    (731, "toolbar_trains.png", "ogfx21_base_32ez.nfo"),
    (732, "toolbar_road_vehicles.png", "ogfx21_base_32ez.nfo"),
    (733, "toolbar_ships.png", "ogfx21_base_32ez.nfo"),
    (734, "toolbar_aircraft.png", "ogfx21_base_32ez.nfo"),
    (735, "toolbar_zoom_in.png", "ogfx21_base_32ez.nfo"),
    (736, "toolbar_zoom_out.png", "ogfx21_base_32ez.nfo"),
    (727, "toolbar_build_rail.png", "ogfx21_base_32ez.nfo"),
    (728, "toolbar_build_road.png", "ogfx21_base_32ez.nfo"),
    (729, "toolbar_build_water.png", "ogfx21_base_32ez.nfo"),
    (730, "toolbar_build_air.png", "ogfx21_base_32ez.nfo"),
    (4083, "toolbar_landscape.png", "ogfx21_base_32ez.nfo"),
    (680, "toolbar_messages.png", "ogfx21_base_32ez.nfo"),
    (723, "toolbar_help.png", "ogfx21_base_32ez.nfo"),
    (4082, "toolbar_sign.png", "ogfx21_base_32ez.nfo"),
    (4986, "toolbar_fast_forward.png", "ogfx2e_extra_32ez.nfo"),
    (5075, "toolbar_build_tram.png", "ogfx2e_extra_32ez.nfo"),
    (5040, "toolbar_switch.png", "ogfx2e_extra_32ez.nfo"),
]

# Action5 `OTTD_GUI` no conserva el SpriteID runtime como número de sprite del
# NFO decodificado. Estos controles se recortan de su hoja fuente oficial.
WINDOW_SPRITES = [
    ("window_resize.png", 0, 1),
    ("scroll_up.png", 1, 0),
    ("scroll_down.png", 2, 0),
    ("scroll_left.png", 5, 0),
    ("scroll_right.png", 6, 0),
    ("window_pin_up.png", 3, 1),
    ("window_pin_down.png", 4, 1),
    ("window_shade.png", 4, 2),
    ("window_unshade.png", 5, 2),
]
WINDOW_SPRITE_SHEET = ROOT / "assets/opengfx/.ui-source/icons_8px_32bpp.png"


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


def crop_window_sprites() -> None:
    if not WINDOW_SPRITE_SHEET.is_file():
        raise SystemExit(f"Falta la hoja Action5 de UI: {WINDOW_SPRITE_SHEET}")
    sheet = Image.open(WINDOW_SPRITE_SHEET).convert("RGBA")
    for output_name, column, row in WINDOW_SPRITES:
        x = 1 + column * 9
        y = 1 + row * 9
        icon = dematte_cc_blue(sheet.crop((x, y, x + 8, y + 8)))
        icon.save(TILES_DIR / output_name)
        print(f"  {output_name} (8×8) ← Action5 OTTD_GUI [{column},{row}]")


def load_sheets(sprites_dir: Path) -> dict[str, Image.Image]:
    sheets: dict[str, Image.Image] = {}
    for prefix in (
        "ogfx21_base_32ez",
        "ogfx2e_extra_32ez",
        "ogfx1_base",
        "ogfxe_extra",
    ):
        for p in sorted(sprites_dir.glob(f"{prefix}*.png")):
            if p.stat().st_size == 0:
                continue
            sheets[p.name] = Image.open(p).convert("RGBA")
    return sheets


def main() -> int:
    if SPRITES_DIR is None or not SPRITES_DIR.is_dir():
        print(
            "No se encontró sprites OpenGFX. Ejecutá: ./scripts/descargar_graficos.sh --32bpp",
            file=sys.stderr,
        )
        return 1

    sheets = load_sheets(SPRITES_DIR)
    TILES_DIR.mkdir(parents=True, exist_ok=True)
    crop_window_sprites()

    for sid, out_name, nfo_name in SPRITES:
        nfo_path = SPRITES_DIR / nfo_name
        if not nfo_path.is_file():
            fallback = "ogfxe_extra.nfo" if "extra" in nfo_name else "ogfx1_base.nfo"
            nfo_path = SPRITES_DIR / fallback
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
