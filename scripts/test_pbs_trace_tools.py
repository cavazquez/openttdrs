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

    print("OK: herramientas PBS externas")


if __name__ == "__main__":
    main()
