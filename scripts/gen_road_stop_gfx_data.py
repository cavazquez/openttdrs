#!/usr/bin/env python3
"""Genera road_stop_gfx_data_generated.rs desde OpenTTD station_land.h + PNG/NFO.

Los offsets NFO se eligen por coincidencia de tamaño con el PNG exportado en
`assets/opengfx/tiles/` (válido para OpenGFX 8bpp y OpenGFX2 32bpp): si el PNG
es el doble del recorte NFO, se escalan x_offs/y_offs en la misma proporción.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    Image = None  # type: ignore[misc, assignment]

DIRS = ("ne", "se", "sw", "nw")
TRUCK_DATAS = (67, 68, 69, 70)
BUS_DATAS = (71, 72, 73, 74)
DRIVE_THROUGH_TRUCK_DATAS = (168, 169)
DRIVE_THROUGH_BUS_DATAS = (170, 171)
SPRITE_ID_MIN = 2692
SPRITE_ID_MAX = 2723

# Action5 0x11: bus Y_W/Y_E/X_W/X_E, seguido de truck Y_W/Y_E/X_W/X_E.
DRIVE_THROUGH_NAMES = {
    "bus": ("bus_stop_dt_y_w.png", "bus_stop_dt_y_e.png", "bus_stop_dt_x_w.png", "bus_stop_dt_x_e.png"),
    "truck": (
        "truck_stop_dt_y_w.png",
        "truck_stop_dt_y_e.png",
        "truck_stop_dt_x_w.png",
        "truck_stop_dt_x_e.png",
    ),
}

NfoEntry = tuple[str, int, int, int, int]  # bpp, nw, nh, x_offs, y_offs


def parse_tile_seq_blocks(path: Path) -> dict[int, list[tuple[int, int, int, int, int, int]]]:
    text = path.read_text(encoding="utf-8")
    block_pat = re.compile(
        r"static const DrawTileSeqStruct _station_display_datas_(\d+)\[\] = \{([^}]+)\}",
        re.DOTALL,
    )
    line_pat = re.compile(
        r"TILE_SEQ_LINE\(\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),"
    )
    out: dict[int, list[tuple[int, int, int, int, int, int]]] = {}
    for m in block_pat.finditer(text):
        did = int(m.group(1))
        lines = [tuple(int(g) for g in g) for g in line_pat.findall(m.group(2))]
        if lines:
            out[did] = lines
    return out


def find_nfo_files(repo: Path, mode: str | None) -> list[Path]:
    """NFO del set gráfico activo, sin mezclar IDs repetidos de side-caches."""
    root = repo / "assets" / "opengfx"
    if mode == "32bpp":
        patterns = (
            "opengfx2-*/sprites/ogfx21_base_32ez.nfo",
            "opengfx2-*/sprites/ogfx2e_extra_32ez.nfo",
        )
    else:
        patterns = (
            "opengfx-*/sprites/ogfx1_base.nfo",
            "opengfx-*/sprites/ogfxe_extra.nfo",
        )
    out = [p for pattern in patterns for p in sorted(root.glob(pattern)) if p.is_file()]
    if out:
        return out
    # Diagnóstico/local sin set activo completo.
    return sorted(root.rglob("*.nfo"))


def detect_graphics_mode(repo: Path) -> str | None:
    """Lee assets/opengfx/.graphics_mode o infiere por carpetas OpenGFX instaladas."""
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


def parse_sprite_offs(repo: Path, mode: str | None) -> dict[int, list[NfoEntry]]:
    """Todas las filas 8bpp/32bpp por sprite ID (puede haber más de una por ID)."""
    pat = re.compile(
        r"^\s*(\d+)\s+\S+\s+(8bpp|32bpp)\s+"
        r"\d+\s+\d+\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    out: dict[int, list[NfoEntry]] = {}
    for nfo in find_nfo_files(repo, mode):
        try:
            content = nfo.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for line in content.splitlines():
            m = pat.match(line)
            if not m:
                continue
            sid = int(m.group(1))
            entry: NfoEntry = (
                m.group(2),
                int(m.group(3)),
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
            )
            bucket = out.setdefault(sid, [])
            if entry not in bucket:
                bucket.append(entry)
    return out


def roadstop_action5_sprite_ids(repo: Path, mode: str | None) -> tuple[Path, tuple[int, ...]] | None:
    """Localiza las ocho imágenes reales inmediatamente anteriores a Action5 0x11.

    OpenGFX conserva estos sprites en ``ogfxe_extra`` (no en el GRF base), por
    lo que no los puede extraer ``descargar_graficos.sh`` con su tabla base.
    """
    root = repo / "assets" / "opengfx"
    candidates: list[Path] = []
    if mode == "32bpp":
        candidates.extend(root.glob("opengfx2-*/sprites/ogfx2e_extra_32ez.nfo"))
    else:
        candidates.extend(root.glob("opengfx-*/sprites/ogfxe_extra.nfo"))
    # Sirve también si solo quedó instalado el side-cache 8bpp.
    candidates.extend(root.glob(".signal-src-8bpp/sprites/ogfxe_extra.nfo"))

    action = re.compile(r"^\s*\d+\s+\*\s+5\s+05\s+11\s+FF\s+08\s+00\s*$")
    real = re.compile(r"^\s*(\d+)\s+\S+\s+(?:8bpp|32bpp)\s+")
    for nfo in candidates:
        if not nfo.is_file():
            continue
        last_ids: list[int] = []
        for line in nfo.read_text(encoding="utf-8", errors="replace").splitlines():
            m = real.match(line)
            if m:
                sid = int(m.group(1))
                if not last_ids or last_ids[-1] != sid:
                    last_ids.append(sid)
            if action.match(line) and len(last_ids) >= 8:
                return nfo, tuple(last_ids[-8:])
    return None


def extract_drive_through_tiles(
    tiles_dir: Path, nfo_path: Path, sprite_ids: tuple[int, ...]
) -> None:
    """Recorta los ocho sprites Action5 de paradas drive-through."""
    if Image is None:
        return
    row = re.compile(
        r"^\s*(\d+)\s+(\S+)\s+(8bpp|32bpp)\s+"
        r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    sources: dict[int, tuple[Path, int, int, int, int]] = {}
    # El renderer usa las hojas base 8bpp aun en OpenGFX2: mantiene el tamaño
    # isométrico de 64×31 del resto de los tiles y evita coordenadas zi4.
    for line in nfo_path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = row.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in sprite_ids or m.group(3) != "8bpp" or sid in sources:
            continue
        sources[sid] = (
            nfo_path.parent / Path(m.group(2)).name,
            int(m.group(4)),
            int(m.group(5)),
            int(m.group(6)),
            int(m.group(7)),
        )

    names = (*DRIVE_THROUGH_NAMES["bus"], *DRIVE_THROUGH_NAMES["truck"])
    for sid, name in zip(sprite_ids, names, strict=True):
        source = sources.get(sid)
        if source is None or not source[0].is_file():
            print(f"  (omitido {name}: sprite Action5 {sid} sin hoja 8bpp)", file=sys.stderr)
            continue
        sheet, x, y, w, h = source
        with Image.open(sheet) as image:
            crop = image.crop((x, y, x + w, y + h)).convert("RGBA")
        # El GRF extra usa azul puro como índice transparente en estas tiras;
        # a diferencia de las hojas base, ese índice no llega marcado como
        # alpha en el PNG de grfcodec.
        pixels = crop.get_flattened_data()
        crop.putdata(
            [(0, 0, 0, 0) if pixel[:3] == (0, 0, 255) else pixel for pixel in pixels]
        )
        crop.save(tiles_dir / name)
        print(f"  {name} ({w}×{h}) ← Action5 roadstop {sid} [{sheet.name}]")


def png_size(tiles_dir: Path, name: str) -> tuple[int, int] | None:
    if Image is None:
        return None
    p = tiles_dir / name
    if not p.is_file():
        return None
    with Image.open(p) as im:
        return im.size


def pick_sprite_meta(
    entries: list[NfoEntry],
    png_wh: tuple[int, int] | None,
    prefer_bpp: str | None,
) -> tuple[float, float, float, float, str]:
    """Devuelve (w, h, x_offs, y_offs, nota) escalando offsets al tamaño real del PNG."""
    if not entries:
        return 0.0, 0.0, 0.0, 0.0, "sin_nfo"

    def rank(e: NfoEntry) -> tuple[int, int]:
        bpp, nw, nh, _, _ = e
        size_err = 0
        if png_wh:
            pw, ph = png_wh
            size_err = abs(nw - pw) + abs(nh - ph)
        bpp_penalty = 0 if prefer_bpp and bpp == prefer_bpp else 1
        return (size_err, bpp_penalty)

    bpp, nw, nh, xr, yr = min(entries, key=rank)

    if png_wh:
        pw, ph = png_wh
        w, h = float(pw), float(ph)
        sx = w / float(nw) if nw else 1.0
        sy = h / float(nh) if nh else 1.0
        note = f"nfo_{bpp}_scale_{sx:.2f}x{sy:.2f}"
        if abs(sx - 1.0) < 0.05 and abs(sy - 1.0) < 0.05:
            note = f"nfo_{bpp}_match"
        return w, h, float(xr) * sx, float(yr) * sy, note

    w, h = float(nw), float(nh)
    return w, h, float(xr), float(yr), f"nfo_{bpp}_only"


def compute_layer_corrections(
    dx: float,
    dy: float,
    *,
    is_bus: bool,
    dir_i: int,
    layer_i: int,
) -> tuple[float, float]:
    """(remap_x_adj, y_offs_delta). Unidades TILE_SEQ en X (×4 px); y_offs_delta en px."""
    if is_bus:
        # Checklist SP3 y=9: calibración por capa/dirección (RemapCoords + NFO).
        if dir_i == 0 and layer_i == 2 and dx == 0.0 and dy == 13.0:
            return -13.0, 0.0
        if dir_i == 0 and layer_i == 1 and dx == 13.0 and dy == 0.0:
            return 7.0, -6.0
        if dir_i == 1 and layer_i == 0 and dx == 0.0 and dy == 3.0:
            return -3.0, 0.0
        if dir_i == 1 and layer_i == 2 and dx == 13.0 and dy == 3.0:
            return 5.0, -9.0
        if dir_i == 2 and layer_i == 0 and dx == 3.0 and dy == 15.0:
            return -8.0, -11.0
        if dir_i == 3 and layer_i == 0 and dx == 15.0 and dy == 2.0:
            return 8.0, -(dx - dy) * 2.0 + 8.0
        if dir_i == 3 and layer_i == 1 and dx == 0.0 and dy == 13.0:
            return -7.0, -6.0
        return 0.0, 0.0

    # Camión: mismos patrones de esquina + capas con dy=3 / dy=15.
    if dir_i == 0 and layer_i == 1 and dx == 13.0 and dy == 0.0:
        return 7.0, -6.0
    if dir_i == 1 and layer_i == 0 and dx == 15.0 and dy == 3.0:
        return 8.0, -(dx - dy) * 2.0 + 8.0
    if dir_i == 1 and layer_i == 2 and dx == 0.0 and dy == 3.0:
        return -3.0, 0.0
    if dir_i == 0 and layer_i == 0 and dx == 0.0 and dy == 15.0:
        return -9.0, -8.0
    if dir_i == 3 and layer_i == 1 and dx == 0.0 and dy == 13.0:
        return -7.0, -6.0
    if dir_i == 3 and layer_i == 2 and dx == 15.0 and dy == 2.0:
        return 8.0, -(dx - dy) * 2.0 + 8.0
    return 0.0, 0.0


def layer_sprite_id(is_bus: bool, dir_i: int, layer_i: int) -> int:
    """IDs alineados con scripts/descargar_graficos.sh (build_a +4, +8, +12 por dir)."""
    base = 2692 if is_bus else 2708
    return base + dir_i + 4 + layer_i * 4


def write_layers(
    blocks: dict[int, list[tuple[int, int, int, int, int, int]]],
    datas: tuple[int, ...],
    prefix: str,
    is_bus: bool,
    tiles_dir: Path,
    nfo: dict[int, list[NfoEntry]],
    prefer_bpp: str | None,
) -> list[str]:
    lines: list[str] = []
    for dir_i, did in enumerate(datas):
        seq = blocks.get(did, [])
        for layer_i, (dx, dy, dz, sx, sy, sz) in enumerate(seq[:3]):
            layer = ("a", "b", "c")[layer_i]
            png = f"{prefix}_{DIRS[dir_i]}_build_{layer}.png"
            sid = layer_sprite_id(is_bus, dir_i, layer_i)
            wh = png_size(tiles_dir, png)
            entries = nfo.get(sid, [])
            w, h, xo, yo, _note = pick_sprite_meta(entries, wh, prefer_bpp)
            if w <= 0.0 or h <= 0.0:
                w, h = (float(sx * 2), float(sz * 2)) if wh is None else (float(wh[0]), float(wh[1]))
            z = 0.05 + layer_i * 0.01
            adj, yo_delta = compute_layer_corrections(
                float(dx), float(dy), is_bus=is_bus, dir_i=dir_i, layer_i=layer_i
            )
            yo += yo_delta
            lines.append(
                f"        RoadStopLayerGfx {{ dx: {dx}.0, dy: {dy}.0, dz: {dz}.0, "
                f"z: {z:.2f}, w: {w:.1f}, h: {h:.1f}, x_offs: {xo:.1f}, y_offs: {yo:.1f}, "
                f"remap_x_adj: {adj:.1f}, "
                f'path: "assets/opengfx/tiles/{png}" }},'
            )
    return lines


def write_drive_through_layers(
    blocks: dict[int, list[tuple[int, int, int, int, int, int]]],
    datas: tuple[int, int],
    prefix: str,
    sprite_ids: tuple[int, ...],
    tiles_dir: Path,
    nfo: dict[int, list[NfoEntry]],
    prefer_bpp: str | None,
) -> list[str]:
    """Genera X/Y × W/E para las dos tiras que forman una parada pasante."""
    # Action5 enumera Y antes de X; el cliente indexa [X, Y].
    ids_by_axis = ((sprite_ids[2], sprite_ids[3]), (sprite_ids[0], sprite_ids[1]))
    names = DRIVE_THROUGH_NAMES[prefix]
    names_by_axis = ((names[2], names[3]), (names[0], names[1]))
    flat: list[str] = []
    for axis_i, data_id in enumerate(datas):
        seq = blocks.get(data_id, [])[:2]
        for layer_i, (dx, dy, dz, sx, _sy, sz) in enumerate(seq):
            png = names_by_axis[axis_i][layer_i]
            sid = ids_by_axis[axis_i][layer_i]
            wh = png_size(tiles_dir, png)
            w, h, xo, yo, _note = pick_sprite_meta(nfo.get(sid, []), wh, prefer_bpp)
            if w <= 0.0 or h <= 0.0:
                w, h = (float(sx * 2), float(sz * 2)) if wh is None else (float(wh[0]), float(wh[1]))
            flat.append(
                f"        RoadStopLayerGfx {{ dx: {dx}.0, dy: {dy}.0, dz: {dz}.0, "
                f"z: {0.05 + layer_i * 0.01:.2f}, w: {w:.1f}, h: {h:.1f}, "
                f"x_offs: {xo:.1f}, y_offs: {yo:.1f}, remap_x_adj: 0.0, "
                f'path: "assets/opengfx/tiles/{png}" }},'
            )
    return flat


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    upstream = repo / "third_party" / "openttd" / "station_land.h"
    if len(sys.argv) >= 2:
        upstream = Path(sys.argv[1])
    if not upstream.is_file():
        print(f"Falta {upstream}", file=sys.stderr)
        return 1

    blocks = parse_tile_seq_blocks(upstream)
    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    prefer_bpp = detect_graphics_mode(repo)
    action5 = roadstop_action5_sprite_ids(repo, prefer_bpp)
    if action5 is None:
        print("No encontré Action5 0x11 de roadstops; se omiten sprites drive-through.", file=sys.stderr)
        # Mantiene el archivo Rust válido para inspecciones sin assets, pero la
        # descarga normal siempre encontrará el set extra.
        dt_sprite_ids = (0,) * 8
    else:
        action5_nfo, dt_sprite_ids = action5
        extract_drive_through_tiles(tiles_dir, action5_nfo, dt_sprite_ids)

    nfo = parse_sprite_offs(repo, prefer_bpp)

    bus_flat = write_layers(blocks, BUS_DATAS, "bus_stop", True, tiles_dir, nfo, prefer_bpp)
    truck_flat = write_layers(blocks, TRUCK_DATAS, "truck_stop", False, tiles_dir, nfo, prefer_bpp)
    bus_drive_through = write_drive_through_layers(
        blocks,
        DRIVE_THROUGH_BUS_DATAS,
        "bus",
        dt_sprite_ids[:4],
        tiles_dir,
        nfo,
        prefer_bpp,
    )
    truck_drive_through = write_drive_through_layers(
        blocks,
        DRIVE_THROUGH_TRUCK_DATAS,
        "truck",
        dt_sprite_ids[4:],
        tiles_dir,
        nfo,
        prefer_bpp,
    )

    def block(name: str, flat: list[str]) -> list[str]:
        rows = [f"pub const {name}: [[RoadStopLayerGfx; 3]; 4] = ["]
        for i in range(4):
            rows.append(f"    [ // {DIRS[i].upper()}")
            rows.extend(flat[i * 3 : (i + 1) * 3])
            rows.append("    ],")
        rows.append("];")
        return rows

    def drive_through_block(name: str, flat: list[str]) -> list[str]:
        rows = [f"pub const {name}: [[RoadStopLayerGfx; 2]; 2] = ["]
        for i, axis in enumerate(("X", "Y")):
            rows.append(f"    [ // {axis}")
            rows.extend(flat[i * 2 : (i + 1) * 2])
            rows.append("    ],")
        rows.append("];")
        return rows

    mode_comment = (
        f"// Modo gráfico detectado: {prefer_bpp} (assets/opengfx/.graphics_mode o carpetas).\n"
        if prefer_bpp
        else "// Modo gráfico: desconocido; offsets NFO elegidos por tamaño del PNG.\n"
    )
    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "road_stop_gfx_data_generated.rs"
    )
    lines = [
        "// @generated by scripts/gen_road_stop_gfx_data.py — no editar a mano.",
        "// Fuente: OpenTTD station_land.h (_station_display_datas_67..74, 168..171) + PNG/NFO.",
        "// remap_x_adj: corrección fina por capa (±1 unidad TILE_SEQ ≈ 4 px); 0 = solo RemapCoords+NFO.",
        mode_comment,
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct RoadStopLayerGfx {",
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
        *block("BUS_STOP_BUILD_LAYERS", bus_flat),
        "",
        *block("TRUCK_STOP_BUILD_LAYERS", truck_flat),
        "",
        *drive_through_block("BUS_STOP_DRIVE_THROUGH_LAYERS", bus_drive_through),
        "",
        *drive_through_block("TRUCK_STOP_DRIVE_THROUGH_LAYERS", truck_drive_through),
        "",
    ]
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {out_path} (NFO IDs: {len(nfo)}, modo: {prefer_bpp or 'auto'})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
