#!/usr/bin/env python3
"""Pruebas sin binarios externos para `generation_phase_parity.py`."""

from __future__ import annotations

import tempfile
import copy
import json
import sys
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))
import generation_phase_parity as phase  # noqa: E402


def test_state_gate_rejects_rng_or_town_divergence_with_identical_tiles() -> None:
    reference = {"random_state_0": 10, "random_state_1": 20, "town_count": 2,
                 "town_positions": [{"id": 0, "x": 2, "y": 3, "population": 40, "num_houses": 3},
                                    {"id": 1, "x": 4, "y": 5, "population": 50, "num_houses": 4}]}
    tiles = {"exact_match": True, "tile_difference_count": 0}
    assert phase.include_generation_state(tiles, reference, reference)["exact_match"]
    for key in ("random_state_0", "random_state_1", "id", "x", "y", "population", "num_houses", "order", "count"):
        candidate = copy.deepcopy(reference)
        if key.startswith("random"):
            candidate[key] += 1
        elif key == "order":
            candidate["town_positions"].reverse()
        elif key == "count":
            candidate["town_positions"].pop()
            candidate["town_count"] -= 1
        else:
            candidate["town_positions"][0][key] += 10
        result = phase.include_generation_state(tiles, reference, candidate)
        assert result["tiles_exact_match"] and not result["exact_match"], key
        assert phase.first_divergent_stage({"towns": result}) == "towns"
        if key in ("population", "num_houses"):
            first = result["generation_state"]["first_town_difference"]
            assert first["index"] == 0
            assert first["reference"][key] + 10 == first["candidate"][key]
    assert not phase.include_generation_state({"exact_match": False}, reference, reference)["exact_match"]


def test_state_gate_fails_closed_for_unobserved_or_malformed_state() -> None:
    valid = {"random_state_0": 0, "random_state_1": 1, "town_count": 0, "town_positions": []}
    invalid = [{}, {**valid, "random_state_0": None}, {**valid, "random_state_1": True},
               {**valid, "town_count": 1}, {**valid, "town_positions": None},
               {**valid, "town_count": 1, "town_positions": [{"id": 0, "x": -1, "y": 0, "population": 0, "num_houses": 0}]},
               {**valid, "town_count": 2, "town_positions": [{"id": 0, "x": 1, "y": 0, "population": 0, "num_houses": 0}] * 2}]
    for bad in invalid:
        for reference, candidate in ((bad, valid), (valid, bad)):
            try:
                phase.compare_generation_state(reference, candidate)
            except phase.GenerationPhaseError:
                continue
            raise AssertionError(f"el estado inválido debería fallar: {bad}")


def test_state_gate_rejects_missing_demographics_even_when_both_sides_omit_them() -> None:
    town = {"id": 0, "x": 2, "y": 3, "population": 40, "num_houses": 3}
    for key in ("population", "num_houses"):
        for value in (None, -1, True, 1 << 32):
            bad = {"random_state_0": 0, "random_state_1": 1, "town_count": 1,
                   "town_positions": [{**town, key: value}]}
            try:
                phase.compare_generation_state(bad, bad)
            except phase.GenerationPhaseError:
                continue
            raise AssertionError(f"demografía inválida debería fallar: {key}={value}")
        old = {k: v for k, v in town.items() if k != key}
        bad = {"random_state_0": 0, "random_state_1": 1, "town_count": 1, "town_positions": [old]}
        try:
            phase.compare_generation_state(bad, bad)
        except phase.GenerationPhaseError:
            continue
        raise AssertionError(f"el exportador debe observar {key}")


def test_versioned_oracle_exports_generation_state() -> None:
    source = (phase.ROOT / "patches/openttd-15.3-snapshot-export/src/snapshot_export.cpp").read_text()
    stage = source.split("void OpenttdrsMaybeCaptureGenerationStage(const char *stage)", 1)[1]
    stage = stage.split("void OpenttdrsTraceTreePlacement", 1)[0]
    assert '#include "town.h"' in source
    for statement in (
        'metadata["random_state_0"] = _random.state[0];',
        'metadata["random_state_1"] = _random.state[1];',
        'metadata["town_count"] = Town::GetNumItems();',
        'metadata["town_positions"] = town_positions;',
        'Town::Iterate()', 'town->index.base()', 'TileX(town->xy)', 'TileY(town->xy)',
        'town->cache.population', 'town->cache.num_houses',
    ):
        assert statement in stage, statement


