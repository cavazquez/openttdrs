#!/usr/bin/env python3
"""Compara snapshot oráculo (OpenTTD) vs candidato (openttdrs).

Uso: compare_snapshots.py <oracle.json> <candidate.json>
"""
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


def get_path(data: dict, path: tuple[str, ...]):
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

    if oracle.get("producer") == "openttdrs" and candidate.get("producer") == "openttdrs":
        print(
            "FAIL: ambos producer=openttdrs — comparación circular "
            "(el oráculo debe venir de OpenTTD C++, producer=openttd)"
        )
        return 1

    mismatches: list[str] = []
    for path in HARD_PATHS:
        try:
            a = get_path(oracle, path)
            b = get_path(candidate, path)
        except (KeyError, TypeError) as e:
            dotted = ".".join(path)
            mismatches.append(f"{dotted}: campo ausente ({e})")
            continue
        if a != b:
            dotted = ".".join(path)
            mismatches.append(f"{dotted}: oracle={a!r} candidate={b!r}")

    if mismatches:
        print("FAIL: primera divergencia:")
        print(f" - {mismatches[0]}")
        if len(mismatches) > 1:
            print(f"  (+{len(mismatches) - 1} más)")
            for m in mismatches[1:]:
                print(f" - {m}")
        return 1

    print("OK: snapshots equivalentes en campos hard")
    if "producer" in oracle or "producer" in candidate:
        print(
            f"  producers: oracle={oracle.get('producer')!r} "
            f"candidate={candidate.get('producer')!r}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
