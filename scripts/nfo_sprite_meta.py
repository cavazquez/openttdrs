#!/usr/bin/env python3
"""Metadatos w/h/xrel/yrel desde NFO OpenGFX + PNG en assets/opengfx/tiles/."""
from __future__ import annotations

import re
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    Image = None  # type: ignore[misc, assignment]

NfoEntry = tuple[str, int, int, int, int]  # bpp, nw, nh, x_offs, y_offs


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


def active_global_sprite_nfo(repo: Path, mode: str | None = None) -> Path | None:
    """Devuelve el NFO del GRF base del perfil gráfico activo.

    Los IDs numéricos dentro de un NFO son locales a cada GRF. Por eso una
    constante global ``SPR_*`` de OpenTTD sólo se puede resolver contra
    ``ogfx1_base``/``ogfx21_base_32ez``; ``ogfxe_extra`` se consume mediante
    extractores Action5 específicos y nunca se mezcla aquí por número.
    """
    mode = mode or detect_graphics_mode(repo)
    root = repo / "assets" / "opengfx"
    if mode == "32bpp":
        candidates = sorted(
            root.glob("opengfx2-*/sprites/ogfx21_base_32ez.nfo"), reverse=True
        )
    elif mode == "8bpp":
        candidates = sorted(root.glob("opengfx-*/sprites/ogfx1_base.nfo"), reverse=True)
    else:
        return None
    return next((path for path in candidates if path.is_file()), None)


def find_nfo_files(repo: Path, mode: str | None = None) -> list[Path]:
    """Compatibilidad: el único NFO válido para IDs globales del perfil activo."""
    path = active_global_sprite_nfo(repo, mode)
    return [path] if path is not None else []


def parse_sprite_offs(repo: Path, mode: str | None = None) -> dict[int, list[NfoEntry]]:
    """Filas NFO de IDs globales OpenTTD del GRF base activo.

    No se incluyen NFO ``extra`` ni side-caches: sus IDs se reinician por GRF
    y una mezcla silenciosa puede sustituir un sprite de 64×31 por uno pequeño
    e irrelevante de otro namespace.
    """
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


def png_size(tiles_dir: Path, png_name: str) -> tuple[int, int] | None:
    if Image is None:
        return None
    p = tiles_dir / png_name
    if not p.is_file():
        return None
    with Image.open(p) as im:
        return im.size


def pick_sprite_meta(
    entries: list[NfoEntry],
    png_wh: tuple[int, int] | None,
    prefer_bpp: str | None,
) -> tuple[float, float, float, float, str]:
    """(w, h, xrel, yrel, nota) escalando offsets si el PNG difiere del recorte NFO."""
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
        note = f"nfo_{bpp}_scale"
        if abs(sx - 1.0) < 0.05 and abs(sy - 1.0) < 0.05:
            note = f"nfo_{bpp}_match"
        return w, h, float(xr) * sx, float(yr) * sy, note

    return float(nw), float(nh), float(xr), float(yr), f"nfo_{bpp}_only"


def sprite_dims_from_assets(
    repo: Path,
    tiles_dir: Path,
    nfo: dict[int, list[NfoEntry]],
    sprite_id: int,
    png_name: str,
    prefer_bpp: str | None,
    *,
    macro_dx: int = 0,
    macro_dy: int = 0,
    macro_sx: int = 0,
    macro_sy: int = 0,
    fallback: tuple[float, float, float, float] = (64.0, 48.0, -32.0, -32.0),
) -> tuple[float, float, float, float, str]:
    """Resuelve dims para un sprite_id con PNG + NFO; macro solo si falta ambos."""
    if sprite_id == 0:
        return (0.0, 0.0, 0.0, 0.0, "none")
    wh = png_size(tiles_dir, png_name)
    w, h, xr, yr, note = pick_sprite_meta(nfo.get(sprite_id, []), wh, prefer_bpp)
    if w > 0.0 and h > 0.0:
        return w, h, xr, yr, note
    w = max(float(macro_sx) * 8.0, 32.0)
    h = max(float(macro_sy) * 8.0, 24.0)
    xr = float(macro_dx) * 8.0 - w * 0.5 + 8.0
    yr = float(macro_dy) * 8.0 - h + 12.0
    return w, h, xr, yr, "macro"
