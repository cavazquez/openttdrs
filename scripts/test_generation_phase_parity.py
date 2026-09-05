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


def valid_generation_state() -> dict[str, object]:
    """Metadata v5 mínima, completa y ordenada para mutar sin un oráculo externo."""
    return {
        "random_state_0": 10,
        "random_state_1": 20,
        "town_count": 2,
        "town_positions": [
            {"id": 0, "x": 2, "y": 3, "population": 40, "num_houses": 3},
            {"id": 1, "x": 4, "y": 5, "population": 50, "num_houses": 4},
        ],
        "industry_count": 2,
        "industry_positions": [
            {
                "id": 3,
                "type": 6,
                "x": 7,
                "y": 8,
                "selected_layout": 1,
                "random": 0xBEEF,
                "random_colour": 5,
                "counter": 0x345,
                "prod_level": 16,
                "town_id": 0,
            },
            {
                "id": 4,
                "type": 9,
                "x": 9,
                "y": 10,
                "selected_layout": 2,
                "random": 0xCAFE,
                "random_colour": 7,
                "counter": 0x456,
                "prod_level": 24,
                "town_id": 0xFFFFFFFF,
            },
        ],
        "object_count": 2,
        "object_positions": [
            {"id": 5, "type": 0, "x": 11, "y": 12, "width": 1, "height": 1, "view": 0},
            {"id": 6, "type": 1, "x": 13, "y": 14, "width": 2, "height": 1, "view": 1},
        ],
    }


def test_state_gate_rejects_rng_or_town_divergence_with_identical_tiles() -> None:
    reference = valid_generation_state()
    tiles = {"exact_match": True, "tile_difference_count": 0}
    assert phase.include_generation_state(tiles, reference, reference)["exact_match"]
    mutations = (
        ("random_state_0", "random_state_0", None, None),
        ("random_state_1", "random_state_1", None, None),
        ("town population", "town_positions", 0, "population"),
        ("industry type", "industry_positions", 0, "type"),
        ("industry layout", "industry_positions", 0, "selected_layout"),
        ("industry origin", "industry_positions", 0, "x"),
        ("industry random", "industry_positions", 0, "random"),
        ("industry colour", "industry_positions", 0, "random_colour"),
        ("industry counter", "industry_positions", 0, "counter"),
        ("industry production level", "industry_positions", 0, "prod_level"),
        ("industry town", "industry_positions", 0, "town_id"),
        ("object type", "object_positions", 0, "type"),
        ("object footprint", "object_positions", 0, "width"),
        ("object view", "object_positions", 0, "view"),
    )
    for name, collection, index, field in mutations:
        candidate = copy.deepcopy(reference)
        if index is None:
            candidate[collection] += 1
        else:
            candidate[collection][index][field] += 10
        result = phase.include_generation_state(tiles, reference, candidate)
        assert result["tiles_exact_match"] and not result["exact_match"], name
        assert phase.first_divergent_stage({"towns": result}) == "towns"
        state = result["generation_state"]
        if collection == "town_positions":
            first = state["first_town_difference"]
        elif collection == "industry_positions":
            first = state["first_industry_difference"]
        elif collection == "object_positions":
            first = state["first_object_difference"]
        else:
            continue
        assert first["index"] == 0
        assert first["reference"][field] + 10 == first["candidate"][field]
    for positions in ("town_positions", "industry_positions", "object_positions"):
        candidate = copy.deepcopy(reference)
        candidate[positions].reverse()
        try:
            phase.compare_generation_state(reference, candidate)
        except phase.GenerationPhaseError:
            continue
        raise AssertionError(f"{positions} desordenado debería fallar cerrado")
    for count, positions in (
        ("town_count", "town_positions"),
        ("industry_count", "industry_positions"),
        ("object_count", "object_positions"),
    ):
        candidate = copy.deepcopy(reference)
        candidate[positions].pop()
        candidate[count] -= 1
        result = phase.include_generation_state(tiles, reference, candidate)
        assert not result["exact_match"], positions
    assert not phase.include_generation_state({"exact_match": False}, reference, reference)["exact_match"]


