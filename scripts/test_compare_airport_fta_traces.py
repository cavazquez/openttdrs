#!/usr/bin/env python3
"""Regresión del contrato físico del comparador Airport FTA."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from compare_airport_fta_traces import AIRCRAFT_FIELDS, comparable_aircraft


def aircraft(**overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "pos": 11,
        "previous_pos": 17,
        "state": 13,
        "targetairport": 1,
        "speed": 320,
        "progress": 195,
        "subspeed": 0,
        "direction": 2,
        "x_pos": 696,
        "y_pos": 744,
        "z_pos": 117,
        "running": True,
    }
    value.update(overrides)
    return value


def main() -> int:
    reference = {"aircraft": [aircraft()]}
    expected = [
        tuple(reference["aircraft"][0][field] for field in AIRCRAFT_FIELDS)
    ]
    if comparable_aircraft(reference) != expected:
        print("FAIL: el contrato no incluye todos los campos físicos", file=sys.stderr)
        return 1

    moved = {"aircraft": [aircraft(x_pos=697)]}
    raised = {"aircraft": [aircraft(z_pos=118)]}
    if comparable_aircraft(reference) == comparable_aircraft(moved):
        print("FAIL: un cambio de x_pos pasó inadvertido", file=sys.stderr)
        return 1
    if comparable_aircraft(reference) == comparable_aircraft(raised):
        print("FAIL: un cambio de z_pos pasó inadvertido", file=sys.stderr)
        return 1

    print("OK: el comparador Airport FTA detecta cambios de posición física")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
