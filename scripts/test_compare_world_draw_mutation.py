#!/usr/bin/env python3
"""Regresión sintética del comparador `world-draw` (#307)."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_world_draw.py"


def metadata(producer: str) -> dict[str, object]:
    return {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-draw",
        "producer": producer,
        "width": 1,
        "height": 1,
        "region": {"min_x": 0, "min_y": 0, "max_x": 0, "max_y": 0},
        "save_sha256": "a" * 64,
    }


def tile() -> dict[str, object]:
    return {"kind": "tile", "index": 0, "x": 0, "y": 0, "tile_type": 9}


def draw(sprite: int, primitive: str, *, fallback: bool = False) -> dict[str, object]:
    return {
        "kind": "draw",
        "x": 0,
        "y": 0,
        "ordinal": 0,
        "role": primitive,
        "primitive": primitive,
        "sprite": {"id": sprite},
        "fallback": fallback,
    }


def write(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def stream(producer: str, draws: list[dict[str, object]]) -> list[dict[str, object]]:
    return [metadata(producer), tile(), *draws, {"kind": "complete", "tiles": 1, "draws": len(draws)}]


def run(reference: Path, candidate: Path, *extra: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(COMPARE), str(reference), str(candidate), *extra],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def main() -> int:
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        reference, candidate = root / "cpp.jsonl", root / "rust.jsonl"
        # El 6139 representa un bounding box de orden, no píxeles visibles.
        write(reference, stream("openttd", [draw(2391, "ground"), draw(2392, "sortable"), draw(6139, "empty_bounds")]))
        write(candidate, stream("openttdrs", [draw(2391, "ground"), draw(2392, "sortable")]))
        ok = run(reference, candidate)
        if ok.returncode != 0:
            print(ok.stdout, ok.stderr, file=sys.stderr)
            return 1
        grouped = run(reference, candidate, "--by-role")
        if grouped.returncode != 0 or "ground: selecciones=1, IDs=1" not in grouped.stdout:
            print(grouped.stdout, grouped.stderr, file=sys.stderr)
            return 1

        write(candidate, stream("openttdrs", [draw(2391, "ground"), draw(9999, "sortable")]))
        missing = run(reference, candidate)
        if missing.returncode != 1 or "candidate_sprite_missing_in_reference" not in missing.stdout:
            print(missing.stdout, missing.stderr, file=sys.stderr)
            return 1

        # La comparación es por multiconjunto: una segunda capa con el mismo
        # ID no puede reutilizar la única llamada del oráculo.
        write(
            candidate,
            stream(
                "openttdrs",
                [draw(2391, "ground"), draw(2391, "sortable"), draw(2392, "sortable")],
            ),
        )
        duplicate = run(reference, candidate)
        if duplicate.returncode != 1 or "candidate_sprite_missing_in_reference" not in duplicate.stdout:
            print(duplicate.stdout, duplicate.stderr, file=sys.stderr)
            return 1

        write(candidate, stream("openttdrs", [draw(2391, "ground", fallback=True)]))
        fallback = run(reference, candidate)
        if fallback.returncode != 1 or "candidate_fallback" not in fallback.stdout:
            print(fallback.stdout, fallback.stderr, file=sys.stderr)
            return 1

    print("OK: compare_world_draw detecta selección inválida, fallback y separadores")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
