#!/usr/bin/env python3
"""Pruebas puras del comparador de la fase ``GenerateTrees``."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

import tree_phase_parity as tree_phase


def trace(metadata_extra: dict[str, object], placements: list[dict[str, object]]) -> str:
    metadata = {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "tree-generation-trace",
        "producer": "test",
        "trace": "tree_placements",
        "stage": "GenerateTrees",
        "climate": 0,
        "random_state": [1, 2],
        "width": 64,
        "height": 64,
        **metadata_extra,
    }
    return "\n".join(json.dumps(row) for row in [metadata, *placements]) + "\n"


def placement(ordinal: int, random: int = 7) -> dict[str, object]:
    return {
        "kind": "tree_placement",
        "ordinal": ordinal,
        "origin": "same_height" if ordinal else "random",
        "x": 3 + ordinal,
        "y": 4,
        "random": random,
        "parent": {"x": 3, "y": 4} if ordinal else None,
    }


def test_exact_trace_compares_without_producer_path_noise() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        reference = root / "reference.jsonl"
        candidate = root / "candidate.jsonl"
        rows = [placement(0), placement(1, 11)]
        reference.write_text(trace({"producer": "openttd", "source_path": "reference"}, rows), encoding="utf-8")
        candidate.write_text(trace({"producer": "openttdrs", "source_path": "candidate"}, rows), encoding="utf-8")
        comparison = tree_phase.compare_tree_traces(
            tree_phase.read_tree_trace(reference), tree_phase.read_tree_trace(candidate)
        )
    assert comparison["exact_match"]
    assert comparison["candidate_placement_count"] == 2


def test_trace_reports_first_placement_field() -> None:
    reference = ({"schema_version": 1, "contract": "tree-generation-trace", "trace": "tree_placements", "stage": "GenerateTrees", "climate": 0, "random_state": [1, 2], "width": 64, "height": 64}, [placement(0), placement(1)])
    candidate = ({**reference[0]}, [placement(0), placement(1, 9)])
    comparison = tree_phase.compare_tree_traces(reference, candidate)
    assert not comparison["exact_match"]
    assert comparison["first_difference"]["ordinal"] == 1
    assert comparison["first_difference"]["fields"] == ["random"]


def test_climate_names_match_openttd_landscape_ids() -> None:
    assert tree_phase.CLIMATES == {
        "temperate": 0,
        "arctic": 1,
        "tropic": 2,
        "toyland": 3,
    }


if __name__ == "__main__":
    test_exact_trace_compares_without_producer_path_noise()
    test_trace_reports_first_placement_field()
    test_climate_names_match_openttd_landscape_ids()
    print("OK: tree_phase_parity tests")
