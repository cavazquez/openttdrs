#!/usr/bin/env python3
"""Genera `crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap` (2×2, v5+12, TNBP JGR mínimo)."""

from __future__ import annotations

import struct
from pathlib import Path


def write_gamma(v: int, buf: bytearray) -> None:
    if v < (1 << 7):
        buf.append(v)
    elif v < (1 << 14):
        buf.append(0x80 | (v >> 8))
        buf.append(v & 0xFF)
    else:
        raise SystemExit("gamma demasiado grande para el fixture")


def write_string(s: str, buf: bytearray) -> None:
    b = s.encode("utf-8")
    write_gamma(len(b), buf)
    buf.extend(b)


def tnbp_inner() -> bytes:
    inner = bytearray()
    hdr = bytearray()
    SLE_U32, SLE_U8, SLE_I8, SLE_END = 6, 2, 1, 0
    hdr.append(SLE_U32)
    write_string("tile_n", hdr)
    hdr.append(SLE_U32)
    write_string("tile_s", hdr)
    hdr.append(SLE_U8)
    write_string("height", hdr)
    hdr.append(SLE_I8)
    write_string("is_chunnel", hdr)
    hdr.append(SLE_END)
    write_gamma(len(hdr) + 1, inner)
    inner.extend(hdr)
    row = bytearray()
    row.extend(struct.pack(">II", 0, 1))
    row.append(4)
    row.append(0)
    write_gamma(len(row) + 1, inner)
    inner.extend(row)
    write_gamma(0, inner)
    return bytes(inner)


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    out = root / "crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap"
    out.parent.mkdir(parents=True, exist_ok=True)
    w, h = 2, 2
    n = w * h
    body = bytearray()
    body.extend(b"MAP1")
    body.extend(struct.pack("<IIHH", w, h, 1, 1))  # version, flags(HAS_M2_HI)
    # Orden v1: MAPT, MAPH, M1, M2, M2_HI, M3, M3HI, M5, M6, M7, M8
    body.extend([0x90, 0x90, 0x00, 0x00])  # MAPT
    body.extend([1, 1, 1, 1])  # MAPH
    body.extend([0] * n)  # m1
    body.extend([0] * n)  # m2
    body.extend([0] * n)  # m2_hi
    body.extend([0] * n)  # m3
    body.extend([0] * n)  # m3hi
    body.extend([0] * n)  # m5
    body.extend([0] * n)  # m6
    body.extend([0] * n)  # m7
    body.extend([0] * (2 * n))  # m8
    tnbp = tnbp_inner()
    body.extend(b"TNBP")
    body.extend(struct.pack("<I", len(tnbp)))
    body.extend(tnbp)
    out.write_bytes(body)
    print(f"Escrito {out} ({len(body)} bytes)")


if __name__ == "__main__":
    main()
