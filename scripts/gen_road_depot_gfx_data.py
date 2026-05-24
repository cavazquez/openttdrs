#!/usr/bin/env python3
"""Genera road_depot_gfx_data_generated.rs desde OpenTTD road_land.h + PNG/NFO."""
from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    Image = None  # type: ignore[misc, assignment]

DIRS = ("ne", "se", "sw", "nw")
NfoEntry = tuple[str, int, int, int, int]  # bpp, nw, nh, x_offs, y_offs


def parse_depot_blocks(path: Path) -> dict[str, list[tuple[int, int, int, int, int, int]]]:
    text = path.read_text(encoding="utf-8")
    block_pat = re.compile(
        r"static const DrawTileSeqStruct _road_depot_(NE|SE|SW|NW)\[\] = \{([^}]+)\}",
        re.DOTALL,
    )
    line_pat = re.compile(
        r"TILE_SEQ_LINE\(\s*0x([0-9A-Fa-f]+)[^,]*,\s*[^,]+,\s*"
        r"(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+)"
    )
    out: dict[str, list[tuple[int, int, int, int, int, int]]] = {}
    for m in block_pat.finditer(text):
        name = m.group(1).lower()
        lines = []
        for img_hex, dx, dy, sx, sy in line_pat.findall(m.group(2)):
            sid = int(img_hex, 16) & 0x0FFF
            lines.append((sid, int(dx), int(dy), int(dx), int(dy), int(sx)))
            # sx/sy in TILE_SEQ are size; dz unused → 0
            lines[-1] = (sid, int(dx), int(dy), 0, int(sx), int(sy))
        if lines:
            out[name] = lines
    return out


def find_nfo_files(repo: Path) -> list[Path]:
    out: list[Path] = []
    for root in (repo / "assets" / "opengfx", repo / ".downloads" / "openttd"):
        if root.is_dir():
            out.extend(root.rglob("*.nfo"))
    return out


def detect_graphics_mode(repo: Path) -> str | None:
    marker = repo / "assets" / "opengfx" / ".graphics_mode"
    if marker.is_file():
        mode = marker.read_text(encoding="utf-8").strip()
        if mode in ("8bpp", "32bpp"):
            return mode
    opengfx = repo / "assets" / "opengfx"
    if (opengfx / "opengfx2-32ez").is_dir():
        return "32bpp"
    if any(opengfx.glob("opengfx-*")):
        return "8bpp"
    return None


