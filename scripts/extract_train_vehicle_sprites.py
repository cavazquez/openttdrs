#!/usr/bin/env python3
"""Recorta sprites de locomotoras OpenGFX sin re-ejecutar descargar_graficos.sh.

Genera vehicle_train_*.png (Kirby + grupos t0/t1/td/te) en assets/opengfx/tiles/.
Luego: python3 scripts/gen_vehicle_gfx_data.py

Uso: python3 scripts/extract_train_vehicle_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"

TRAIN_SPRITES: tuple[tuple[int, str], ...] = (
    (2921, "vehicle_train_n.png"),
    (2922, "vehicle_train_ne.png"),
    (2923, "vehicle_train_e.png"),
    (2924, "vehicle_train_se.png"),
    (2925, "vehicle_train_s.png"),
    (2926, "vehicle_train_sw.png"),
    (2927, "vehicle_train_w.png"),
    (2928, "vehicle_train_nw.png"),
    (2905, "vehicle_train_t0_n.png"),
    (2906, "vehicle_train_t0_ne.png"),
    (2907, "vehicle_train_t0_e.png"),
    (2908, "vehicle_train_t0_se.png"),
    (2909, "vehicle_train_t0_s.png"),
    (2910, "vehicle_train_t0_sw.png"),
    (2911, "vehicle_train_t0_w.png"),
    (2912, "vehicle_train_t0_nw.png"),
    (2913, "vehicle_train_t1_n.png"),
    (2914, "vehicle_train_t1_ne.png"),
    (2915, "vehicle_train_t1_e.png"),
    (2916, "vehicle_train_t1_se.png"),
    (2917, "vehicle_train_t1_s.png"),
    (2918, "vehicle_train_t1_sw.png"),
    (2919, "vehicle_train_t1_w.png"),
    (2920, "vehicle_train_t1_nw.png"),
    (2949, "vehicle_train_td_n.png"),
    (2950, "vehicle_train_td_ne.png"),
    (2951, "vehicle_train_td_e.png"),
    (2952, "vehicle_train_td_se.png"),
    (2953, "vehicle_train_td_s.png"),
    (2954, "vehicle_train_td_sw.png"),
    (2955, "vehicle_train_td_w.png"),
    (2956, "vehicle_train_td_nw.png"),
    (2965, "vehicle_train_te_n.png"),
    (2966, "vehicle_train_te_ne.png"),
    (2967, "vehicle_train_te_e.png"),
    (2968, "vehicle_train_te_se.png"),
    (2969, "vehicle_train_te_s.png"),
    (2970, "vehicle_train_te_sw.png"),
    (2971, "vehicle_train_te_w.png"),
    (2972, "vehicle_train_te_nw.png"),
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
    print(f"Modo {mode}: extrayendo {len(TRAIN_SPRITES)} sprites de tren a {tiles_dir}/")
    ok = 0
    for sid, name in TRAIN_SPRITES:
        if crop_sprite(sheets, rows, sid, name):
            ok += 1
    print(f"Listo: {ok}/{len(TRAIN_SPRITES)} PNG")
    if ok < len(TRAIN_SPRITES):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