def test_state_gate_fails_closed_for_unobserved_or_malformed_state() -> None:
    valid = valid_generation_state()
    invalid = [
        {},
        {**valid, "random_state_0": None},
        {**valid, "random_state_1": True},
        {**valid, "town_count": 1},
        {**valid, "town_positions": None},
        {
            **valid,
            "town_count": 1,
            "town_positions": [
                {"id": 0, "x": -1, "y": 0, "population": 0, "num_houses": 0}
            ],
        },
        {
            **valid,
            "town_count": 2,
            "town_positions": [
                {"id": 0, "x": 1, "y": 0, "population": 0, "num_houses": 0}
            ]
            * 2,
        },
    ]
    for count, positions in (
        ("industry_count", "industry_positions"),
        ("object_count", "object_positions"),
    ):
        bad = copy.deepcopy(valid)
        bad[positions][1]["id"] = bad[positions][0]["id"]
        invalid.append(bad)
        bad = copy.deepcopy(valid)
        bad[positions][0]["type"] = None
        invalid.append(bad)
        bad = copy.deepcopy(valid)
        bad[count] -= 1
        invalid.append(bad)
    for field in ("random", "random_colour", "counter", "prod_level", "town_id"):
        bad = copy.deepcopy(valid)
        bad["industry_positions"][0][field] = None
        invalid.append(bad)
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
            bad = valid_generation_state()
            bad["town_count"] = 1
            bad["town_positions"] = [{**town, key: value}]
            try:
                phase.compare_generation_state(bad, bad)
            except phase.GenerationPhaseError:
                continue
            raise AssertionError(f"demografía inválida debería fallar: {key}={value}")
        old = {k: v for k, v in town.items() if k != key}
        bad = valid_generation_state()
        bad["town_count"] = 1
        bad["town_positions"] = [old]
        try:
            phase.compare_generation_state(bad, bad)
        except phase.GenerationPhaseError:
            continue
        raise AssertionError(f"el exportador debe observar {key}")


def test_versioned_oracle_exports_generation_state() -> None:
    source = (phase.ROOT / "patches/openttd-15.3-snapshot-export/src/snapshot_export.cpp").read_text()
    stage = source.split("void OpenttdrsMaybeCaptureGenerationStage(const char *stage)", 1)[1]
    stage = stage.split("void OpenttdrsTraceTreePlacement", 1)[0]
    for include in ('#include "town.h"', '#include "industry.h"', '#include "object_base.h"'):
        assert include in source
    for statement in (
        'metadata["random_state_0"] = _random.state[0];',
        'metadata["random_state_1"] = _random.state[1];',
        'metadata["town_count"] = Town::GetNumItems();',
        'metadata["town_positions"] = town_positions;',
        'Town::Iterate()', 'town->index.base()', 'TileX(town->xy)', 'TileY(town->xy)',
        'town->cache.population', 'town->cache.num_houses',
        'metadata["industry_count"] = Industry::GetNumItems();',
        'metadata["industry_positions"] = industry_positions;',
        'Industry::Iterate()', 'industry->index.base()', 'industry->type',
        'industry->location.tile', 'industry->selected_layout', 'industry->random',
        'industry->random_colour', 'industry->counter', 'industry->prod_level',
        'industry->town == nullptr', 'industry->town->index.base()',
        'metadata["object_count"] = Object::GetNumItems();',
        'metadata["object_positions"] = object_positions;',
        'Object::Iterate()', 'object->index.base()', 'object->type',
        'object->location.tile', 'object->location.w', 'object->location.h', 'object->view',
    ):
        assert statement in stage, statement


def test_unpinned_integration_synchronizes_the_versioned_snapshot_exporter() -> None:
    """Un fork instrumentado no puede conservar en silencio un exportador viejo."""
    source = (
        phase.ROOT / "patches/openttd-15.3-snapshot-export/integrate.sh"
    ).read_text(encoding="utf-8")
    for statement in (
        'python3 - "$DEST" "$MODE" "$PATCH_DIR" <<\'PY\'',
        "patch_dir = Path(sys.argv[3])",
        'for name in ("snapshot_export.cpp", "snapshot_export.h"):',
        "source = patch_dir / \"src\" / name",
        "target.write_bytes(source.read_bytes())",
    ):
        assert statement in source, statement


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


