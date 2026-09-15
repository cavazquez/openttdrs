#!/usr/bin/env python3
"""Regresión del analizador de bandas del sorter de captura."""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from analyze_viewport_sort_bands import TraceError, analyze, load_candidate, load_trace


def parent(segment: int, final_ordinal: int, sprite: int, xmin: int) -> dict[str, object]:
    return {
        "kind": "parent",
        "segment": segment,
        "final_ordinal": final_ordinal,
        "image": sprite,
        "world_bounds": {
            "xmin": xmin,
            "ymin": 0,
            "zmin": 0,
            "xmax": xmin + 15,
            "ymax": 15,
            "zmax": 47,
        },
    }


def main() -> int:
    metadata = {
        "kind": "metadata",
        "schema_version": 1,
        "contract": "world-screenshot-sort",
        "producer": "openttd",
        "stage": "post_viewport_sprite_sorter",
        "width": 1280,
        "height": 720,
        "zoom": 2,
    }
    rows = [
        metadata,
        {
            "kind": "segment",
            "ordinal": 0,
            "virtual": {"left": 0, "top": 0, "width": 1600, "height": 204},
            "parents": 2,
            "children": 0,
        },
        parent(0, 0, 100, 0),
        parent(0, 1, 200, 16),
        {
            "kind": "segment",
            "ordinal": 1,
            "virtual": {"left": 0, "top": 204, "width": 1600, "height": 204},
            "parents": 2,
            "children": 1,
        },
        parent(1, 0, 101, 0),
        parent(1, 1, 300, 32),
        {"kind": "complete", "segments": 2, "parents": 4},
    ]
    candidate = {
        "contract": "openttdrs-viewport-sort",
        "schema_version": 1,
        "stage": "post_viewport_sprite_sorter",
        "parents_before_sort": 3,
        "parents": [
            {
                "sprite_id": 100,
                "world_bounds": rows[2]["world_bounds"],
            },
            {
                "sprite_id": 200,
                "world_bounds": rows[3]["world_bounds"],
            },
        ],
        "local_proxies": [
            {
                "band": 1,
                "band_ordinal": 0,
                "source_child": 42,
                "original_parent": 7,
                "sprite_id": 300,
                "world_bounds": rows[6]["world_bounds"],
            }
        ],
    }

    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        reference_path = root / "sort.jsonl"
        candidate_path = root / "candidate.json"
        reference_path.write_text(
            "".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8"
        )
        candidate_path.write_text(json.dumps(candidate), encoding="utf-8")

        trace = load_trace(reference_path)
        report = analyze(trace, load_candidate(candidate_path))
        summary = report["summary"]
        candidate_summary = report["candidate"]
        if (
            summary["segments"] != 2
            or summary["parents"] != 4
            or summary["promoted_world_bounds"] != 1
            or summary["extra_sprite_variants"] != 1
            or candidate_summary["same_bounds_different_sprite"] != 1
            or candidate_summary["reference_bounds_not_in_candidate"] != 0
            or candidate_summary["local_proxies"] != 1
            or candidate_summary["effective_parents"] != 3
        ):
            print(json.dumps(report, indent=2), file=sys.stderr)
            return 1

        malformed = rows[:-1] + [{"kind": "complete", "segments": 2, "parents": 3}]
        reference_path.write_text(
            "".join(json.dumps(row) + "\n" for row in malformed), encoding="utf-8"
        )
        try:
            load_trace(reference_path)
        except TraceError:
            pass
        else:
            print("FAIL: un complete truncado debe rechazarse", file=sys.stderr)
            return 1

    print("OK: el analizador separa promociones, cajas ausentes y sprites distintos")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
