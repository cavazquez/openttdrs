#!/usr/bin/env python3
"""Extrae tablas de movimiento de vehículos de carretera de OpenTTD a un fixture JSON.

Lee (solo lectura) ``OpenTTD/src/table/roadveh_movement.h`` y vuelca a
``crates/openttdrs-core/tests/fixtures/parity/roadveh_movement_golden.json``:

- ``_roadveh_drive_data_0`` (recta NE, carril izquierdo)
- ``_roadveh_drive_data_2`` (curva corta NW→NE)
- ``_roadveh_drive_data_3`` (curva larga NE→SE)
- ``_road_stop_stop_frame`` (frame de parada en bahías, valores 11-20)
- ``_rv_station_left_{sw,nw,ne,se}_{far,near}`` (trayectorias de entrada/lazo
  dentro de la bahía, lado izquierdo — el que usa el port) con su stop frame

El test ``golden_roadveh`` de openttdrs-core compara estas tablas contra las
copiadas en ``src/road_movement.rs`` y contra la trayectoria del runner.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HEADER = REPO_ROOT / "OpenTTD" / "src" / "table" / "roadveh_movement.h"
OUT = (
    REPO_ROOT
    / "openttdrs"
    / "crates"
    / "openttdrs-core"
    / "tests"
    / "fixtures"
    / "parity"
    / "roadveh_movement_golden.json"
)

DRIVE_TABLES = ["_roadveh_drive_data_0", "_roadveh_drive_data_2", "_roadveh_drive_data_3"]

# Tablas de bahía del lado izquierdo (el port copia data_0 = carril izquierdo).
# El índice en _road_stop_stop_frame sigue el orden de _road_drive_data:
# [sw_far, nw_far, sw_near, nw_near] ×2, [ne_far, se_far, ne_near, se_near] ×2.
STATION_TABLES = {
    "_rv_station_left_sw_far": 0,
    "_rv_station_left_nw_far": 1,
    "_rv_station_left_sw_near": 2,
    "_rv_station_left_nw_near": 3,
    "_rv_station_left_ne_far": 8,
    "_rv_station_left_se_far": 9,
    "_rv_station_left_ne_near": 10,
    "_rv_station_left_se_near": 11,
}

ENTRY_RE = re.compile(r"\{\s*(\d+)\s*,\s*(\d+)\s*\}")
MARKER_RE = re.compile(
    r"\{(RDE_NEXT_TILE|RDE_TURNED)\s*\|\s*to_underlying\(DiagDirection::(\w+)\)"
)


def extract_array_body(text: str, name: str) -> str:
    match = re.search(rf"{re.escape(name)}\[\]\s*=\s*\{{(.*?)\n\}};", text, re.DOTALL)
    if match is None:
        raise SystemExit(f"tabla no encontrada: {name}")
    return match.group(1)


def parse_drive_table(text: str, name: str) -> dict:
    body = extract_array_body(text, name)
    frames = []
    marker = None
    for line in body.splitlines():
        line = line.strip().rstrip(",")
        if not line:
            continue
        m = MARKER_RE.search(line)
        if m:
            marker = {"flag": m.group(1), "diagdir": m.group(2)}
            continue
        m = ENTRY_RE.search(line)
        if m:
            frames.append([int(m.group(1)), int(m.group(2))])
    if marker is None:
        raise SystemExit(f"tabla sin marcador RDE_*: {name}")
    return {"frames": frames, "end": marker}


def parse_stop_frames(text: str) -> list[int]:
    body = extract_array_body(text, "_road_stop_stop_frame")
    values = [int(v) for v in re.findall(r"\b(\d+)\b", body)]
    if len(values) != 32:
        raise SystemExit(f"_road_stop_stop_frame: se esperaban 32 valores, hay {len(values)}")
    return values


def parse_station_tables(text: str, stop_frames: list[int]) -> dict:
    tables = {}
    for name, frame_index in STATION_TABLES.items():
        table = parse_drive_table(text, name)
        stop = stop_frames[frame_index]
        points = table["frames"]
        if not 0 <= stop < len(points):
            raise SystemExit(f"{name}: stop frame {stop} fuera de rango ({len(points)} puntos)")
        # El stop frame debe ser el punto más profundo del lazo (el vehículo
        # se detiene justo donde la trayectoria empieza a retroceder).
        if stop + 1 < len(points) and points[stop + 1] != points[stop - 1]:
            raise SystemExit(f"{name}: el punto {stop} no es el vértice del lazo")
        table["stop_frame"] = stop
        tables[name] = table
    return tables


def main() -> int:
    text = HEADER.read_text(encoding="utf-8")
    stop_frames = parse_stop_frames(text)
    fixture = {
        "source": "OpenTTD/src/table/roadveh_movement.h",
        "drive_data": {name: parse_drive_table(text, name) for name in DRIVE_TABLES},
        "station_tables": parse_station_tables(text, stop_frames),
        "road_stop_stop_frame": stop_frames,
        "constants": {
            "tile_axial_distance": 192,
            "tile_corner_distance": 256,
            "advance_speed_numerator": 3,
            "advance_speed_denominator": 4,
            "curve_speed_penalty_shift": 2,
        },
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(fixture, indent=2) + "\n", encoding="utf-8")
    print(f"fixture escrito: {OUT.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
