#!/usr/bin/env python3
"""Regresión sintética del comparador JSONL `world-raw` (#305)."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_world_raw.py"


def metadata(producer: str, stage: str) -> dict[str, object]:
    return {
        "kind": "metadata",
        "schema_version": 2,
        "contract": "world-raw",
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


def tile(index: int, m4: int = 0) -> dict[str, object]:
    return {
        "kind": "tile_raw",
        "index": index,
        "x": index,
        "y": 0,
        "height": 1,
        "type": 0x10,
        "m1": 2,
        "m2": 0x1234,
        "m3": 3,
        "m4": m4,
        "m5": 5,
        "m6": 6,
        "m7": 7,
        "m8": 0x5678,
    }


def write_stream(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


def compare(reference: Path, candidate: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(COMPARE), str(reference), str(candidate), "--max-diffs", "2"],
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

        ok = compare(reference, candidate)
        if ok.returncode != 0:
            print(ok.stdout, ok.stderr, file=sys.stderr)
            print("FAIL: streams idénticos deberían coincidir", file=sys.stderr)
            return 1

        candidate_rows[2]["m4"] = 0x9A
        write_stream(candidate, candidate_rows)
        bad = compare(reference, candidate)
        if bad.returncode != 1 or "m4" not in bad.stdout or "x=1, y=0" not in bad.stdout:
            print(bad.stdout, bad.stderr, file=sys.stderr)
            print("FAIL: la mutación m4 debería señalar coordenada y campo", file=sys.stderr)
            return 1

        write_stream(candidate, candidate_rows[:1])
        truncated = compare(reference, candidate)
        if truncated.returncode != 1 or "missing_candidate_tile" not in truncated.stdout:
            print(truncated.stdout, truncated.stderr, file=sys.stderr)
            print("FAIL: el stream truncado debería distinguirse", file=sys.stderr)
            return 1

    print("OK: compare_world_raw detecta byte mutado y stream truncado")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
