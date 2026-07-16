#!/usr/bin/env python3
"""Mutación sintética: compare_snapshots debe fallar ante un hash alterado (#110)."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_snapshots.py"
FIXTURE = ROOT / "crates" / "openttdrs-core" / "tests" / "fixtures" / "m3_road_tram_2x2.ottdmap"


def main() -> int:
    if not FIXTURE.is_file():
        print(f"FAIL: falta fixture {FIXTURE}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory() as td:
        td_path = Path(td)
        base = td_path / "base.json"
        mutated = td_path / "mutated.json"
        # Generar snapshot candidato (no es oráculo; solo base para mutar).
        r = subprocess.run(
            [
                "cargo",
                "run",
                "-q",
                "-p",
                "openttdrs-core",
                "--bin",
                "snapshot_dumper",
                "--",
                str(FIXTURE),
                str(base),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if r.returncode != 0:
            print(r.stderr, file=sys.stderr)
            print("FAIL: snapshot_dumper", file=sys.stderr)
            return 1

        data = json.loads(base.read_text(encoding="utf-8"))
        data["producer"] = "openttd"  # simula oráculo
        data["openttd_commit"] = "0" * 40
        oracle = td_path / "oracle.json"
        oracle.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")

        cand = json.loads(base.read_text(encoding="utf-8"))
        cand["producer"] = "openttdrs"
        # Mutación sintética en un campo hard.
        h = cand["hashes"]["height_hash_fnv1a64"]
        cand["hashes"]["height_hash_fnv1a64"] = ("0" if h[0] != "0" else "1") + h[1:]
        mutated.write_text(json.dumps(cand, indent=2) + "\n", encoding="utf-8")

        ok = subprocess.run(
            [sys.executable, str(COMPARE), str(oracle), str(base)],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if ok.returncode != 0:
            print(ok.stdout, ok.stderr, file=sys.stderr)
            print("FAIL: comparación idéntica debería ser OK", file=sys.stderr)
            return 1

        bad = subprocess.run(
            [sys.executable, str(COMPARE), str(oracle), str(mutated)],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if bad.returncode != 1:
            print(bad.stdout, bad.stderr, file=sys.stderr)
            print("FAIL: mutación debería devolver exit 1", file=sys.stderr)
            return 1
        if "primera divergencia" not in bad.stdout and "height_hash" not in bad.stdout:
            print(bad.stdout, file=sys.stderr)
            print("FAIL: no se reportó la divergencia de height_hash", file=sys.stderr)
            return 1

        circular = subprocess.run(
            [sys.executable, str(COMPARE), str(base), str(base)],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if circular.returncode != 1 or "circular" not in circular.stdout:
            print(circular.stdout, file=sys.stderr)
            print("FAIL: debería rechazar comparación circular openttdrs/openttdrs", file=sys.stderr)
            return 1

    print("OK: mutación detectada y comparación circular rechazada")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
