#!/usr/bin/env python3
"""Regresión sintética del comparador JSONL `world-semantic` (#306)."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_world_semantic.py"


def metadata(producer: str, stage: str) -> dict[str, object]:
    return {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-semantic",
        "producer": producer,
        "stage": stage,
        "tick": 12_345,
        "climate": 0,
        "openttd_commit": "a" * 40,
        "source_path": "/tmp/test.sav",
        "save_sha256": "b" * 64,
        "save_version": 300,
        "width": 2,
        "height": 1,
        "tile_count": 2,
        "emitted_tile_count": 2,
        "region": None,
    }


def tile(index: int, road_type: int = 0) -> dict[str, object]:
    return {
        "kind": "tile_semantic",
        "index": index,
        "x": index,
        "y": 0,
        "tile_type": 2,
        "class": "road",
        "tileh": 0,
        "base_z": 1,
        "owner": 2,
        "bridge_above_axis": None,
        "supported": True,
        "unsupported_reason": None,
        "raw": {"height": 1, "type": 0x20, "m1": 2, "m2": 0, "m3": 0, "m4": 0, "m5": 1, "m6": 0, "m7": 0, "m8": 0},
        "details": {
            "family": "road",
            "road_tile_type": 0,
            "road_bits": 1,
            "tram_bits": 0,
            "road_type": road_type,
            "tram_type": None,
            "crossing_road_axis": None,
            "crossing_rail_axis": None,
            "depot_direction": None,
            "roadside": 0,
        },
    }


def write_stream(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


def run(reference: Path, candidate: Path, *extra: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(COMPARE), str(reference), str(candidate), "--max-diffs", "2", *extra],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def main() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        reference = temp / "reference.jsonl"
        candidate = temp / "candidate.jsonl"
        reference_rows = [metadata("openttd", "after_load_game"), tile(0), tile(1)]
        candidate_rows = [metadata("openttdrs", "sav_map"), tile(0), tile(1)]
        write_stream(reference, reference_rows)
        write_stream(candidate, candidate_rows)

        report_path = temp / "semantic-report.json"
        ok = run(reference, candidate, "--json-report", str(report_path), "--show-inventory")
        if ok.returncode != 0:
            print(ok.stdout, ok.stderr, file=sys.stderr)
            print("FAIL: streams idénticos deberían coincidir", file=sys.stderr)
            return 1
        report = json.loads(report_path.read_text(encoding="utf-8"))
        inventory = report["candidate_inventory"]
        if (
            report["reference_inventory"] != inventory
            or inventory["classes"] != {"road": 2}
            or inventory["orientations"]["details.road_bits"] != {"1": 2}
            or inventory["variants"]["details.road_type"] != {"0": 2}
        ):
            print(json.dumps(report, ensure_ascii=False, indent=2), file=sys.stderr)
            print("FAIL: el inventario debe resumir clases, orientación y variantes", file=sys.stderr)
            return 1

        candidate_rows[2]["details"]["road_type"] = 7  # type: ignore[index]
        write_stream(candidate, candidate_rows)
        bad = run(reference, candidate)
        if bad.returncode != 1 or "details.road_type" not in bad.stdout or "x=1, y=0" not in bad.stdout:
            print(bad.stdout, bad.stderr, file=sys.stderr)
            print("FAIL: la mutación semántica debería señalar campo y coordenada", file=sys.stderr)
            return 1

        focused = run(reference, candidate, "--where", "0,0")
        if focused.returncode != 0:
            print(focused.stdout, focused.stderr, file=sys.stderr)
            print("FAIL: --where debería omitir la mutación fuera de foco", file=sys.stderr)
            return 1

        write_stream(candidate, candidate_rows[:1])
        truncated = run(reference, candidate)
        if truncated.returncode != 1 or "missing_candidate_tile" not in truncated.stdout:
            print(truncated.stdout, truncated.stderr, file=sys.stderr)
            print("FAIL: el stream truncado debería distinguirse", file=sys.stderr)
            return 1

    print("OK: compare_world_semantic detecta campo, filtro y stream truncado")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
