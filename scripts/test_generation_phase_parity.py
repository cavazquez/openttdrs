#!/usr/bin/env python3
"""Pruebas sin binarios externos para `generation_phase_parity.py`."""

from __future__ import annotations

import tempfile
import sys
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))
import generation_phase_parity as phase  # noqa: E402


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


if __name__ == "__main__":
    test_parse_phases_rejects_reordered_or_unknown_values()
    test_first_divergent_stage_uses_pipeline_order()
    test_river_settings_are_written_for_non_default_oracle_runs()
    test_river_settings_reject_values_outside_openttd_ranges()
    print("OK: generation_phase_parity tests")
