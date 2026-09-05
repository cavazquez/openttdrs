#!/usr/bin/env python3
"""Pruebas puras del harness de mapas aleatorios."""

from __future__ import annotations

import tempfile
import hashlib
import subprocess
from pathlib import Path
from unittest.mock import patch

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


def test_write_config_accepts_each_openttd_landscape() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "openttd.cfg"
        matrix.write_config(path, 64, climate=1)
        assert "landscape = 1\n" in path.read_text(encoding="utf-8")
        try:
            matrix.write_config(path, 64, climate=4)
        except matrix.MatrixError:
            pass
        else:
            raise AssertionError("un landscape fuera de 0..3 debe rechazarse")


def test_existing_candidate_is_rebuilt_and_failed_build_is_not_reused() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        binary = root / "target/debug/world_raw_dumper"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"old binary")
        with patch.object(matrix, "ROOT", root), patch.object(matrix.subprocess, "run") as build:
            assert matrix.ensure_candidate_binary(None) == binary
            build.assert_called_once()
            assert "--locked" in build.call_args.args[0]
            assert build.call_args.kwargs["check"]
            build.side_effect = subprocess.CalledProcessError(101, "cargo")
            try:
                matrix.ensure_candidate_binary(None)
            except subprocess.CalledProcessError:
                pass
            else:
                raise AssertionError("a failed build must not reuse the old binary")


def test_explicit_candidate_is_not_replaced_or_attributed_to_local_source() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        binary = Path(tmp) / "external"
        binary.write_bytes(b"external binary")
        with patch.object(matrix.subprocess, "run") as build:
            assert matrix.ensure_candidate_binary(binary) == binary
            build.assert_not_called()
        provenance = matrix.candidate_provenance(binary, managed=False)
        assert provenance["build"] == "external"
        assert provenance["binary_sha256"] == hashlib.sha256(b"external binary").hexdigest()
        assert "source_commit" not in provenance


def test_managed_candidate_records_revision_and_dirty_source() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        binary = Path(tmp) / "candidate"
        binary.write_bytes(b"fresh binary")
        with patch.object(matrix.subprocess, "check_output", side_effect=["abc123\n", " M src/map.rs\n"]):
            provenance = matrix.candidate_provenance(binary, managed=True)
        assert provenance["source_commit"] == "abc123"
        assert provenance["source_tracked_changes"] == [" M src/map.rs"]
        assert provenance["build"] == "cargo-build-locked"


if __name__ == "__main__":
    test_parse_matrix_sorts_and_validates()
    test_compare_tiles_reports_4x4_blocks()
    test_write_ottdmap_preserves_dense_planes()
    test_write_config_accepts_each_openttd_landscape()
    test_existing_candidate_is_rebuilt_and_failed_build_is_not_reused()
    test_explicit_candidate_is_not_replaced_or_attributed_to_local_source()
    test_managed_candidate_records_revision_and_dirty_source()
    print("OK: random_map_parity tests")