def parse_sprite_offs(repo: Path) -> dict[int, list[NfoEntry]]:
    pat = re.compile(
        r"^\s*(\d+)\s+\S+\s+(8bpp|32bpp)\s+"
        r"\d+\s+\d+\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    out: dict[int, list[NfoEntry]] = {}
    for nfo in find_nfo_files(repo):
        try:
            content = nfo.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for m in pat.finditer(content):
            sid = int(m.group(1))
            out.setdefault(sid, []).append(
                (m.group(2), int(m.group(3)), int(m.group(4)), int(m.group(5)), int(m.group(6)))
            )
    return out


def png_size(tiles_dir: Path, name: str) -> tuple[int, int] | None:
    path = tiles_dir / name
    if not path.is_file() or Image is None:
        return None
    with Image.open(path) as im:
        return im.size


def pick_sprite_meta(
    entries: list[NfoEntry], png_wh: tuple[int, int] | None, prefer_bpp: str | None
) -> tuple[float, float, float, float, str]:
    if not entries:
        if png_wh:
            return float(png_wh[0]), float(png_wh[1]), 0.0, 0.0, "png-only"
        return 0.0, 0.0, 0.0, 0.0, "missing"
    ordered = sorted(
        entries,
        key=lambda e: (0 if prefer_bpp and e[0] == prefer_bpp else 1, e[0]),
    )
    for bpp, nw, nh, xo, yo in ordered:
        if png_wh and (png_wh[0] != nw or png_wh[1] != nh):
            scale_x = png_wh[0] / max(nw, 1)
            scale_y = png_wh[1] / max(nh, 1)
            if abs(scale_x - scale_y) < 0.01 and scale_x > 1.5:
                return (
                    float(png_wh[0]),
                    float(png_wh[1]),
                    float(xo) * scale_x,
                    float(yo) * scale_y,
                    f"{bpp}-scaled",
                )
        return float(nw), float(nh), float(xo), float(yo), bpp
    return 0.0, 0.0, 0.0, 0.0, "empty"


def sprite_png_name(sid: int) -> str:
    if 1408 <= sid <= 1411:
        return f"road_depot_{sid - 1408}.png"
    return f"rail_{sid}.png"


def write_layers(
    blocks: dict[str, list[tuple[int, int, int, int, int, int]]],
    tiles_dir: Path,
    nfo: dict[int, list[NfoEntry]],
    prefer_bpp: str | None,
) -> list[str]:
    lines: list[str] = []
    for dir_name in DIRS:
        seq = blocks.get(dir_name, [])
        layer_lines: list[str] = []
        for layer_i, (sid, dx, dy, dz, sx, sy) in enumerate(seq):
            png = sprite_png_name(sid)
            wh = png_size(tiles_dir, png)
            entries = nfo.get(sid, [])
            w, h, xo, yo, _note = pick_sprite_meta(entries, wh, prefer_bpp)
            if w <= 0.0 or h <= 0.0:
                w, h = (float(sx * 2), float(sy * 2)) if wh is None else (float(wh[0]), float(wh[1]))
            z = 0.05 + layer_i * 0.01
            layer_lines.append(
                f"        RoadDepotLayerGfx {{ dx: {dx}.0, dy: {dy}.0, dz: {dz}.0, "
                f"z: {z:.2f}, w: {w:.1f}, h: {h:.1f}, x_offs: {xo:.1f}, y_offs: {yo:.1f}, "
                f"remap_x_adj: 0.0, "
                f'path: "assets/opengfx/tiles/{png}" }},'
            )
        lines.append(f"    &[ // {dir_name.upper()}")
        lines.extend(layer_lines or ["        // sin capas"])
        lines.append("    ],")
    return lines


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    upstream = repo / "third_party" / "openttd" / "road_land.h"
    if len(sys.argv) >= 2:
        upstream = Path(sys.argv[1])
    if not upstream.is_file():
        print(f"Falta {upstream}", file=sys.stderr)
        return 1

    blocks = parse_depot_blocks(upstream)
    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    nfo = parse_sprite_offs(repo)
    prefer_bpp = detect_graphics_mode(repo)

    ground_png = "road_depot_ground.png"
    if not (tiles_dir / ground_png).is_file():
        ground_png = "airport_apron.png"

    mode_comment = (
        f"// Modo gráfico detectado: {prefer_bpp}.\n" if prefer_bpp else "// Modo gráfico: auto.\n"
    )
    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "road_depot_gfx_data_generated.rs"
    )
    layer_rows = write_layers(blocks, tiles_dir, nfo, prefer_bpp)
    lines = [
        "// @generated by scripts/gen_road_depot_gfx_data.py — no editar a mano.",
        "// Fuente: OpenTTD road_land.h (_road_depot_NE..NW) + PNG/NFO.",
        mode_comment,
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct RoadDepotLayerGfx {",
        "    pub dx: f32,",
        "    pub dy: f32,",
        "    pub dz: f32,",
        "    pub z: f32,",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub x_offs: f32,",
        "    pub y_offs: f32,",
        "    pub remap_x_adj: f32,",
        "    pub path: &'static str,",
        "}",
        "",
        f'pub const ROAD_DEPOT_GROUND_PATH: &str = "assets/opengfx/tiles/{ground_png}";',
        "",
        "pub const ROAD_DEPOT_BUILD_LAYERS: [&[RoadDepotLayerGfx]; 4] = [",
        *layer_rows,
        "];",
        "",
    ]
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {out_path} ({sum(len(v) for v in blocks.values())} capas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
