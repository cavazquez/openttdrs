#!/usr/bin/env python3
"""Valida el contrato JSONL de la traza PBS emitida por OpenTTD parcheado."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def main() -> None:
    if len(sys.argv) not in (3, 4):
        fail("uso: validate_pbs_trace.py <traza.jsonl> <ticks esperados> [openttd|openttdrs]")
    path = Path(sys.argv[1])
    try:
        expected_ticks = int(sys.argv[2])
    except ValueError:
        fail(f"ticks inválidos: {sys.argv[2]}")
    if expected_ticks <= 0:
        fail("ticks esperados debe ser positivo")
    expected_producer = sys.argv[3] if len(sys.argv) == 4 else None

    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer JSONL {path}: {exc}")
    if not rows:
        fail("traza vacía")

    metadata, *samples = rows
    if metadata.get("kind") != "metadata":
        fail("primera fila debe ser metadata")
    producer = metadata.get("producer")
    if producer not in {"openttd", "openttdrs"}:
        fail("metadata producer debe ser openttd u openttdrs")
    if expected_producer is not None and producer != expected_producer:
        fail(f"metadata producer debe ser {expected_producer}")
    if metadata.get("schema_version") not in {1, 2}:
        fail("schema_version PBS no soportada")
    allowed_tick_points = {"after_state_game_loop", "after_game_state_step"}
    tick_point = metadata.get("tick_sample_point", metadata.get("sample_point"))
    if tick_point not in allowed_tick_points:
        fail("tick_sample_point PBS inesperado")
    initial_rows = [row for row in samples if row.get("kind") == "initial"]
    if len(initial_rows) > 1:
        fail("solo se admite una muestra initial")
    ticks = [row for row in samples if row.get("kind") == "tick"]
    if len(initial_rows) + len(ticks) != len(samples):
        fail("kind PBS debe ser initial o tick")
    if len(ticks) != expected_ticks:
        fail(f"se esperaban {expected_ticks} ticks, se recibieron {len(ticks)}")

    previous_tick = None
    for row in initial_rows + ticks:
        tick = row.get("tick")
        if not isinstance(tick, int):
            fail("tick debe ser entero")
        if row.get("kind") == "tick" and previous_tick is not None and tick != previous_tick + 1:
            fail(f"ticks no consecutivos: {previous_tick} → {tick}")
        if row.get("kind") == "tick":
            previous_tick = tick
        for train in row.get("trains", []):
            if not all(
                isinstance(train.get(field), int)
                for field in ("vehicle", "x", "y", "progress", "speed", "subspeed", "direction")
            ):
                fail("tren PBS inválido")
            units = train.get("units")
            if units is not None:
                if not isinstance(units, list):
                    fail("units PBS debe ser lista")
                for unit in units:
                    if not all(
                        isinstance(unit.get(field), int)
                        for field in ("index", "x", "y", "rail_pixel", "direction")
                    ):
                        fail("unidad PBS inválida")
        for road in row.get("road_vehicles", []):
            if not all(
                isinstance(road.get(field), int)
                for field in (
                    "vehicle",
                    "x",
                    "y",
                    "progress",
                    "speed",
                    "subspeed",
                    "direction",
                    "state",
                    "frame",
                    "blocked_ctr",
                    "overtaking",
                    "overtaking_ctr",
                    "crashed_ctr",
                    "reverse_ctr",
                )
            ):
                fail("vehículo de carretera inválido")
        for reservation in row.get("rail_reservations", []):
            if not all(isinstance(reservation.get(field), int) for field in ("x", "y", "track_bits")):
                fail("reserva PBS inválida")

    print(f"OK: {path} · {len(ticks)} ticks · {producer}")


if __name__ == "__main__":
    main()