def test_parse_phases_rejects_reordered_or_unknown_values() -> None:
    assert phase.parse_phases("landscape,clear,objects") == ("landscape", "clear", "objects")
    for value in ("clear,landscape", "clear,clear", "clear,rivers"):
        try:
            phase.parse_phases(value)
        except phase.GenerationPhaseError:
            continue
        raise AssertionError(f"{value!r} debería fallar")


def test_first_divergent_stage_uses_pipeline_order() -> None:
    comparisons = {
        "towns": {"exact_match": False},
        "objects": {"exact_match": False},
        "landscape": {"exact_match": True},
        "clear": {"exact_match": True},
    }
    assert phase.first_divergent_stage(comparisons) == "towns"
    assert phase.first_divergent_stage({"clear": {"exact_match": True}}) is None


def test_climate_codes_cover_all_openttd_landscapes() -> None:
    assert phase.CLIMATE_CODES == {
        "temperate": 0,
        "arctic": 1,
        "tropic": 2,
        "toyland": 3,
    }


def test_river_settings_are_written_for_non_default_oracle_runs() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "openttd.cfg"
        phase.matrix.write_config(
            path,
            64,
            amount_of_rivers=3,
            min_river_length=32,
            river_route_random=17,
            water_borders=0,
        )
        text = path.read_text(encoding="utf-8")
    assert "amount_of_rivers = 3\n" in text
    assert "min_river_length = 32\n" in text
    assert "river_route_random = 17\n" in text
    assert "water_borders = 0\n" in text


def test_river_settings_reject_values_outside_openttd_ranges() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "openttd.cfg"
        for kwargs in (
            {"amount_of_rivers": 4},
            {"min_river_length": 1},
            {"river_route_random": 0},
            {"water_borders": 17},
        ):
            try:
                phase.matrix.write_config(path, 64, **kwargs)
            except phase.matrix.MatrixError:
                continue
            raise AssertionError(f"la configuración {kwargs} debería fallar")


def test_rmap_145_toyland_512_evidence_keeps_its_exact_and_limited_scope() -> None:
    """La cohorte publicada no debe perder fases ni convertirse en una afirmación global."""
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-145.json").read_text(encoding="utf-8")
    )
    assert evidence["contract"] == "RMAP-145 Toyland 512x512 generation phases"
    assert evidence["scope"] == {
        "size": 512,
        "seed": 1330935381,
        "climate": "toyland",
        "generation_settings": {
            "amount_of_rivers": None,
            "min_river_length": None,
            "river_route_random": None,
            "water_borders": None,
        },
        "phases": ["landscape", "clear", "towns", "industries", "objects", "trees"],
    }
    comparison = evidence["comparison"]
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_size"] == 4
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    assert comparison["raw_tile_fields"] == list(phase.matrix.RAW_FIELDS)
    assert comparison["generation_state_fields"] == [
        "random_state_0",
        "random_state_1",
        "town_count",
        "town_positions[id,x,y,population,num_houses]",
    ]
    results = evidence["phase_results"]
    assert [result["phase"] for result in results] == evidence["scope"]["phases"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert evidence["town_summary_at_trees"] == {
        "count": 85,
        "total_population": 53778,
        "total_houses": 1955,
        "ordered_sequence_sha256": "4d58dd60305087f4251e93b18f7f082ae2e8b770e02d31bdbf3b5547df1b5ecf",
    }
    assert evidence["not_observed"] == [
        "industry pool identities and fields",
        "object pool identities and fields",
        "startup and subsequent simulation ticks",
    ]


if __name__ == "__main__":
    test_state_gate_rejects_rng_or_town_divergence_with_identical_tiles()
    test_state_gate_fails_closed_for_unobserved_or_malformed_state()
    test_versioned_oracle_exports_generation_state()
    test_state_gate_rejects_missing_demographics_even_when_both_sides_omit_them()
    test_parse_phases_rejects_reordered_or_unknown_values()
    test_first_divergent_stage_uses_pipeline_order()
    test_river_settings_are_written_for_non_default_oracle_runs()
    test_river_settings_reject_values_outside_openttd_ranges()
    test_rmap_145_toyland_512_evidence_keeps_its_exact_and_limited_scope()
    print("OK: generation_phase_parity tests")
