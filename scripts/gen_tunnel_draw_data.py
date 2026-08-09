#!/usr/bin/env python3
"""Extrae y describe las dos capas de cada boca de túnel vanilla.

`DrawTile_TunnelBridge` no dibuja una única imagen: el sprite ``rear`` se
emite como suelo y el inmediatamente siguiente (``front``) como sprite
sortable. Extraer solamente el rear deja la vía visualmente cortada al llegar
al túnel, sobre todo en mono/maglev y túneles electrificados.

Este script recorta ambas capas del set gráfico activo y genera sus metadatos
NFO para el anclaje del frente.

Salida:

* ``assets/opengfx/tiles/tunnel_{tipo}_{rear,front}_{dir}.png``
* ``crates/openttdrs-client/src/sprites/tunnel_draw_data_generated.rs``

Uso: ``python3 scripts/gen_tunnel_draw_data.py``
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/tunnel_draw_data_generated.rs"

DIRS = ("ne", "se", "sw", "nw")

# El rear es el `DrawGroundSprite`; el front es `AddSortableSpriteToDraw` y
# siempre sigue al rear en el banco base de OpenGFX.
TUNNEL_BASES = (
    ("rail", (2365, 2367, 2369, 2371)),
    ("mono", (2373, 2375, 2377, 2379)),
    ("mglv", (2381, 2383, 2385, 2387)),
    ("road", (2389, 2391, 2393, 2395)),
)

NFO_ROW = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def active_sprites_dir(mode: str) -> Path:
    opengfx = REPO / "assets" / "opengfx"
    if mode == "32bpp":
        sprites = opengfx / "opengfx2-32ez" / "sprites"
        if sprites.is_dir():
            return sprites
    candidates = sorted(opengfx.glob("opengfx-*/sprites"), reverse=True)
    if candidates:
        return candidates[0]
    sys.exit("No se encontró el set OpenGFX decodificado; corré descargar_graficos.sh")


def load_sheet(path: Path, mode: str) -> Image.Image:
    """Replica la desmatización usada por los demás extractores del proyecto."""
    img = Image.open(path)
    if mode == "32bpp":
        if img.mode == "P":
            palette = img.getpalette()
            transparent_rgb = tuple(palette[:3]) if palette else None
            rgba = img.convert("RGBA")
            if transparent_rgb is not None:
                rgba.putdata(
                    [
                        (0, 0, 0, 0) if (r, g, b) == transparent_rgb else (r, g, b, a)
                        for r, g, b, a in rgba.getdata()
                    ]
                )
            return rgba
        return img.convert("RGBA")

    rgba = img.convert("RGBA")
    rgba.putdata(
        [
            (0, 0, 0, 0) if (r, g, b) == (0, 0, 255) else (r, g, b, a)
            for r, g, b, a in rgba.getdata()
        ]
    )
    return rgba


def parse_rects(nfo: Path) -> dict[int, tuple[Path, int, int, int, int, int, int]]:
    rects: dict[int, tuple[Path, int, int, int, int, int, int]] = {}
    for line in nfo.read_text(encoding="utf-8", errors="replace").splitlines():
        match = NFO_ROW.match(line)
        if match is None:
            continue
        sid = int(match.group(1))
        sheet = nfo.parent / Path(match.group(2)).name
        rects[sid] = (
            sheet,
            int(match.group(4)),
            int(match.group(5)),
            int(match.group(6)),
            int(match.group(7)),
            int(match.group(8)),
            int(match.group(9)),
        )
    return rects


def crop_tunnel_sprites(mode: str) -> dict[int, tuple[float, float, float, float]]:
    sprites_dir = active_sprites_dir(mode)
    nfo_name = "ogfx21_base_32ez.nfo" if mode == "32bpp" else "ogfx1_base.nfo"
    rects = parse_rects(sprites_dir / nfo_name)
    TILES_DIR.mkdir(parents=True, exist_ok=True)
    sheets: dict[Path, Image.Image] = {}
    meta: dict[int, tuple[float, float, float, float]] = {}

    for kind, rear_ids in TUNNEL_BASES:
        for direction, rear_id in enumerate(rear_ids):
            for layer, sid in (("rear", rear_id), ("front", rear_id + 1)):
                if sid not in rects:
                    sys.exit(f"sprite de túnel {sid} no encontrado en {nfo_name}")
                sheet_path, x, y, w, h, xrel, yrel = rects[sid]
                if not sheet_path.is_file():
                    alt = sheet_path.with_suffix(".pcx")
                    if alt.is_file():
                        sheet_path = alt
                    else:
                        sys.exit(f"falta sheet {sheet_path} para sprite {sid}")
                if sheet_path not in sheets:
                    sheets[sheet_path] = load_sheet(sheet_path, mode)
                crop = sheets[sheet_path].crop((x, y, x + w, y + h))
                out_name = f"tunnel_{kind}_{layer}_{DIRS[direction]}.png"
                crop.save(TILES_DIR / out_name)
                meta[sid] = (float(crop.width), float(crop.height), float(xrel), float(yrel))

    return meta


def sprite_rows() -> list[tuple[int, str]]:
    rows: list[tuple[int, str]] = []
    for kind, rear_ids in TUNNEL_BASES:
        for direction, rear_id in enumerate(rear_ids):
            rows.extend(
                (
                    (rear_id, f"tunnel_{kind}_rear_{DIRS[direction]}.png"),
                    (rear_id + 1, f"tunnel_{kind}_front_{DIRS[direction]}.png"),
                )
            )
    return rows


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    metadata = crop_tunnel_sprites(mode)
    rows = sprite_rows()
    lines = [
        "// Generado por scripts/gen_tunnel_draw_data.py — NO EDITAR A MANO.",
        "// Offsets NFO (sprite_id, w, h, xrel, yrel) de capas rear/front de túnel.",
        "",
        "/// Metadata NFO de las capas de cada portal de túnel.",
        f"pub static TUNNEL_SPRITE_META: [(u32, f32, f32, f32, f32); {len(rows)}] = [",
    ]
    for sid, png in rows:
        w, h, xrel, yrel = metadata[sid]
        lines.append(
            f"    ({sid}, {w:.1f}, {h:.1f}, {xrel:.1f}, {yrel:.1f}), // {png}"
        )
    lines += ["];", ""]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Recortadas {len(rows)} capas de túnel en {TILES_DIR.relative_to(REPO)}")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
