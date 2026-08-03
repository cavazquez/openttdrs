#!/usr/bin/env python3
"""
Save sintético OpenTTD (OTTN sin compresión) con señales block/entry/exit/combo/path/oneway.

Mapa 64×64: fila y=8 con vía horizontal (TRACK_X) y señales en x=1..6 (tipos 0–5).
Cada señal mira al NE (sig_bit 2) sobre carril X, variante eléctrica.

Uso:
  python3 scripts/gen_rail_signals_sav.py [salida.sav]
  python3 scripts/gen_rail_signals_sav.py \\
    crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

W = H = 64
N = W * H
SAVE_VERSION = 350

MP_RAILWAY = 1
TRACK_X = 1
RAIL_TILE_SIGNALS = 1
CH_RIFF = 0
CH_TABLE = 3

# Fila de señales (alineada con build_rail_signals_mixed: x=1..6).
SIGNAL_ROW_Y = 8
SIGNAL_X_TYPES: list[tuple[int, int]] = [
    (1, 0),
    (2, 1),
    (3, 2),
    (4, 3),
    (5, 4),
    (6, 5),
]
# Cara NE sobre TRACK_X (`signal_bit_for_facing`, face 0).
SIG_BIT_NE = 2


def write_gamma(v: int, buf: bytearray) -> None:
    assert v < (1 << 14), "el generador usa gammas pequeños"
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


def idx(x: int, y: int) -> int:
    return y * W + x


def signal_m2(sig_type: int, variant: int = 1) -> int:
    return (sig_type & 7) | (variant << 3)


def signal_present_m3(sig_bit: int) -> int:
    return (1 << sig_bit) << 4


def build_map_planes() -> tuple[bytearray, bytearray, bytearray, bytearray, bytearray, bytearray]:
    mapt = bytearray(N)
    maph = bytearray(N)
    m5 = bytearray(N)
    m6 = bytearray(N)
    m8 = bytearray(N * 2)
    map2 = bytearray(N * 2)
    m3lo = bytearray(N)
    m3hi = bytearray(N)

    def set_tile(
        x: int,
        y: int,
        *,
        m5v: int = 0,
        m2_lo: int = 0,
        m2_hi: int = 0,
        m3_lo: int = 0,
        m3_hi: int = 0,
    ) -> None:
        i = idx(x, y)
        mapt[i] = (MP_RAILWAY << 4) & 0xFF
        m5[i] = m5v & 0xFF
        m3lo[i] = m3_lo & 0xFF
        m3hi[i] = m3_hi & 0xFF
        map2[i * 2] = m2_hi & 0xFF
        map2[i * 2 + 1] = m2_lo & 0xFF

    for x in range(0, 8):
        set_tile(x, SIGNAL_ROW_Y, m5v=TRACK_X)

    present = signal_present_m3(SIG_BIT_NE)
    for x, sig_type in SIGNAL_X_TYPES:
        set_tile(
            x,
            SIGNAL_ROW_Y,
            m5v=TRACK_X | (RAIL_TILE_SIGNALS << 6),
            m2_lo=signal_m2(sig_type),
            m3_lo=present,
            m3_hi=present,
        )

    return mapt, maph, m5, m6, m8, map2, m3lo, m3hi


def build_sav() -> bytes:
    mapt, maph, m5, m6, m8, map2, m3lo, m3hi = build_map_planes()

    data = bytearray()
    # MAPS CH_TABLE (SLV ≥ 294), igual que gen_demo_sav / writer Rust.
    data.extend(
        table_chunk(
            b"MAPS",
            [(6, "dim_x"), (6, "dim_y")],
            [struct.pack(">II", W, H)],
        )
    )
    data.extend(riff_chunk(b"MAPT", bytes(mapt)))
    data.extend(riff_chunk(b"MAPH", bytes(maph)))
    data.extend(riff_chunk(b"MAPO", bytes(N)))
    data.extend(riff_chunk(b"MAP2", bytes(map2)))
    data.extend(riff_chunk(b"M3LO", bytes(m3lo)))
    data.extend(riff_chunk(b"M3HI", bytes(m3hi)))
    data.extend(riff_chunk(b"MAP5", bytes(m5)))
    data.extend(riff_chunk(b"MAPE", bytes(m6)))
    data.extend(riff_chunk(b"MAP7", bytes(N)))
    data.extend(riff_chunk(b"MAP8", bytes(m8)))
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
        else Path("crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav")
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(build_sav())
    print(f"✓ Escrito: {out_path} ({out_path.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
