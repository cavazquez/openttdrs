#!/usr/bin/env python3
"""Recorta sprites de aeronaves OpenGFX sin descargar nuevamente el baseset.

Genera Dakota, Fokker y Tricario en ``assets/opengfx/tiles/`` y elimina el
azul índice 0 usado como transparencia por los sprites clásicos.

Uso: python3 scripts/extract_aircraft_vehicle_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"

AIRCRAFT_SPRITES: tuple[tuple[int, str], ...] = (
    (3765, "vehicle_aircraft_dakota_n.png"),
    (3766, "vehicle_aircraft_dakota_ne.png"),
    (3767, "vehicle_aircraft_dakota_e.png"),
    (3768, "vehicle_aircraft_dakota_se.png"),
    (3769, "vehicle_aircraft_dakota_s.png"),
    (3770, "vehicle_aircraft_dakota_sw.png"),
    (3771, "vehicle_aircraft_dakota_w.png"),
    (3772, "vehicle_aircraft_dakota_nw.png"),
    (3773, "vehicle_aircraft_fokker_n.png"),
    (3774, "vehicle_aircraft_fokker_ne.png"),
    (3775, "vehicle_aircraft_fokker_e.png"),
    (3776, "vehicle_aircraft_fokker_se.png"),
    (3777, "vehicle_aircraft_fokker_s.png"),
    (3778, "vehicle_aircraft_fokker_sw.png"),
    (3779, "vehicle_aircraft_fokker_w.png"),
    (3780, "vehicle_aircraft_fokker_nw.png"),
    (3813, "vehicle_aircraft_tricario_n.png"),
    (3814, "vehicle_aircraft_tricario_ne.png"),
    (3815, "vehicle_aircraft_tricario_e.png"),
    (3816, "vehicle_aircraft_tricario_se.png"),
    (3817, "vehicle_aircraft_tricario_s.png"),
    (3818, "vehicle_aircraft_tricario_sw.png"),
    (3819, "vehicle_aircraft_tricario_w.png"),
    (3820, "vehicle_aircraft_tricario_nw.png"),
)

NFO_ROW = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def resolve_paths(mode: str) -> tuple[Path, str]:
    opengfx = REPO / "assets" / "opengfx"
    if mode == "32bpp":
        return opengfx / "opengfx2-32ez" / "sprites", "ogfx21_base_32ez"
    version_dirs = sorted(opengfx.glob("opengfx-*"))
    if not version_dirs:
        raise SystemExit("No hay assets OpenGFX; ejecutá descargar_graficos.sh")
    return version_dirs[-1] / "sprites", "ogfx1_base"


def load_sheets(sprites_dir: Path, prefix: str) -> dict[str, Image.Image]:
    sheets: dict[str, Image.Image] = {}
    for pattern in (f"{prefix}*.png", f"{prefix}*.pcx"):
        for path in sorted(sprites_dir.glob(pattern)):
            if path.stat().st_size > 0:
                sheets[path.name] = Image.open(path).convert("RGBA")
    return sheets


def parse_nfo(nfo_path: Path) -> dict[int, tuple[int, int, int, int, str]]:
    rows: dict[int, tuple[int, int, int, int, str]] = {}
    for line in nfo_path.read_text(errors="replace").splitlines():
        match = NFO_ROW.match(line)
        if match:
            rows.setdefault(
                int(match.group(1)),
                (
                    int(match.group(4)),
                    int(match.group(5)),
                    int(match.group(6)),
                    int(match.group(7)),
                    Path(match.group(2)).name,
                ),
            )
    return rows


def dematte_index_zero(img: Image.Image) -> Image.Image:
    rgba = img.convert("RGBA")
    rgba.putdata(
        [
            (0, 0, 0, 0) if (r, g, b) == (0, 0, 255) else (r, g, b, a)
            for r, g, b, a in rgba.get_flattened_data()
        ]
    )
    return rgba


def main() -> int:
    mode = detect_graphics_mode(REPO) or "32bpp"
    sprites_dir, prefix = resolve_paths(mode)
    nfo_path = sprites_dir / f"{prefix}.nfo"
    sheets = load_sheets(sprites_dir, prefix)
    rows = parse_nfo(nfo_path)
    TILES.mkdir(parents=True, exist_ok=True)

    extracted = 0
    for sprite_id, filename in AIRCRAFT_SPRITES:
        row = rows.get(sprite_id)
        if row is None:
            print(f"  omitido {filename}: sprite {sprite_id} no en NFO", file=sys.stderr)
            continue
        x, y, width, height, sheet_name = row
        sheet = sheets.get(sheet_name)
        if sheet is None:
            print(f"  omitido {filename}: sheet {sheet_name} ausente", file=sys.stderr)
            continue
        crop = dematte_index_zero(sheet.crop((x, y, x + width, y + height)))
        crop.save(TILES / filename)
        print(f"  {filename} ({width}x{height}) <- sprite {sprite_id}")
        extracted += 1

    print(f"Modo {mode}: {extracted}/{len(AIRCRAFT_SPRITES)} PNG de aeronave")
    return 0 if extracted == len(AIRCRAFT_SPRITES) else 1


if __name__ == "__main__":
    raise SystemExit(main())
