#!/usr/bin/env python3
"""
Fixture mínimo cargable por OpenTTD 15.3 dedicated (MVP #226).

Contenido: MAPS CH_TABLE + planos RIFF vacíos 64×64 + CITY (≥1) + DATE + PLYR.
Sin STNN/VEHS/INDY (fixture base). Con estaciones: `mvp_openttd_stations.sav`
(regenerar con `OPENTTDRS_DUMP_MVP_STATIONS_SAV=... cargo test ...export_stnn_is_modern_savebyte_schema`).

Uso:
  python3 scripts/gen_mvp_openttd_load_sav.py \\
    crates/openttdrs-core/tests/fixtures/mvp_openttd_load.sav
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

W = H = 64
N = W * H
SAVE_VERSION = 350  # mínimo viable; ver EXPORT_SAVE_VERSION en sav/write/mod.rs

CH_RIFF = 0
CH_TABLE = 3


def write_gamma(v: int, buf: bytearray) -> None:
    assert v < (1 << 14)
    if v < (1 << 7):
        buf.append(v)
    else:
        buf.append(0x80 | (v >> 8))
        buf.append(v & 0xFF)


def write_str(s: str, buf: bytearray) -> None:
    raw = s.encode("utf-8")
    write_gamma(len(raw), buf)
    buf.extend(raw)


def riff_chunk(name: bytes, payload: bytes) -> bytes:
    size = len(payload)
    out = bytearray(name)
    out.append(((size >> 24) << 4) | CH_RIFF)
    out.append((size >> 16) & 0xFF)
    out.append((size >> 8) & 0xFF)
    out.append(size & 0xFF)
    out.extend(payload)
    return bytes(out)


def table_chunk(name: bytes, fields: list[tuple[int, str]], records: list[bytes]) -> bytes:
    header = bytearray()
    for ftype, key in fields:
        header.append(ftype)
        write_str(key, header)
    header.append(0)
    out = bytearray(name)
    out.append(CH_TABLE)
    write_gamma(len(header) + 1, out)
    out.extend(header)
    for rec in records:
        write_gamma(len(rec) + 1, out)
        out.extend(rec)
    write_gamma(0, out)
    return bytes(out)


def build_sav() -> bytes:
    data = bytearray()
    data.extend(
        table_chunk(
            b"MAPS",
            [(6, "dim_x"), (6, "dim_y")],
            [struct.pack(">II", W, H)],
        )
    )
    for name, size in (
        (b"MAPT", N),
        (b"MAPH", N),
        (b"MAPO", N),
        (b"MAP2", N * 2),
        (b"M3LO", N),
        (b"M3HI", N),
        (b"MAP5", N),
        (b"MAPE", N),
        (b"MAP7", N),
        (b"MAP8", N * 2),
    ):
        data.extend(riff_chunk(name, bytes(size)))

    city = bytearray()
    city.extend(struct.pack(">I", (H // 2) * W + (W // 2)))
    write_str("Town", city)
    city.extend(struct.pack(">I", 500))
    city.extend(struct.pack(">I", 0))
    city.extend(struct.pack(">H", 0x20C0))
    city.extend(struct.pack(">I", 0))
    data.extend(
        table_chunk(
            b"CITY",
            [
                (6, "xy"),
                (0x0A | 0x10, "name"),
                (6, "cache.population"),
                (6, "townnamegrfid"),
                (4, "townnametype"),
                (6, "townnameparts"),
            ],
            [bytes(city)],
        )
    )

    date = struct.pack(">i", 1950 * 365) + struct.pack(">Q", 1000)
    data.extend(table_chunk(b"DATE", [(5, "date"), (8, "tick_counter")], [date]))
    plyr = struct.pack(">q", 100_000) + bytes([1])
    data.extend(table_chunk(b"PLYR", [(7, "money"), (2, "colour")], [plyr]))
    data.extend(b"\x00\x00\x00\x00")

    out = bytearray(b"OTTN")
    out.extend(struct.pack(">H", SAVE_VERSION))
    out.extend(b"\x00\x00")
    out.extend(data)
    return bytes(out)


def main() -> None:
    out_path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path("crates/openttdrs-core/tests/fixtures/mvp_openttd_load.sav")
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    raw = build_sav()
    out_path.write_bytes(raw)
    print(f"✓ Escrito: {out_path} ({len(raw):,} bytes)")


if __name__ == "__main__":
    main()
