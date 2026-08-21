#!/usr/bin/env python3
"""Pruebas puras del harness de mapas aleatorios."""

from __future__ import annotations

import tempfile
from pathlib import Path

import random_map_parity as matrix


def test_parse_matrix_sorts_and_validates() -> None:
    assert matrix.parse_matrix("256:2,64:8,128:4") == [(64, 8), (128, 4), (256, 2)]
    try:
        matrix.parse_matrix("32:1")
    except matrix.MatrixError:
        pass
    else:
        raise AssertionError("un tamaño menor que 64 debe rechazarse")


def test_compare_tiles_reports_4x4_blocks() -> None:
    base = [(0, 0, 0, 0, 0, 0, 0, 0, 0, 0) for _ in range(64)]
    candidate = list(base)
    candidate[1] = (1, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    candidate[34] = (0, 0, 0, 0, 0, 7, 0, 0, 0, 0)
    report = matrix.compare_tiles(base, candidate, 8, 8)
    assert report["tile_difference_count"] == 2
    assert report["changed_block_count"] == 2
    assert report["changed_block_ratio"] == 2 / 4
    assert report["field_difference_counts"] == {"height": 1, "m4": 1}


def test_write_ottdmap_preserves_dense_planes() -> None:
    metadata = {"width": 2, "height": 2}
    tiles = [
        (4, 0x10, 1, 0x1234, 5, 6, 7, 8, 9, 0xBEEF),
        (4, 0x20, 2, 0xABCD, 6, 7, 8, 9, 10, 0xCAFE),
        (5, 0x30, 3, 0x0102, 7, 8, 9, 10, 11, 0x0001),
        (6, 0x40, 4, 0x0304, 8, 9, 10, 11, 12, 0x0203),
    ]
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "map.ottdmap"
        matrix.write_ottdmap(path, metadata, tiles)
        raw = path.read_bytes()
    assert raw[:4] == b"MAP1"
    assert len(raw) == 16 + 2 * 2 * 10 + 2 * 2 * 2


if __name__ == "__main__":
    test_parse_matrix_sorts_and_validates()
    test_compare_tiles_reports_4x4_blocks()
    test_write_ottdmap_preserves_dense_planes()
    print("OK: random_map_parity tests")