def test_rmap_147_evidence_records_ordered_industry_and_object_pools() -> None:
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-147.json").read_text(encoding="utf-8")
    )
    assert evidence["issue"] == 362
    assert evidence["contract"] == "RMAP-147 ordered industry and object pools at generation boundaries"
    assert evidence["scope"]["size"] == 512
    assert evidence["scope"]["seed"] == 1330935378
    assert evidence["scope"]["climate"] == "temperate"
    comparison = evidence["comparison"]
    assert comparison["report_schema_version"] == 4
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_size"] == 4
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    assert comparison["generation_state_fields"] == [
        "random_state_0",
        "random_state_1",
        "town_count",
        "town_positions[id,x,y,population,num_houses]",
        "industry_count",
        "industry_positions[id,type,x,y,selected_layout]",
        "object_count",
        "object_positions[id,type,x,y,width,height,view]",
    ]
    results = evidence["phase_results"]
    assert [result["phase"] for result in results] == evidence["scope"]["phases"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert [(result["industry_count"], result["object_count"]) for result in results] == [
        (0, 0),
        (0, 0),
        (0, 0),
        (213, 0),
        (213, 65),
        (213, 65),
    ]
    assert set(evidence["ordered_sequence_sha256"]) == {
        "towns_at_towns",
        "industries_at_industries",
        "objects_at_objects",
    }
    assert evidence["not_observed"] == [
        "industry fields outside identity, type, origin and selected_layout",
        "object fields outside identity, type, origin, footprint and view",
        "industry placement attempt traces",
        "startup and subsequent simulation ticks",
    ]


def test_rmap_148_evidence_extends_ordered_pools_to_tropic_rivers() -> None:
    """La cohorte publicada conserva configuración explícita y límites honestos."""
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-148.json").read_text(encoding="utf-8")
    )
    assert evidence["issue"] == 363
    assert evidence["contract"] == "RMAP-148 Tropic river ordered entity pools at generation boundaries"
    assert evidence["scope"] == {
        "size": 512,
        "seed": 1330935380,
        "climate": "tropic",
        "generation_settings": {
            "amount_of_rivers": 1,
            "min_river_length": 2,
            "river_route_random": 1,
            "water_borders": 0,
        },
        "phases": ["landscape", "clear", "towns", "industries", "objects", "trees"],
    }
    comparison = evidence["comparison"]
    assert comparison["report_schema_version"] == 4
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_size"] == 4
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    assert comparison["generation_state_fields"] == [
        "random_state_0",
        "random_state_1",
        "town_count",
        "town_positions[id,x,y,population,num_houses]",
        "industry_count",
        "industry_positions[id,type,x,y,selected_layout]",
        "object_count",
        "object_positions[id,type,x,y,width,height,view]",
    ]
    results = evidence["phase_results"]
    assert [result["phase"] for result in results] == evidence["scope"]["phases"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert [(result["town_count"], result["industry_count"], result["object_count"])
            for result in results] == [
        (0, 0, 0),
        (0, 0, 0),
        (98, 0, 0),
        (98, 213, 0),
        (98, 213, 60),
        (98, 213, 60),
    ]
    assert set(evidence["ordered_sequence_sha256"]) == {
        "towns_at_towns",
        "industries_at_industries",
        "objects_at_objects",
    }
    assert evidence["not_observed"] == [
        "industry fields outside identity, type, origin and selected_layout",
        "object fields outside identity, type, origin, footprint and view",
        "industry placement attempt traces and aquatic industries",
        "startup and subsequent simulation ticks",
        "other seeds, sizes, climates and generation setting combinations",
    ]


def test_rmap_149_evidence_extends_ordered_pools_to_arctic_rivers() -> None:
    """La evidencia Arctic conserva conteos, settings y cobertura v4 explícitos."""
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-149.json").read_text(encoding="utf-8")
    )
    assert evidence["issue"] == 364
    assert evidence["contract"] == "RMAP-149 Arctic river ordered entity pools at generation boundaries"
    assert evidence["scope"] == {
        "size": 512,
        "seed": 1330935379,
        "climate": "arctic",
        "generation_settings": {
            "amount_of_rivers": 1,
            "min_river_length": 2,
            "river_route_random": 1,
            "water_borders": 0,
        },
        "phases": ["landscape", "clear", "towns", "industries", "objects", "trees"],
    }
    comparison = evidence["comparison"]
    assert comparison["report_schema_version"] == 4
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    results = evidence["phase_results"]
    assert [result["phase"] for result in results] == evidence["scope"]["phases"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert [(result["town_count"], result["industry_count"], result["object_count"])
            for result in results] == [
        (0, 0, 0),
        (0, 0, 0),
        (96, 0, 0),
        (96, 217, 0),
        (96, 217, 61),
        (96, 217, 61),
    ]
    assert set(evidence["ordered_sequence_sha256"]) == {
        "towns_at_towns",
        "industries_at_industries",
        "objects_at_objects",
    }
    assert evidence["not_observed"][-1] == "other seeds, sizes, climates and generation setting combinations"


def test_rmap_150_evidence_records_toyland_empty_object_pool() -> None:
    """Un pool vacío sigue siendo estado observado, no cobertura de objetos reales."""
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-150.json").read_text(encoding="utf-8")
    )
    assert evidence["issue"] == 365
    assert evidence["contract"] == "RMAP-150 Toyland ordered entity pools at generation boundaries"
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
    assert comparison["report_schema_version"] == 4
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    results = evidence["phase_results"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert [(result["town_count"], result["industry_count"], result["object_count"])
            for result in results] == [
        (0, 0, 0),
        (0, 0, 0),
        (85, 0, 0),
        (85, 203, 0),
        (85, 203, 0),
        (85, 203, 0),
    ]
    assert set(evidence["ordered_sequence_sha256"]) == {
        "towns_at_towns",
        "industries_at_industries",
        "objects_at_objects",
    }
    assert evidence["not_observed"] == [
        "industry fields outside identity, type, origin and selected_layout",
        "object fields outside identity, type, origin, footprint and view",
        "non-empty Toyland object pool coverage",
        "industry placement attempt traces and aquatic industries",
        "startup and subsequent simulation ticks",
        "other seeds, sizes, climates and generation setting combinations",
    ]


def test_rmap_151_evidence_records_industry_constructor_state() -> None:
    """El gate v5 fija el estado constructor real, no sólo la identidad del pool."""
    evidence = json.loads(
        (phase.ROOT / "docs/parity/evidence/rmap-151.json").read_text(encoding="utf-8")
    )
    assert evidence["issue"] == 366
    assert evidence["contract"] == "RMAP-151 industry constructor state at generation boundaries"
    assert evidence["scope"] == {
        "size": 512,
        "seed": 1330935378,
        "climate": "temperate",
        "generation_settings": {
            "amount_of_rivers": None,
            "min_river_length": None,
            "river_route_random": None,
            "water_borders": None,
        },
        "phases": ["landscape", "clear", "towns", "industries", "objects", "trees"],
    }
    comparison = evidence["comparison"]
    assert comparison["report_schema_version"] == 5
    assert comparison["all_exact"] and comparison["first_divergent_stage"] is None
    assert comparison["block_size"] == 4
    assert comparison["block_grid"] == {"width": 128, "height": 128, "count": 16384}
    assert comparison["generation_state_fields"] == [
        "random_state_0",
        "random_state_1",
        "town_count",
        "town_positions[id,x,y,population,num_houses]",
        "industry_count",
        "industry_positions[id,type,x,y,selected_layout,random,random_colour,counter,prod_level,town_id]",
        "object_count",
        "object_positions[id,type,x,y,width,height,view]",
    ]
    results = evidence["phase_results"]
    assert [result["phase"] for result in results] == evidence["scope"]["phases"]
    assert all(
        result["tile_difference_count"] == 0 and result["changed_block_count"] == 0
        for result in results
    )
    assert [(result["town_count"], result["industry_count"], result["object_count"])
            for result in results] == [
        (0, 0, 0),
        (0, 0, 0),
        (96, 0, 0),
        (96, 213, 0),
        (96, 213, 65),
        (96, 213, 65),
    ]
    assert set(evidence["ordered_sequence_sha256"]) == {
        "towns_at_towns",
        "industries_at_industries",
        "objects_at_objects",
    }
    assert evidence["not_observed"] == [
        "industry fields outside identity, constructor random/colour/counter/level/town and selected_layout",
        "object fields outside identity, type, origin, footprint and view",
        "industry placement attempt traces",
        "aquatic industries including IT_OIL_RIG",
        "startup and subsequent simulation ticks",
        "other seeds, sizes, climates and generation setting combinations",
    ]


if __name__ == "__main__":
    test_state_gate_rejects_rng_or_town_divergence_with_identical_tiles()
    test_state_gate_fails_closed_for_unobserved_or_malformed_state()
    test_versioned_oracle_exports_generation_state()
    test_unpinned_integration_synchronizes_the_versioned_snapshot_exporter()
    test_state_gate_rejects_missing_demographics_even_when_both_sides_omit_them()
    test_parse_phases_rejects_reordered_or_unknown_values()
    test_first_divergent_stage_uses_pipeline_order()
    test_river_settings_are_written_for_non_default_oracle_runs()
    test_river_settings_reject_values_outside_openttd_ranges()
    test_rmap_145_toyland_512_evidence_keeps_its_exact_and_limited_scope()
    test_rmap_147_evidence_records_ordered_industry_and_object_pools()
    test_rmap_148_evidence_extends_ordered_pools_to_tropic_rivers()
    test_rmap_149_evidence_extends_ordered_pools_to_arctic_rivers()
    test_rmap_150_evidence_records_toyland_empty_object_pool()
    test_rmap_151_evidence_records_industry_constructor_state()
    print("OK: generation_phase_parity tests")
