#!/usr/bin/env python3
"""Regresión sintética del diagnóstico post-sort de sprites (#323)."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_world_sort.py"


def write(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def bounds(xmin: int) -> dict[str, int]:
    return {
        "xmin": xmin,
        "ymin": 0,
        "zmin": 0,
        "xmax": xmin + 2,
        "ymax": 15,
        "zmax": 15,
    }


def sort_metadata() -> dict[str, object]:
    return {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-sort",
        "producer": "openttd",
        "stage": "post_viewport_sprite_sorter",
        "sorter": "ViewportSortParentSprites",
        "width": 2,
        "height": 1,
        "region": {"min_x": 0, "min_y": 0, "max_x": 1, "max_y": 0},
        "save_sha256": "a" * 64,
    }


def world_draw_metadata() -> dict[str, object]:
    return {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-draw",
        "producer": "openttdrs",
        "width": 2,
        "height": 1,
        "region": {"min_x": 0, "min_y": 0, "max_x": 1, "max_y": 0},
        "save_sha256": "a" * 64,
    }


def parent(final_ordinal: int, parent_id: int, sprite: int, xmin: int) -> dict[str, object]:
    return {
        "kind": "parent",
        "final_ordinal": final_ordinal,
        "parent_id": parent_id,
        "sprite": {"id": sprite},
        "palette": 775,
        "world_bounds": bounds(xmin),
        "first_child": -1,
    }


def candidate_draw(x: int, ordinal: int, sprite: int, offset_x: int) -> dict[str, object]:
    return {
        "kind": "draw",
        "x": x,
        "y": 0,
        "ordinal": ordinal,
        "primitive": "sortable",
        "sprite": {"id": sprite},
        "palette": 775,
        "resolved_palette": 775,
        "world": {"x": x * 16, "y": 0, "z": 0},
        "bounds": {"ox": offset_x, "oy": 0, "oz": 0, "ex": 3, "ey": 16, "ez": 16},
    }


def sort_stream(parents: list[dict[str, object]]) -> list[dict[str, object]]:
    return [
        sort_metadata(),
        {"kind": "sort_begin", "parents": len(parents), "children": 0},
        *parents,
        {"kind": "complete", "parents": len(parents), "children": 0},
    ]


def candidate_stream(draws: list[dict[str, object]]) -> list[dict[str, object]]:
    return [
        world_draw_metadata(),
        {"kind": "tile", "x": 0, "y": 0},
        *draws,
        {"kind": "complete", "tiles": 1, "draws": len(draws)},
    ]


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
        reference, candidate = root / "sort.jsonl", root / "candidate.jsonl"

        # El orden final coincide con la inserción: no debe informar una
        # divergencia sólo porque `world-sort` tenga un contrato separado.
        write(reference, sort_stream([parent(0, 0, 5982, 13), parent(1, 1, 5983, 16)]))
        write(candidate, candidate_stream([candidate_draw(0, 0, 5982, 13), candidate_draw(1, 0, 5983, 0)]))
        ok = run(reference, candidate)
        if ok.returncode != 0 or "subsecuencia del orden final" not in ok.stdout:
            print(ok.stdout, ok.stderr, file=sys.stderr)
            return 1

        # Caso mínimo real de Kale: `5982` se crea antes, pero el sorter pone
        # el `5983` con caja x=0 delante. El diagnóstico debe señalar el
        # primer par, no una cascada de todas las capas posteriores.
        write(reference, sort_stream([parent(0, 1, 5983, 0), parent(1, 0, 5982, 13)]))
        write(candidate, candidate_stream([candidate_draw(0, 0, 5982, 13), candidate_draw(0, 1, 5983, 0)]))
        report_path = root / "report.json"
        inverted = run(reference, candidate, "--json-report", str(report_path))
        if inverted.returncode != 1 or "candidate_order_inversion" not in inverted.stdout:
            print(inverted.stdout, inverted.stderr, file=sys.stderr)
            return 1
        report = json.loads(report_path.read_text(encoding="utf-8"))
        if (
            report["kind"] != "world-sort-compare"
            or report["status"] != "different"
            or report["first_inversion"]["before"]["expected_final_ordinal"] != 1
            or report["first_inversion"]["after"]["expected_final_ordinal"] != 0
        ):
            print(json.dumps(report, indent=2), file=sys.stderr)
            return 1

        # La cobertura parcial es informativa por defecto y se vuelve gate al
        # terminar la instrumentación de parents de una familia.
        write(candidate, candidate_stream([candidate_draw(0, 0, 5983, 0)]))
        partial = run(reference, candidate)
        if partial.returncode != 0 or "sin candidato equivalente" not in partial.stdout:
            print(partial.stdout, partial.stderr, file=sys.stderr)
            return 1
        strict = run(reference, candidate, "--strict-reference")
        if strict.returncode != 1 or "reference_parent_missing_in_candidate" not in strict.stdout:
            print(strict.stdout, strict.stderr, file=sys.stderr)
            return 1

        malformed = sort_stream([parent(1, 0, 5982, 13)])
        write(reference, malformed)
        bad_stream = run(reference, candidate)
        if bad_stream.returncode != 2 or "final_ordinal" not in bad_stream.stderr:
            print(bad_stream.stdout, bad_stream.stderr, file=sys.stderr)
            return 1

    print("OK: compare_world_sort detecta inversión, cobertura parcial y stream inválido")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
