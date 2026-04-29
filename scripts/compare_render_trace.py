#!/usr/bin/env python3
from __future__ import annotations

import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Tile:
    height: int
    kind: str
    mapt: int
    m5: int


class MapData:
    def __init__(self, path: Path):
        data = path.read_bytes()
        if len(data) < 16 or data[:4] != b"MAP1":
            raise ValueError(f"{path}: no es .ottdmap válido (magic MAP1)")
        self.path = str(path)
        self.w, self.h = struct.unpack_from("<II", data, 4)
        n = self.w * self.h
        off = 16

        def take(size: int) -> bytes:
            nonlocal off
            chunk = data[off : off + size]
            off += size
            return chunk if len(chunk) == size else b""

        self.mapt = take(n)
        self.heights = take(n)
        self.m1 = take(n)
        self.m2 = take(n)
        self.m2_hi = take(n)
        self.m3 = take(n)
        self.m3hi = take(n)
        self.m5 = take(n)
        self.m6 = take(n)
        self.m7 = take(n)
        self.m8 = take(2 * n)

    def in_bounds(self, x: int, y: int) -> bool:
        return 0 <= x < self.w and 0 <= y < self.h

    def idx(self, x: int, y: int) -> int:
        return y * self.w + x

    def tile(self, x: int, y: int) -> Tile:
        i = self.idx(x, y)
        mapt = self.mapt[i]
        mapt_n = (mapt >> 4) & 0xF
        kind = ottd_kind_from_type(mapt_n, self.m5[i] if i < len(self.m5) else 0)
        return Tile(
            height=self.heights[i] if i < len(self.heights) else 0,
            kind=kind,
            mapt=mapt,
            m5=self.m5[i] if i < len(self.m5) else 0,
        )


def ottd_kind_from_type(mapt_n: int, m5: int) -> str:
    if mapt_n in (0, 10):
        return "Grass"
    if mapt_n == 1:
        return "Rail"
    if mapt_n == 2:
        return "Road"
    if mapt_n == 3:
        return "House"
    if mapt_n == 4:
        return "Forest"
    if mapt_n == 5:
        return "Station"
    if mapt_n == 6:
        return "Water"
    if mapt_n == 7:
        return "Void"
    if mapt_n == 8:
        return "Industry"
    if mapt_n == 9:
        return "Rail" if (m5 & 0x04) else "Road"
    return f"Unknown({mapt_n})"


def slope_bits_from_corner_vals(hn: int, hw: int, he: int, hs: int) -> tuple[int, int]:
    min_h = min(hn, hw, he, hs)
    t = 0
    if hw > min_h:
        t |= 1
    if hs > min_h:
        t |= 2
    if he > min_h:
        t |= 4
    if hn > min_h:
        t |= 8
    return min(t, 14), min_h


def water_void_effective_height_for_slope(m: MapData, x: int, y: int, stored: int) -> int:
    if stored != 0:
        return stored
    neigh8 = [(0, -1), (0, 1), (-1, 0), (1, 0), (-1, -1), (-1, 1), (1, -1), (1, 1)]
    best = None
    for dx, dy in neigh8:
        nx, ny = x + dx, y + dy
        if not m.in_bounds(nx, ny):
            continue
        t = m.tile(nx, ny)
        if t.kind in ("Water", "Void"):
            continue
        best = t.height if best is None else min(best, t.height)
    return stored if best is None else best


def slope_sample_h(m: MapData, x: int, y: int) -> int:
    if not m.in_bounds(x, y):
        return 0
    t = m.tile(x, y)
    if t.kind in ("Water", "Void"):
        return water_void_effective_height_for_slope(m, x, y, t.height)
    return t.height


def tile_slope_and_min_z(m: MapData, x: int, y: int) -> tuple[int, int]:
    hn = slope_sample_h(m, x, y)
    hw = slope_sample_h(m, x + 1, y)
    he = slope_sample_h(m, x, y + 1)
    hs = slope_sample_h(m, x + 1, y + 1)
    tileh, min_h = slope_bits_from_corner_vals(hn, hw, he, hs)
    is_water = m.tile(x, y).kind == "Water"
    return (0 if is_water else tileh), min_h


def tile_slope_bits_from_heights(m: MapData, x: int, y: int) -> tuple[int, int]:
    hn = slope_sample_h(m, x, y)
    hw = slope_sample_h(m, x + 1, y)
    he = slope_sample_h(m, x, y + 1)
    hs = slope_sample_h(m, x + 1, y + 1)
    return slope_bits_from_corner_vals(hn, hw, he, hs)


