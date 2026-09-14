#!/usr/bin/env python3
"""Pruebas sin OpenTTD para el contrato y comparador PBS externo."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def run(*args: str) -> None:
    subprocess.run(args, cwd=ROOT, check=True)


def write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        raw = work / "candidate.raw.jsonl"
        candidate = work / "candidate.jsonl"
        oracle = work / "oracle.jsonl"
        write_jsonl(
            raw,
            [
                {
                    "tick": 1,
                    "vehicles": [
                        {
                            "id": 42,
                            "tile": {"x": 2, "y": 0},
                            "progress": 64,
                            "speed": 12,
                            "subspeed": 3,
                            "dir": 5,
                            "rail": {},
                        }
                    ],
                    "rail_reservations": [{"tile": {"x": 3, "y": 0}, "track_bits": 1}],
                }
            ],
        )
        write_jsonl(
            oracle,
            [
                {
                    "kind": "metadata",
                    "schema_version": 1,
                    "producer": "openttd",
                    "sample_point": "after_state_game_loop",
                },
                {
                    "kind": "tick",
                    "tick": 1,
                    "trains": [
                        {
                            "vehicle": 7,
                            "x": 2,
                            "y": 0,
                            "progress": 64,
                            "speed": 12,
                            "subspeed": 3,
                            "direction": 5,
                        }
                    ],
                    "rail_reservations": [{"x": 3, "y": 0, "track_bits": 1}],
                },
            ],
        )
        run(sys.executable, "scripts/normalize_pbs_trace.py", str(raw), str(candidate))
        run(sys.executable, "scripts/validate_pbs_trace.py", str(candidate), "1", "openttdrs")
        run(sys.executable, "scripts/validate_pbs_trace.py", str(oracle), "1", "openttd")
        run(sys.executable, "scripts/compare_pbs_traces.py", str(oracle), str(candidate))

        road_raw = work / "road-candidate.raw.jsonl"
        road_candidate = work / "road-candidate.jsonl"
        road_oracle = work / "road-oracle.jsonl"
        road_state = {
            "state": 9,
            "frame": 4,
            "blocked_ctr": 2,
            "overtaking": 1,
            "overtaking_ctr": 7,
            "crashed_ctr": 0,
            "reverse_ctr": 3,
        }
        write_jsonl(
            road_raw,
            [
                {
                    "tick": 8,
                    "vehicles": [
                        {
                            "id": 99,
                            "tile": {"x": 4, "y": 5},
                            "progress": 17,
                            "speed": 23,
                            "subspeed": 6,
                            "dir": 2,
                            "road": road_state,
                        }
                    ],
                    "rail_reservations": [],
                }
            ],
        )
        write_jsonl(
            road_oracle,
            [
                {
                    "kind": "metadata",
                    "schema_version": 2,
                    "producer": "openttd",
                    "sample_point": "after_state_game_loop",
                },
                {
                    "kind": "tick",
                    "tick": 8,
                    "trains": [],
                    "road_vehicles": [
                        {
                            "vehicle": 13,
                            "x": 4,
                            "y": 5,
                            "progress": 17,
                            "speed": 23,
                            "subspeed": 6,
                            "direction": 2,
                            **road_state,
                        }
                    ],
                    "rail_reservations": [],
                },
            ],
        )
        run(
            sys.executable,
            "scripts/normalize_pbs_trace.py",
            str(road_raw),
            str(road_candidate),
        )
        run(sys.executable, "scripts/validate_pbs_trace.py", str(road_candidate), "1", "openttdrs")
        run(sys.executable, "scripts/validate_pbs_trace.py", str(road_oracle), "1", "openttd")
        run(
            sys.executable,
            "scripts/compare_pbs_traces.py",
            str(road_oracle),
            str(road_candidate),
            "--scope",
            "road",
        )

        html = work / "trace.html"
        fixture = (
            ROOT
            / "crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl"
        )
        run(
            sys.executable,
            "scripts/view_pbs_trace.py",
            str(fixture),
            str(html),
            "--signal",
            "46,37,path",
        )
        content = html.read_text(encoding="utf-8")
        assert "PBS trace viewer" in content
        assert '"x":46' in content and '"y":37' in content
        assert '"label":"path"' in content

    print("OK: herramientas PBS externas")


if __name__ == "__main__":
    main()
