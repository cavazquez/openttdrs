#!/usr/bin/env python3
"""Recorta el catálogo SPR_IMG_* de toolbar a assets/opengfx/tiles/.

Elevar/bajar: sprites 694–695 en el NFO base.
Nivelar: sprite 4964 (`SPR_IMG_LEVEL_LAND` = `SPR_OPENTTD_BASE + 68`) en el GRF extra.
Ajustes / audio: sprites 751 (`SPR_IMG_SETTINGS`) y 713 (`SPR_IMG_MUSIC`).

Útil sin volver a correr descargar_graficos.sh entero. Usa estrictamente el
perfil 8bpp o 32bpp marcado por el pipeline, incluso para los Action5 de GUI.

Uso:
  python3 scripts/crop_ui_terraform_icons.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from nfo_sprite_meta import detect_graphics_mode
from pillow_compat import flattened_data

try:
    from PIL import Image
except ImportError:
    print("Instala Pillow: pip install Pillow", file=sys.stderr)
    raise SystemExit(1) from None

ROOT = Path(__file__).resolve().parents[1]
TILES_DIR = ROOT / "assets/opengfx/tiles"
SPR_OPENTTD_BASE = 4896


def active_graphics_mode() -> str:
    mode = detect_graphics_mode(ROOT)
    if mode in ("8bpp", "32bpp"):
        return mode
    raise SystemExit(
        "No se detectó .graphics_mode; ejecutá ./scripts/descargar_graficos.sh --8bpp o --32bpp"
    )


def active_sprites_dir(mode: str) -> Path:
    if mode == "32bpp":
        candidate = ROOT / "assets/opengfx/opengfx2-32ez/sprites"
        if candidate.is_dir():
            return candidate
    else:
        candidates = sorted((ROOT / "assets/opengfx").glob("opengfx-*/sprites"))
        if candidates:
            return candidates[0]
    raise SystemExit(
        f"No se encontró sprites OpenGFX {mode}. Ejecutá ./scripts/descargar_graficos.sh --{mode}"
    )


GRAPHICS_MODE = active_graphics_mode()
SPRITES_DIR = active_sprites_dir(GRAPHICS_MODE)

NFO_LINE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+"
    r"(?:8bpp|32bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
ACTION5_GUI_LINE = re.compile(
    r"^\s*\d+\s+\*\s+\d+\s+05\s+95\s+FF\s+([0-9A-F]{2})\s+"
    r"([0-9A-F]{2})\s+FF\s+([0-9A-F]{2})\s+([0-9A-F]{2})\s*$",
    re.IGNORECASE,
)

NFO_NAMES = {
    "8bpp": {"base": "ogfx1_base.nfo", "extra": "ogfxe_extra.nfo"},
    "32bpp": {"base": "ogfx21_base_32ez.nfo", "extra": "ogfx2e_extra_32ez.nfo"},
}

# (sprite_id, output_name, NFO de perfil)
SPRITES = [
    (143, "window_close.png", "base"),
    (694, "ui_terraform_up.png", "base"),
    (695, "ui_terraform_down.png", "base"),
    (751, "ui_settings.png", "base"),
    (713, "ui_sound.png", "base"),
    (726, "toolbar_pause.png", "base"),
    (724, "toolbar_save.png", "base"),
    (708, "toolbar_smallmap.png", "base"),
    (4077, "toolbar_town.png", "base"),
    (679, "toolbar_subsidies.png", "base"),
    (1299, "toolbar_stations.png", "base"),
    (737, "toolbar_finances.png", "base"),
    (743, "toolbar_companies.png", "base"),
    (745, "toolbar_graphs.png", "base"),
    (684, "toolbar_league.png", "base"),
    (741, "toolbar_industry.png", "base"),
    (742, "toolbar_trees.png", "base"),
    (731, "toolbar_trains.png", "base"),
    (732, "toolbar_road_vehicles.png", "base"),
    (733, "toolbar_ships.png", "base"),
    (734, "toolbar_aircraft.png", "base"),
    (735, "toolbar_zoom_in.png", "base"),
    (736, "toolbar_zoom_out.png", "base"),
    (727, "toolbar_build_rail.png", "base"),
    (728, "toolbar_build_road.png", "base"),
    (729, "toolbar_build_water.png", "base"),
    (730, "toolbar_build_air.png", "base"),
    (4083, "toolbar_landscape.png", "base"),
    (680, "toolbar_messages.png", "base"),
    (723, "toolbar_help.png", "base"),
    (4082, "toolbar_sign.png", "base"),
]

# Action5 0x95 (`OTTD_GUI`) define estos `SPR_OPENTTD_BASE + offset`. Los
# NFO decodificados usan números físicos distintos, así que hay que resolverlos
# desde el propio Action5 y nunca desde una hoja OpenGFX2 ajena al perfil activo.
ACTION5_SPRITES = [
    (SPR_OPENTTD_BASE + 44, "window_resize.png"),
    (SPR_OPENTTD_BASE + 45, "scroll_down.png"),
    (SPR_OPENTTD_BASE + 46, "scroll_up.png"),
    (SPR_OPENTTD_BASE + 47, "scroll_left.png"),
    (SPR_OPENTTD_BASE + 48, "scroll_right.png"),
    (SPR_OPENTTD_BASE + 51, "window_pin_up.png"),
    (SPR_OPENTTD_BASE + 52, "window_pin_down.png"),
    (SPR_OPENTTD_BASE + 151, "window_shade.png"),
    (SPR_OPENTTD_BASE + 152, "window_unshade.png"),
    (SPR_OPENTTD_BASE + 91, "ui_terraform_level.png"),
    (SPR_OPENTTD_BASE + 90, "toolbar_fast_forward.png"),
    (SPR_OPENTTD_BASE + 144, "toolbar_switch.png"),
    (SPR_OPENTTD_BASE + 179, "toolbar_build_tram.png"),
]


def dematte_colorkey(img: Image.Image) -> Image.Image:
    data = []
    for r, g, b, a in flattened_data(img):
        # OpenGFX clásico usa magenta de transparencia; OpenGFX2 suele usar
        # azul CC. Ambos son colorkeys de la hoja, no píxeles del icono.
        if a > 0 and ((r, g, b) == (0, 0, 255) or (r > 220 and g < 32 and b > 220)):
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
                int(m.group(3)),
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                sheet,
            )
    return rects


def load_action5_gui_rects(
    nfo_path: Path, physical_rects: dict[int, tuple[int, int, int, int, str]]
) -> dict[int, tuple[int, int, int, int, str]]:
    """Resuelve los sprites virtuales `SPR_OPENTTD_BASE + offset` del Action5."""
    lines = nfo_path.read_text(errors="replace").splitlines()
    virtual: dict[int, tuple[int, int, int, int, str]] = {}
    for index, line in enumerate(lines):
        action = ACTION5_GUI_LINE.match(line)
        if action is None:
            continue
        count = int(action.group(1), 16) | (int(action.group(2), 16) << 8)
        offset = int(action.group(3), 16) | (int(action.group(4), 16) << 8)
        seen = 0
        for following in lines[index + 1 :]:
            row = NFO_LINE.match(following)
            if row is None:
                continue
            physical_id = int(row.group(1))
            rect = physical_rects.get(physical_id)
            if rect is None:
                continue
            virtual[SPR_OPENTTD_BASE + offset + seen] = rect
            seen += 1
            if seen == count:
                break
        if seen != count:
            raise SystemExit(
                f"Action5 OTTD_GUI incompleto en {nfo_path}: offset {offset}, {seen}/{count} sprites"
            )
    return virtual


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


def save_crop(
    rect: tuple[int, int, int, int, str],
    sheets: dict[str, Image.Image],
    output_name: str,
    source: str,
) -> bool:
    x, y, w, h, sheet = rect
    sheet_key = sheet
    if sheet_key not in sheets:
        alt = Path(sheet).with_suffix(".pcx").name
        sheet_key = alt if alt in sheets else sheet
    if sheet_key not in sheets:
        print(f"  (omitido {output_name}: sheet {sheet} no encontrado)")
        return False
    crop = dematte_colorkey(sheets[sheet_key].crop((x, y, x + w, y + h)))
    out = TILES_DIR / output_name
    crop.save(out)
    print(f"  {output_name} ({w}×{h}) ← {source}")
    return True


def main() -> int:
    sheets = load_sheets(SPRITES_DIR)
    TILES_DIR.mkdir(parents=True, exist_ok=True)

    nfo_paths = {
        kind: SPRITES_DIR / filename
        for kind, filename in NFO_NAMES[GRAPHICS_MODE].items()
    }
    missing_nfos = [path for path in nfo_paths.values() if not path.is_file()]
    if missing_nfos:
        raise SystemExit(f"Falta NFO del perfil {GRAPHICS_MODE}: {missing_nfos[0]}")
    rects = {kind: load_sprite_rects(path) for kind, path in nfo_paths.items()}
    action5_rects = load_action5_gui_rects(nfo_paths["extra"], rects["extra"])

    missing: list[str] = []
    for sid, out_name, nfo_kind in SPRITES:
        rect = rects[nfo_kind].get(sid)
        if rect is None:
            print(f"  (omitido {out_name}: sprite {sid} no en {nfo_paths[nfo_kind].name})")
            missing.append(out_name)
            continue
        if not save_crop(rect, sheets, out_name, f"sprite {sid} [{nfo_paths[nfo_kind].name}]"):
            missing.append(out_name)

    for sid, out_name in ACTION5_SPRITES:
        rect = action5_rects.get(sid)
        if rect is None:
            print(f"  (omitido {out_name}: Action5 OTTD_GUI no define sprite {sid})")
            missing.append(out_name)
            continue
        if not save_crop(rect, sheets, out_name, f"Action5 OTTD_GUI sprite {sid}"):
            missing.append(out_name)

    if missing:
        raise SystemExit(
            f"Faltan {len(missing)} iconos UI del perfil {GRAPHICS_MODE}: {', '.join(missing)}"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
