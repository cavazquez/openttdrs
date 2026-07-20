#!/usr/bin/env python3
"""Compara el contrato PBS v1/v2 de OpenTTD y openttdrs por primera divergencia."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def read_trace(path: Path) -> tuple[dict[str, object], list[dict[str, object]]]:
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer {path}: {exc}")
    if not rows or rows[0].get("kind") != "metadata":
        fail(f"{path}: falta metadata")
    metadata = rows[0]
    if metadata.get("schema_version") not in {1, 2}:
        fail(f"{path}: schema PBS no soportado")
    samples = rows[1:]
    if any(row.get("kind") not in {"initial", "tick"} for row in samples):
        fail(f"{path}: fila PBS inválida")
    return metadata, samples


def comparable_trains(row: dict[str, object]) -> list[tuple[int, int, int, int, int, int]]:
    trains = row.get("trains", [])
    assert isinstance(trains, list)
    # Los IDs de pool no son comparables entre motores; el estado cinemático sí.
    return sorted(
        (
            train["x"],
            train["y"],
            train["progress"],
            train["speed"],
            train["subspeed"],
            train["direction"],
        )
        for train in trains
    )


def comparable_units(row: dict[str, object]) -> list[list[tuple[int, int, int, int, int]]] | None:
    """Unidades por tren (ordenadas). None si el oráculo no declara units (v1)."""
    trains = row.get("trains", [])
    assert isinstance(trains, list)
    if not any(isinstance(train, dict) and "units" in train for train in trains):
        return None
    per_train: list[list[tuple[int, int, int, int, int]]] = []
    for train in trains:
        assert isinstance(train, dict)
        units = train.get("units", [])
        assert isinstance(units, list)
        per_train.append(
            sorted(
                (
                    unit["index"],
                    unit["x"],
                    unit["y"],
                    unit["rail_pixel"],
                    unit["direction"],
                )
                for unit in units
            )
        )
    per_train.sort()
    return per_train


def comparable_reservations(row: dict[str, object]) -> list[tuple[int, int, int]]:
    reservations = row.get("rail_reservations", [])
    assert isinstance(reservations, list)
    return sorted((entry["x"], entry["y"], entry["track_bits"]) for entry in reservations)


def main() -> None:
    if len(sys.argv) != 3:
        fail("uso: compare_pbs_traces.py <openttd.jsonl> <openttdrs.jsonl>")
    expected_path, actual_path = map(Path, sys.argv[1:])
    expected_meta, expected = read_trace(expected_path)
    actual_meta, actual = read_trace(actual_path)
    if expected_meta.get("producer") != "openttd":
        fail("la primera traza debe tener producer=openttd")
    if actual_meta.get("producer") != "openttdrs":
        fail("la segunda traza debe tener producer=openttdrs")
    if len(expected) != len(actual):
        fail(f"cantidad de ticks: OpenTTD={len(expected)} openttdrs={len(actual)}")

    for frame, (expected_row, actual_row) in enumerate(zip(expected, actual, strict=True)):
        expected_tick = expected_row.get("tick")
        actual_tick = actual_row.get("tick")
        if expected_row.get("kind") != actual_row.get("kind"):
            fail(
                f"muestra {frame}: kind OpenTTD={expected_row.get('kind')} "
                f"openttdrs={actual_row.get('kind')}"
            )
        frame_label = (
            f"muestra {frame} {expected_row.get('kind')} "
            f"(tick OpenTTD={expected_tick}, openttdrs={actual_tick})"
        )
        expected_trains = comparable_trains(expected_row)
        actual_trains = comparable_trains(actual_row)
        if expected_trains != actual_trains:
            fail(f"{frame_label}: trenes OpenTTD={expected_trains} openttdrs={actual_trains}")
        expected_units = comparable_units(expected_row)
        if expected_units is not None:
            actual_units = comparable_units(actual_row)
            if actual_units is None:
                fail(f"{frame_label}: el candidato no declara units[] (schema v2)")
            if expected_units != actual_units:
                fail(f"{frame_label}: units OpenTTD={expected_units} openttdrs={actual_units}")
        expected_reservations = comparable_reservations(expected_row)
        actual_reservations = comparable_reservations(actual_row)
        if expected_reservations != actual_reservations:
            fail(
                f"{frame_label}: reservas OpenTTD={expected_reservations} "
                f"openttdrs={actual_reservations}"
            )

    print(f"OK: PBS externo sin divergencias ({len(expected)} ticks)")


if __name__ == "__main__":
    main()
