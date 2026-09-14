#!/usr/bin/env python3
"""Convierte parity_runner al contrato PBS externo v1/v2."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def main() -> None:
    if len(sys.argv) != 3:
        fail("uso: normalize_pbs_trace.py <parity_runner.jsonl> <salida.jsonl>")
    source, destination = map(Path, sys.argv[1:])
    try:
        records = [
            json.loads(line)
            for line in source.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer {source}: {exc}")
    if not records:
        fail("traza candidata vacía")

    has_road = any(
        isinstance(vehicle, dict) and isinstance(vehicle.get("road"), dict)
        for record in records
        for vehicle in record.get("vehicles", [])
    )
    schema_version = 2 if has_road else 1

    rows: list[dict[str, object]] = [
        {
            "kind": "metadata",
            "schema_version": schema_version,
            "producer": "openttdrs",
            "tick_sample_point": "after_game_state_step",
            "source_path": str(source),
        }
    ]
    for record in records:
        trains = []
        road_vehicles = []
        for vehicle in record.get("vehicles", []):
            rail = vehicle.get("rail")
            tile = vehicle["tile"]
            if rail is not None:
                trains.append(
                    {
                        "vehicle": vehicle["id"],
                        "x": tile["x"],
                        "y": tile["y"],
                        "progress": vehicle["progress"],
                        "speed": vehicle["speed"],
                        "subspeed": vehicle["subspeed"],
                        "direction": vehicle["dir"],
                    }
                )
            road = vehicle.get("road")
            if road is not None:
                road_vehicles.append(
                    {
                        "vehicle": vehicle["id"],
                        "x": tile["x"],
                        "y": tile["y"],
                        "progress": vehicle["progress"],
                        "speed": vehicle["speed"],
                        "subspeed": vehicle["subspeed"],
                        "direction": vehicle["dir"],
                        "state": road["state"],
                        "frame": road["frame"],
                        "blocked_ctr": road["blocked_ctr"],
                        "overtaking": road["overtaking"],
                        "overtaking_ctr": road["overtaking_ctr"],
                        "crashed_ctr": road["crashed_ctr"],
                        "reverse_ctr": road["reverse_ctr"],
                    }
                )
        trains.sort(key=lambda train: (train["x"], train["y"], train["vehicle"]))
        road_vehicles.sort(
            key=lambda vehicle: (vehicle["x"], vehicle["y"], vehicle["vehicle"])
        )
        reservations = [
            {
                "x": reservation["tile"]["x"],
                "y": reservation["tile"]["y"],
                "track_bits": reservation["track_bits"],
            }
            for reservation in record.get("rail_reservations", [])
        ]
        reservations.sort(key=lambda reservation: (reservation["y"], reservation["x"]))
        rows.append(
            {
                "kind": "tick",
                "tick": record["tick"],
                "trains": trains,
                "road_vehicles": road_vehicles,
                "rail_reservations": reservations,
            }
        )

    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in rows),
        encoding="utf-8",
    )
    print(f"OK: {source} → {destination} ({len(records)} ticks)")


if __name__ == "__main__":
    main()
