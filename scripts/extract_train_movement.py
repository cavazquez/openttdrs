#!/usr/bin/env python3
"""Extrae tablas y constantes ferroviarias de OpenTTD a un fixture JSON.

Lee (solo lectura):

- ``OpenTTD/src/train_cmd.cpp`` — ``_accel_slowdown``, ``_vehicle_initial_*_fract``,
  constantes de ``Train::UpdateSpeed`` (AM_ORIGINAL).
- ``OpenTTD/src/rail_cmd.cpp`` — ``_fractcoords_enter``, ``_fractcoords_behind``,
  ``_deltacoord_leaveoffset``.
- ``OpenTTD/src/vehicle.cpp`` — ``_vehicle_subcoord``.
- ``OpenTTD/src/tunnelbridge_cmd.cpp`` — ``_tunnel_visibility_frame``.

Vuelca a ``crates/openttdrs-core/tests/fixtures/parity/train_movement_golden.json``.
El test ``golden_rail`` compara estas tablas contra las copiadas en
``src/train_movement.rs``.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
TRAIN_CMD = REPO_ROOT / "OpenTTD" / "src" / "train_cmd.cpp"
RAIL_CMD = REPO_ROOT / "OpenTTD" / "src" / "rail_cmd.cpp"
VEHICLE_CPP = REPO_ROOT / "OpenTTD" / "src" / "vehicle.cpp"
TUNNEL_CMD = REPO_ROOT / "OpenTTD" / "src" / "tunnelbridge_cmd.cpp"
OUT = (
    REPO_ROOT
    / "openttdrs"
    / "crates"
    / "openttdrs-core"
    / "tests"
    / "fixtures"
    / "parity"
    / "train_movement_golden.json"
)

DIAG_DIRS = ["NE", "SE", "SW", "NW"]
TRACKS = ["TRACK_X", "TRACK_Y", "TRACK_UPPER", "TRACK_LOWER", "TRACK_LEFT", "TRACK_RIGHT"]
DIRECTIONS = ["NE", "SE", "SW", "NW", "E", "S", "W", "N", "INVALID"]

COORD_RE = re.compile(r"\{\s*(-?\d+)\s*,\s*(-?\d+)\s*\}")
SUBCOORD_RE = re.compile(
    r"\{\s*\{\s*(\d+)\s*,\s*(\d+)\s*\}\s*,\s*Direction::(\w+)\s*\}"
)
EXPR_RE = re.compile(r"256\s*/\s*(\d+)")


def eval_accel_field(raw: str) -> int:
    raw = raw.strip()
    if EXPR_RE.fullmatch(raw):
        return 256 // int(EXPR_RE.fullmatch(raw).group(1))
    return int(raw)


def parse_diag_array_u8(text: str, name: str) -> list[int]:
    match = re.search(rf"{re.escape(name)}\{{([^}}]+)\}}", text)
    if match is None:
        raise SystemExit(f"tabla no encontrada: {name}")
    return [int(v) for v in re.findall(r"\b(\d+)\b", match.group(1))]


def parse_coord2d_array(text: str, name: str) -> list[dict[str, int]]:
    match = re.search(
        rf"static constexpr DiagDirectionIndexArray<Coord2D<[^>]+>> {re.escape(name)}\{{\{{\{{(.*?)\}}\}}\}};",
        text,
        re.DOTALL,
    )
    if match is None:
        raise SystemExit(f"tabla no encontrada: {name}")
    body = match.group(1)
    coords = []
    for m in COORD_RE.finditer(body):
        coords.append({"x": int(m.group(1)), "y": int(m.group(2))})
    if len(coords) != 4:
        raise SystemExit(f"{name}: se esperaban 4 coordenadas, hay {len(coords)}")
    return coords


def parse_delta_leave(text: str) -> list[dict[str, int]]:
    match = re.search(
        r"static constexpr DiagDirectionIndexArray<Coord2D<int8_t>> _deltacoord_leaveoffset\{\{\{(.*?)\}\}\};",
        text,
        re.DOTALL,
    )
    if match is None:
        raise SystemExit("tabla no encontrada: _deltacoord_leaveoffset")
    body = match.group(1)
    coords = []
    for m in COORD_RE.finditer(body):
        coords.append({"x": int(m.group(1)), "y": int(m.group(2))})
    if len(coords) != 4:
        raise SystemExit(f"_deltacoord_leaveoffset: se esperaban 4 entradas, hay {len(coords)}")
    return coords


def parse_accel_slowdown(text: str) -> list[dict[str, int]]:
    match = re.search(
        r"static const AccelerationSlowdownParams _accel_slowdown\[\]\s*=\s*\{(.*?)\};",
        text,
        re.DOTALL,
    )
    if match is None:
        raise SystemExit("tabla no encontrada: _accel_slowdown")
    rows = []
    for line in match.group(1).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        fields = [eval_accel_field(f) for f in re.findall(r"\{([^}]+)\}", line)[0].split(",")]
        if len(fields) != 4:
            raise SystemExit(f"_accel_slowdown: fila inválida: {line}")
        rows.append(
            {
                "small_turn": fields[0],
                "large_turn": fields[1],
                "z_up": fields[2],
                "z_down": fields[3],
            }
        )
    if len(rows) != 3:
        raise SystemExit(f"_accel_slowdown: se esperaban 3 filas, hay {len(rows)}")
    return rows


def parse_vehicle_subcoord(text: str) -> dict[str, dict[str, dict[str, int | str] | None]]:
    match = re.search(
        r"static constexpr DiagDirectionIndexArray<TrackIndexArray<VehicleSubcoordData>> _vehicle_subcoord\{\{\{(.*?)\}\}\};",
        text,
        re.DOTALL,
    )
    if match is None:
        raise SystemExit("tabla no encontrada: _vehicle_subcoord")
    body = match.group(1)
    out: dict[str, dict[str, dict[str, int | str] | None]] = {}
    blocks = re.split(r"\{\{\{ // ", body)
    for block in blocks[1:]:
        enter, _, rest = block.partition("\n")
        enter = enter.strip()
        track_entries: dict[str, dict[str, int | str] | None] = {t: None for t in TRACKS}
        for line in rest.splitlines():
            m_track = re.search(r"// (TRACK_\w+)", line)
            if m_track is None:
                continue
            track = m_track.group(1)
            m_data = SUBCOORD_RE.search(line)
            if m_data:
                track_entries[track] = {
                    "x": int(m_data.group(1)),
                    "y": int(m_data.group(2)),
                    "dir": m_data.group(3),
                }
        out[enter] = track_entries
    if set(out.keys()) != set(DIAG_DIRS):
        raise SystemExit(f"_vehicle_subcoord: direcciones inesperadas: {sorted(out)}")
    return out


def parse_update_speed_constants(text: str) -> dict[str, int]:
    match = re.search(
        r"case AM_ORIGINAL:\s*return this->DoUpdateSpeed\(this->acceleration \* \(this->GetAccelerationStatus\(\) == AS_BRAKE \? -(\d+) : (\d+)\)",
        text,
    )
    if match is None:
        raise SystemExit("constantes UpdateSpeed AM_ORIGINAL no encontradas")
    return {
        "brake_multiplier": int(match.group(1)),
        "accel_multiplier": int(match.group(2)),
    }


def main() -> int:
    train_text = TRAIN_CMD.read_text(encoding="utf-8")
    rail_text = RAIL_CMD.read_text(encoding="utf-8")
    vehicle_text = VEHICLE_CPP.read_text(encoding="utf-8")
    tunnel_text = TUNNEL_CMD.read_text(encoding="utf-8")

    fixture = {
        "source": {
            "train_cmd": "OpenTTD/src/train_cmd.cpp",
            "rail_cmd": "OpenTTD/src/rail_cmd.cpp",
            "vehicle": "OpenTTD/src/vehicle.cpp",
            "tunnelbridge_cmd": "OpenTTD/src/tunnelbridge_cmd.cpp",
        },
        "accel_slowdown": parse_accel_slowdown(train_text),
        "vehicle_initial_x_fract": parse_diag_array_u8(train_text, "_vehicle_initial_x_fract"),
        "vehicle_initial_y_fract": parse_diag_array_u8(train_text, "_vehicle_initial_y_fract"),
        "fractcoords_enter": parse_coord2d_array(rail_text, "_fractcoords_enter"),
        "fractcoords_behind": parse_coord2d_array(rail_text, "_fractcoords_behind"),
        "deltacoord_leaveoffset": parse_delta_leave(rail_text),
        "vehicle_subcoord": parse_vehicle_subcoord(vehicle_text),
        "tunnel_visibility_frame": parse_diag_array_u8(
            tunnel_text, "_tunnel_visibility_frame"
        ),
        "update_speed_am_original": parse_update_speed_constants(train_text),
        "connectivity": {
            "track_bits": {
                "X": 0x01,
                "Y": 0x02,
                "UPPER": 0x04,
                "LOWER": 0x08,
                "LEFT": 0x10,
                "RIGHT": 0x20,
            },
            "side_pairs": [
                {"sides": [0, 2], "bit": 0x01, "track": "X"},
                {"sides": [1, 3], "bit": 0x02, "track": "Y"},
                {"sides": [0, 3], "bit": 0x04, "track": "UPPER"},
                {"sides": [1, 2], "bit": 0x08, "track": "LOWER"},
                {"sides": [2, 3], "bit": 0x10, "track": "LEFT"},
                {"sides": [0, 1], "bit": 0x20, "track": "RIGHT"},
            ],
            "touching_side_masks": {
                "NE": 0x25,
                "SE": 0x2A,
                "SW": 0x19,
                "NW": 0x16,
            },
        },
    }

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(fixture, indent=2) + "\n", encoding="utf-8")
    print(f"fixture escrito: {OUT.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