def infer_coast_tileh_when_flat(m: MapData, x: int, y: int) -> int:
    def is_land(nx: int, ny: int) -> bool:
        if not m.in_bounds(nx, ny):
            return False
        k = m.tile(nx, ny).kind
        return k not in ("Water", "Void")

    land_w = is_land(x + 1, y)
    land_e = is_land(x, y + 1)
    land_s = is_land(x + 1, y + 1)
    land_n_side = is_land(x, y - 1)
    land_w_side = is_land(x - 1, y)

    if land_n_side and (land_w or land_e or land_s):
        return 9
    if land_w and land_s:
        return 3
    if land_e and land_s:
        return 6
    if land_w and land_n_side:
        return 9
    if land_e and land_n_side:
        return 12
    if land_w:
        south_diag_hint = is_land(x - 1, y + 1) or is_land(x, y + 2) or is_land(x + 1, y + 2)
        return 3 if south_diag_hint else 1
    if land_s:
        return 2
    if land_e:
        south_diag_hint = is_land(x + 1, y - 1) or is_land(x + 2, y) or is_land(x + 2, y + 1)
        return 6 if south_diag_hint else 4
    if land_n_side or land_w_side:
        return 8
    return 8


def shore_tileh_for_draw_shore(m: MapData, x: int, y: int) -> int:
    raw, _ = tile_slope_bits_from_heights(m, x, y)
    if raw == 0:
        return infer_coast_tileh_when_flat(m, x, y)
    if raw not in (1, 2, 3, 4, 6, 8, 9, 12):
        return infer_coast_tileh_when_flat(m, x, y)
    return raw


def shore_png_index(tileh: int) -> int:
    return {
        1: 1,
        2: 2,
        3: 6,
        4: 0,
        6: 4,
        8: 3,
        9: 7,
        12: 5,
    }.get(min(tileh, 14), 0)


def water_tile_touches_land(m: MapData, x: int, y: int) -> bool:
    neigh = [(-1, 0), (1, 0), (0, -1), (0, 1)]
    for dx, dy in neigh:
        nx, ny = x + dx, y + dy
        if not m.in_bounds(nx, ny):
            continue
        k = m.tile(nx, ny).kind
        if k not in ("Water", "Void"):
            return True
    return False


def use_shore(m: MapData, x: int, y: int) -> bool:
    t = m.tile(x, y)
    if t.kind != "Water":
        return False
    water_tile_type = (t.m5 >> 4) & 0xF
    return water_tile_type == 1 or (water_tile_type == 0 and water_tile_touches_land(m, x, y))


def render_trace_for_tile(m: MapData, x: int, y: int) -> dict:
    t = m.tile(x, y)
    tileh, base_z = tile_slope_and_min_z(m, x, y)
    shore = use_shore(m, x, y)
    th = shore_tileh_for_draw_shore(m, x, y) if shore else None
    si = shore_png_index(th) if th is not None else None
    return {
        "kind": t.kind,
        "tileh": tileh,
        "base_z": base_z,
        "use_shore": shore,
        "shore_tileh": th,
        "shore_png_index": si,
    }


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print("Uso: compare_render_trace.py <a.ottdmap> <b.ottdmap> [out.json]")
        return 2
    a = MapData(Path(sys.argv[1]))
    b = MapData(Path(sys.argv[2]))
    if (a.w, a.h) != (b.w, b.h):
        print(f"Dimensiones distintas: {a.w}x{a.h} vs {b.w}x{b.h}")
        return 1

    n = a.w * a.h
    mismatches = {"kind": 0, "tileh": 0, "base_z": 0, "use_shore": 0, "shore_tileh": 0, "shore_png_index": 0}
    samples = []
    sample_limit = 80

    for y in range(a.h):
        for x in range(a.w):
            ta = render_trace_for_tile(a, x, y)
            tb = render_trace_for_tile(b, x, y)
            row_has_mismatch = False
            for k in mismatches:
                if ta[k] != tb[k]:
                    mismatches[k] += 1
                    row_has_mismatch = True
            if row_has_mismatch and len(samples) < sample_limit:
                samples.append({"x": x, "y": y, "a": ta, "b": tb})

    report = {
        "map": {"width": a.w, "height": a.h, "tile_count": n},
        "source_a": a.path,
        "source_b": b.path,
        "mismatch_counts": mismatches,
        "mismatch_percent": {k: round(v * 100.0 / n, 4) for k, v in mismatches.items()},
        "samples": samples,
    }
    payload = json.dumps(report, indent=2, ensure_ascii=False)
    if len(sys.argv) == 4:
        Path(sys.argv[3]).write_text(payload, encoding="utf-8")
        print(f"Reporte escrito en {sys.argv[3]}")
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
