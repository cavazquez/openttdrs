#!/usr/bin/env python3
from __future__ import annotations

import json
import struct
import sys
from pathlib import Path


def read_map(path: Path) -> tuple[int, int, dict[str, bytes]]:
    data = path.read_bytes()
    if len(data) < 16 or data[:4] != b"MAP1":
        raise ValueError(f"{path}: no es .ottdmap válido (magic MAP1)")
    w = struct.unpack_from("<I", data, 4)[0]
    h = struct.unpack_from("<I", data, 8)[0]
    n = w * h

    def take(off: int, size: int) -> bytes:
        end = off + size
        return data[off:end] if end <= len(data) else b""

    planes: dict[str, bytes] = {}
    off = 16
    planes["mapt"] = take(off, n)
    off += n
    planes["height"] = take(off, n)
    off += n
    planes["m1"] = take(off, n)
    off += n
    planes["m2"] = take(off, n)
    off += n
    planes["m2_hi"] = take(off, n)
    off += n
    planes["m3"] = take(off, n)
    off += n
    planes["m3hi"] = take(off, n)
    off += n
    planes["m5"] = take(off, n)
    off += n
    planes["m6"] = take(off, n)
    off += n
    planes["m7"] = take(off, n)
    off += n
    planes["m8"] = take(off, n * 2)
    return w, h, planes


def b(plane: bytes, i: int) -> int:
    return plane[i] if i < len(plane) else 0


def u16le(plane: bytes, i: int) -> int:
    o = i * 2
    if o + 1 >= len(plane):
        return 0
    return plane[o] | (plane[o + 1] << 8)


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print("Uso: diff_ottdmap_tiles.py <a.ottdmap> <b.ottdmap> [salida.json]")
        return 2

    pa = Path(sys.argv[1])
    pb = Path(sys.argv[2])
    out = Path(sys.argv[3]) if len(sys.argv) == 4 else None

    wa, ha, a = read_map(pa)
    wb, hb, bmap = read_map(pb)
    if (wa, ha) != (wb, hb):
        print(f"Dimensiones distintas: A={wa}x{ha}, B={wb}x{hb}")
        return 1

    n = wa * ha
    rail_diffs = []
    road_diffs = []

    for i in range(n):
        x = i % wa
        y = i // wa
        ta = (b(a["mapt"], i) >> 4) & 0xF
        tb = (b(bmap["mapt"], i) >> 4) & 0xF

        if ta == 1 or tb == 1:  # rail
            a_row = {
                "mapt": b(a["mapt"], i),
                "m5": b(a["m5"], i),
                "m3": b(a["m3"], i),
                "m3hi": b(a["m3hi"], i),
            }
            b_row = {
                "mapt": b(bmap["mapt"], i),
                "m5": b(bmap["m5"], i),
                "m3": b(bmap["m3"], i),
                "m3hi": b(bmap["m3hi"], i),
            }
            if a_row != b_row:
                rail_diffs.append({"x": x, "y": y, "a": a_row, "b": b_row})

        if ta == 2 or tb == 2:  # road
            a_row = {
                "mapt": b(a["mapt"], i),
                "m5": b(a["m5"], i),
                "m8": u16le(a["m8"], i),
            }
            b_row = {
                "mapt": b(bmap["mapt"], i),
                "m5": b(bmap["m5"], i),
                "m8": u16le(bmap["m8"], i),
            }
            if a_row != b_row:
                road_diffs.append({"x": x, "y": y, "a": a_row, "b": b_row})

    report = {
        "map": {"width": wa, "height": ha},
        "rail_diff_count": len(rail_diffs),
        "road_diff_count": len(road_diffs),
        "rail_diff_sample": rail_diffs[:50],
        "road_diff_sample": road_diffs[:50],
    }
    payload = json.dumps(report, indent=2, ensure_ascii=False)
    if out:
        out.write_text(payload, encoding="utf-8")
        print(f"Reporte escrito en {out}")
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
