#!/usr/bin/env python3
"""Extrae waypoints ferroviarios al estilo OpenGFX2+ Stations.

OpenTTD con OpenGFX2 suele activar el NewGRF ``ogfx2_stations.grf``, que define
las casetas con ballast (sprites 19–20 eje X, 23–24 eje Y). El GRF *extra*
base (4974–4977) solo trae toldos planos de fallback.

Salida (alias compatibles con ``SPR_WAYPOINT_*`` + toldos CC ogfx2):
- ``rail_4974.png`` / ``rail_4975.png`` ← estaciones 19 / 20 (cuerpo X)
- ``rail_4978.png`` / ``rail_4979.png`` ← estaciones 21 / 22 (toldo CC X)
- ``rail_4976.png`` / ``rail_4977.png`` ← estaciones 23 / 24 (cuerpo Y)
- ``rail_4980.png`` / ``rail_4981.png`` ← estaciones 25 / 26 (toldo CC Y)

Uso: python3 scripts/gen_rail_waypoint_sprites.py
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

from PIL import Image

from pillow_compat import flattened_data

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
COMPANY_DATA = (
    REPO / "crates/openttdrs-client/src/sprites/company_palette_data_generated.rs"
)
STATIONS_GRF_URL = (
    "https://github.com/OpenTTD/OpenGFX2/releases/download/v0.6/ogfx2_stations.grf"
)

# (sprite_id salida OpenTTD, sprite_id ogfx2_stations, etiqueta)
WAYPOINT_EXPORTS: list[tuple[int, int, str]] = [
    (4974, 19, "X oeste"),
    (4975, 20, "X este"),
    (4978, 21, "X toldo oeste"),
    (4979, 22, "X toldo este"),
    (4976, 23, "Y oeste"),
    (4977, 24, "Y este"),
    (4980, 25, "Y toldo oeste"),
    (4981, 26, "Y toldo este"),
]

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)

COMPANY_RAMP_INDICES: list[list[int]] = [
    [0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD],
    [0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67],
    [0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31],
    [0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45],
    [0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xA4, 0xA5, 0xA6],
    [0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1],
    [0x52, 0x53, 0x54, 0x55, 0xCE, 0xCF, 0xD0, 0xD1],
    [0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F],
    [0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99],
    [0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79],
    [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87],
    [0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F],
    [0x40, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0x27],
    [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27],
    [0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B],
    [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
]

FUZZY_CC_DIST_SQ = 45 * 45 * 3


def load_company_ramp_rgb() -> list[tuple[str, int, tuple[int, int, int]]]:
    entries: list[tuple[str, int, tuple[int, int, int]]] = []
    for line in COMPANY_DATA.read_text(encoding="utf-8").splitlines():
        m = re.match(
            r"\s*\[(\d+), (\d+), (\d+)\],\s*//\s*(\w+)\[(\d+)\]", line
        )
        if m:
            entries.append(
                (
                    m.group(4),
                    int(m.group(5)),
                    (int(m.group(1)), int(m.group(2)), int(m.group(3))),
                )
            )
    return entries


def build_palette_index_remap(palette: list[int]) -> dict[int, int]:
    dark_indices = COMPANY_RAMP_INDICES[0]
    ramp_entries = load_company_ramp_rgb()
    index_to_shade: dict[int, int] = {}
    for ramp in COMPANY_RAMP_INDICES:
        for shade, pal_idx in enumerate(ramp):
            index_to_shade[pal_idx] = shade
    pal_rgb = [
        (palette[i * 3], palette[i * 3 + 1], palette[i * 3 + 2]) for i in range(256)
    ]
    remap: dict[int, int] = {0: 0}
    for idx in range(1, 256):
        if idx in index_to_shade:
            remap[idx] = dark_indices[index_to_shade[idx]]
            continue
        rgb = pal_rgb[idx]
        best_shade: int | None = None
        best_dist = FUZZY_CC_DIST_SQ + 1
        for _name, shade, crgb in ramp_entries:
            dist = sum((rgb[i] - crgb[i]) ** 2 for i in range(3))
            if dist < best_dist:
                best_dist = dist
                best_shade = shade
        if best_shade is not None and best_dist <= FUZZY_CC_DIST_SQ:
            remap[idx] = dark_indices[best_shade]
    return remap


def bake_company_palette_dark_blue(crop_p: Image.Image) -> Image.Image:
    if crop_p.mode != "P":
        raise SystemExit("waypoint: se esperaba hoja 8bpp indexada para horneado CC")
    palette = crop_p.getpalette()
    if palette is None:
        raise SystemExit("waypoint: paleta ausente en hoja 8bpp")
    remap = build_palette_index_remap(palette)
    baked = Image.new("P", crop_p.size)
    baked.putpalette(palette)
    baked.putdata([remap.get(px, px) for px in flattened_data(crop_p)])
    rgba = baked.convert("RGBA")
    transparent_rgb = tuple(palette[0:3])
    keyed = [
        (0, 0, 0, 0) if px[:3] == transparent_rgb else px
        for px in flattened_data(rgba)
    ]
    rgba.putdata(keyed)
    return rgba


def ensure_stations_grf() -> Path:
    for candidate in (
        REPO / "assets" / "opengfx" / "ogfx2_stations.grf",
        REPO / "assets" / "opengfx" / "opengfx2-32ez" / "ogfx2_stations.grf",
    ):
        if candidate.is_file():
            return candidate
    dest = REPO / "assets" / "opengfx" / "ogfx2_stations.grf"
    dest.parent.mkdir(parents=True, exist_ok=True)
    print(f"Descargando ogfx2_stations.grf…")
    urllib.request.urlretrieve(STATIONS_GRF_URL, dest)
    return dest


def decode_stations_grf(grf: Path) -> Path:
    out = REPO / "assets" / "opengfx" / ".ogfx2_stations_decode"
    nfo = out / "sprites" / "ogfx2_stations.nfo"
    if nfo.is_file():
        return out / "sprites"
    if shutil.which("grfcodec") is None:
        sys.exit("grfcodec no encontrado (necesario para decodificar ogfx2_stations.grf)")
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(grf, out / "ogfx2_stations.grf")
    subprocess.run(
        ["grfcodec", "-d", "-o", "png", "ogfx2_stations.grf", "sprites/"],
        cwd=out,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    return out / "sprites"


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    sprites_dir = nfo.parent
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m or m.group(3) != "8bpp":
            continue
        sid = int(m.group(1))
        if sid not in rows:
            rows[sid] = (
                str(sprites_dir / Path(m.group(2)).name),
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
            )
    return rows


def export_sprite(
    rows: dict[int, tuple[str, int, int, int, int]], src_sid: int, dst_sid: int, label: str
) -> None:
    if src_sid not in rows:
        raise SystemExit(f"sprite {src_sid} ({label}) no encontrado en ogfx2_stations.nfo")
    sheet_path, x, y, w, h = rows[src_sid]
    crop = Image.open(sheet_path).crop((x, y, x + w, y + h))
    img = bake_company_palette_dark_blue(crop)
    TILES.mkdir(parents=True, exist_ok=True)
    out = TILES / f"rail_{dst_sid}.png"
    img.save(out)
    print(f"  rail_{dst_sid}.png <- ogfx2_stations #{src_sid} ({w}x{h}, {label})")


def main() -> None:
    grf = ensure_stations_grf()
    sprites_dir = decode_stations_grf(grf)
    nfo = sprites_dir / "ogfx2_stations.nfo"
    if not nfo.is_file():
        sys.exit(f"no se generó {nfo}")
    rows = parse_rows(nfo)
    for dst_sid, src_sid, label in WAYPOINT_EXPORTS:
        export_sprite(rows, src_sid, dst_sid, label)
    print(f"Waypoints (ogfx2_stations) listos en {TILES}/")


if __name__ == "__main__":
    main()
