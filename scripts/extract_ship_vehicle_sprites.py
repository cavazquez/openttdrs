#!/usr/bin/env python3
"""Recorta sprites de barcos OpenGFX sin re-ejecutar descargar_graficos.sh.

OpenTTD `ship_cmd.cpp` `_ship_sprites[]` = {0x0E5D, 0x0E55, 0x0E65, 0x0E6D}
(+ Direction 0..7):

- MPS Channel Ferry → 3677..3684
- Oil Tanker → 3669..3676
- Coal Trader → 3685..3692
- Passenger Ferry → 3693..3700

Genera vehicle_ship_*.png en assets/opengfx/tiles/.
Luego: python3 scripts/gen_vehicle_gfx_data.py

Uso: python3 scripts/extract_ship_vehicle_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"

SHIP_SPRITES: tuple[tuple[int, str], ...] = (
    # MPS / Channel Ferry (spritenum 0)
    (3677, "vehicle_ship_mps_n.png"),
    (3678, "vehicle_ship_mps_ne.png"),
    (3679, "vehicle_ship_mps_e.png"),
    (3680, "vehicle_ship_mps_se.png"),
    (3681, "vehicle_ship_mps_s.png"),
    (3682, "vehicle_ship_mps_sw.png"),
    (3683, "vehicle_ship_mps_w.png"),
    (3684, "vehicle_ship_mps_nw.png"),
    # Oil tanker (spritenum 1)
    (3669, "vehicle_ship_oil_n.png"),
    (3670, "vehicle_ship_oil_ne.png"),
    (3671, "vehicle_ship_oil_e.png"),
    (3672, "vehicle_ship_oil_se.png"),
    (3673, "vehicle_ship_oil_s.png"),
    (3674, "vehicle_ship_oil_sw.png"),
    (3675, "vehicle_ship_oil_w.png"),
    (3676, "vehicle_ship_oil_nw.png"),
    # Coal trader (spritenum 2)
    (3685, "vehicle_ship_coal_n.png"),
    (3686, "vehicle_ship_coal_ne.png"),
    (3687, "vehicle_ship_coal_e.png"),
    (3688, "vehicle_ship_coal_se.png"),
    (3689, "vehicle_ship_coal_s.png"),
    (3690, "vehicle_ship_coal_sw.png"),
    (3691, "vehicle_ship_coal_w.png"),
    (3692, "vehicle_ship_coal_nw.png"),
    # Passenger ferry (spritenum 3)
    (3693, "vehicle_ship_ferry_n.png"),
    (3694, "vehicle_ship_ferry_ne.png"),
    (3695, "vehicle_ship_ferry_e.png"),
    (3696, "vehicle_ship_ferry_se.png"),
    (3697, "vehicle_ship_ferry_s.png"),
    (3698, "vehicle_ship_ferry_sw.png"),
    (3699, "vehicle_ship_ferry_w.png"),
    (3700, "vehicle_ship_ferry_nw.png"),
)

NFO_ROW = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def resolve_paths(mode: str) -> tuple[Path, Path, str]:
    opengfx = REPO / "assets" / "opengfx"
    if mode == "32bpp":
        base = opengfx / "opengfx2-32ez"
        return base / "sprites", TILES, "ogfx21_base_32ez"
    version_dirs = sorted(opengfx.glob("opengfx-*"))
    if not version_dirs:
        raise SystemExit("No hay assets OpenGFX en assets/opengfx/ (ejecutá descargar_graficos.sh)")
    base = version_dirs[-1]
    return base / "sprites", TILES, "ogfx1_base"


def load_sheets(sprites_dir: Path, prefix: str) -> dict[str, Image.Image]:
    sheets: dict[str, Image.Image] = {}
    for p in sorted(sprites_dir.glob(f"{prefix}*.png")):
        if p.stat().st_size == 0:
            continue
        sheets[p.name] = Image.open(p).convert("RGBA")
    for p in sorted(sprites_dir.glob(f"{prefix}*.pcx")):
        if p.stat().st_size == 0:
            continue
        sheets[p.name] = Image.open(p).convert("RGBA")
    return sheets


def parse_nfo(nfo_path: Path) -> dict[int, tuple[int, int, int, int, int, int, str]]:
    rows: dict[int, tuple[int, int, int, int, int, int, str]] = {}
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = NFO_ROW.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid in rows:
            continue
        rows[sid] = (
            int(m.group(4)),
            int(m.group(5)),
            int(m.group(6)),
            int(m.group(7)),
            int(m.group(8)),
            int(m.group(9)),
            Path(m.group(2)).name,
        )
    return rows


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


def crop_sprite(
    sheets: dict[str, Image.Image],
    rows: dict[int, tuple[int, int, int, int, int, int, str]],
    sid: int,
    out_name: str,
) -> bool:
    if sid not in rows:
        print(f"  omitido {out_name}: sprite {sid} no en NFO", file=sys.stderr)
        return False
    x, y, w, h, _xr, _yr, sheet = rows[sid]
    sheet_key = sheet
    if sheet_key not in sheets:
        alt = Path(sheet).with_suffix(".pcx").name
        sheet_key = alt if alt in sheets else sheet_key
    if sheet_key not in sheets:
        print(f"  omitido {out_name}: sheet {sheet} ausente", file=sys.stderr)
        return False
    crop = sheets[sheet_key].crop((x, y, x + w, y + h))
    crop = dematte_cc_blue(crop.convert("RGBA"))
    out = TILES / out_name
    TILES.mkdir(parents=True, exist_ok=True)
    crop.save(out)
    print(f"  {out_name} ({w}x{h}) <- sprite {sid} [{sheet_key}]")
    return True


def main() -> int:
    mode = detect_graphics_mode(REPO) or "32bpp"
    sprites_dir, tiles_dir, prefix = resolve_paths(mode)
    nfo_path = sprites_dir / f"{prefix}.nfo"
    if not nfo_path.is_file():
        raise SystemExit(f"NFO no encontrado: {nfo_path}")

    sheets = load_sheets(sprites_dir, prefix)
    if not sheets:
        raise SystemExit(f"No hay sheets {prefix}* en {sprites_dir}")

    rows = parse_nfo(nfo_path)
    print(f"Modo {mode}: extrayendo {len(SHIP_SPRITES)} sprites de barco a {tiles_dir}/")
    ok = 0
    for sid, name in SHIP_SPRITES:
        if crop_sprite(sheets, rows, sid, name):
            ok += 1
    print(f"Listo: {ok}/{len(SHIP_SPRITES)} PNG")
    if ok < len(SHIP_SPRITES):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
