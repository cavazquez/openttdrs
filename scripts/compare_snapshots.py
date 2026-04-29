#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path


HARD_PATHS = [
    ("map", "width"),
    ("map", "height"),
    ("hashes", "height_hash_fnv1a64"),
    ("hashes", "kind_hash_fnv1a64"),
    ("hashes", "mapt_hash_fnv1a64"),
    ("hashes", "rail_bits_hash_fnv1a64"),
    ("hashes", "road_bits_hash_fnv1a64"),
    ("components", "industry_components"),
    ("components", "station_components"),
]


def get_path(data: dict, path: tuple[str, str]):
    cur = data
    for key in path:
        cur = cur[key]
    return cur


def main() -> int:
    if len(sys.argv) != 3:
        print("Uso: compare_snapshots.py <oracle.json> <candidate.json>")
        return 2

    p_oracle = Path(sys.argv[1])
    p_candidate = Path(sys.argv[2])
    oracle = json.loads(p_oracle.read_text(encoding="utf-8"))
    candidate = json.loads(p_candidate.read_text(encoding="utf-8"))

    mismatches: list[str] = []
    for path in HARD_PATHS:
        a = get_path(oracle, path)
        b = get_path(candidate, path)
        if a != b:
            dotted = ".".join(path)
            mismatches.append(f"{dotted}: oracle={a!r} candidate={b!r}")

    if mismatches:
        print("FAIL: snapshots distintos en campos hard")
        for m in mismatches:
            print(f" - {m}")
        return 1

    print("OK: snapshots equivalentes en campos hard")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
