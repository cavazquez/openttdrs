#!/usr/bin/env python3
"""Mutación sintética: el check de tablas generadas debe fallar (#119)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "scripts" / "generated_tables_manifest.json"
POP_OUT = ROOT / "crates" / "openttdrs-core" / "src" / "sav" / "house_population_generated.rs"
UPSTREAM = ROOT / "reference" / "openttd-upstream" / "src" / "table" / "town_land.h"


def run(cmd: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    merged = os.environ.copy()
    if env:
        merged.update(env)
    return subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
        env=merged,
    )


def main() -> int:
    if not UPSTREAM.is_file():
        print(
            "FAIL: hace falta OpenTTD pin local para este test "
            "(CI usa --fetch-upstream antes)",
            file=sys.stderr,
        )
        return 1

    clean = run([sys.executable, "scripts/check_generated_tables.py", "--check"])
    if clean.returncode != 0:
        print(clean.stdout)
        print(clean.stderr, file=sys.stderr)
        print("FAIL: check limpio debería pasar", file=sys.stderr)
        return 1

    original = POP_OUT.read_text(encoding="utf-8")
    try:
        POP_OUT.write_text(original + "// mutation\n", encoding="utf-8")
        dirty = run([sys.executable, "scripts/gen_house_population.py", "--check"])
        if dirty.returncode == 0:
            print("FAIL: --check no detectó mutación en house_population", file=sys.stderr)
            return 1
        blob = dirty.stderr + dirty.stdout
        if "DRIFT" not in blob:
            print("FAIL: mensaje DRIFT ausente", file=sys.stderr)
            print(blob, file=sys.stderr)
            return 1
    finally:
        POP_OUT.write_text(original, encoding="utf-8")

    # Hash: manifiesto temporal con sha256 falso para house_draw_data.
    man = json.loads(MANIFEST.read_text(encoding="utf-8"))
    for p in man["pilots"]:
        if p["id"] == "house_draw_data":
            p["output_sha256"] = "0" * 64
            break
    else:
        print("FAIL: piloto house_draw_data ausente", file=sys.stderr)
        return 1

    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as fh:
        json.dump(man, fh)
        bogus_manifest = fh.name
    try:
        hash_dirty = run(
            [sys.executable, "scripts/check_generated_tables.py", "--check"],
            env={"OPENTTDRS_GENERATED_TABLES_MANIFEST": bogus_manifest},
        )
        if hash_dirty.returncode == 0:
            print("FAIL: check no detectó sha256 falso de house_draw_data", file=sys.stderr)
            print(hash_dirty.stdout, hash_dirty.stderr, file=sys.stderr)
            return 1
    finally:
        Path(bogus_manifest).unlink(missing_ok=True)

    # Restaurar: working tree intacto.
    again = run([sys.executable, "scripts/check_generated_tables.py", "--check"])
    if again.returncode != 0:
        print("FAIL: working tree quedó sucio tras mutación", file=sys.stderr)
        print(again.stderr, file=sys.stderr)
        return 1

    print("OK: mutación detectada (regenerate + hash) y working tree limpio")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
