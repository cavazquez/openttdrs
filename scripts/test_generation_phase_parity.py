#!/usr/bin/env python3
"""Pruebas sin binarios externos para `generation_phase_parity.py`."""

from __future__ import annotations

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


if __name__ == "__main__":
    test_parse_phases_rejects_reordered_or_unknown_values()
    test_first_divergent_stage_uses_pipeline_order()
    print("OK: generation_phase_parity tests")
