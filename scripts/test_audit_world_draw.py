#!/usr/bin/env python3
"""Regresión sintética del backlog global `world-draw`."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
AUDIT = ROOT / "scripts" / "audit_world_draw.py"


def stream(producer: str, sprite: int, *, fallback: bool = False) -> list[dict[str, object]]:
    metadata = {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-draw",
        "producer": producer,
        "width": 1,
        "height": 1,
        "region": {"min_x": 0, "min_y": 0, "max_x": 0, "max_y": 0},
    }
    tile = {"kind": "tile", "index": 0, "x": 0, "y": 0, "tile_type": 9}
    draw = {
        "kind": "draw",
        "x": 0,
        "y": 0,
        "ordinal": 0,
        "role": "station-rail-track",
        "primitive": "ground",
        "sprite": {"id": sprite},
        "fallback": fallback,
        "geometry_explicit": True,
        "world": {"x": 0, "y": 0, "z": 0},
        "offset": {"x": 0, "y": 0, "z": 0},
        "bounds": None,
    }
    return [metadata, tile, draw, {"kind": "complete", "tiles": 1, "draws": 1}]


def write(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def main() -> int:
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        reference, candidate, out = root / "cpp.jsonl", root / "rust.jsonl", root / "audit.json"
        write(reference, stream("openttd", 100))
        # El 200 falla por sprite, geometría y orden, pero debe sumar un único
        # desvío priorizable. El informe conserva las tres columnas técnicas.
        write(candidate, stream("openttdrs", 200))
        result = subprocess.run(
            [sys.executable, str(AUDIT), str(reference), str(candidate), "--json-out", str(out)],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        if result.returncode != 0:
            print(result.stdout, result.stderr, file=sys.stderr)
            return 1
        report = json.loads(out.read_text(encoding="utf-8"))
        role = report["role_summary"][0]
        if (
            report["reference_tiles"] != 1
            or report["candidate_tiles"] != 1
            or report["unmatched_draw_count"] != 1
            or role["priority_count"] != 1
            or role["missing_sprite"] != 1
            or role["missing_geometry"] != 1
            or role["missing_order"] != 1
        ):
            print(json.dumps(report, indent=2), file=sys.stderr)
            return 1
        if report["findings"][0]["kind"] != "missing_sprite":
            print(json.dumps(report, indent=2), file=sys.stderr)
            return 1

    print("OK: audit_world_draw agrupa evidencia y prioriza draws únicos")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
