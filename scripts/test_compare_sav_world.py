#!/usr/bin/env python3
"""Regresión de interfaz para el orquestador de paridad SAV (#304)."""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "compare_sav_world.sh"


def run(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SCRIPT), *args], cwd=ROOT, text=True, capture_output=True, env=env
    )


FAKE_EXPORTER = """#!/usr/bin/env python3
import json
import pathlib
import sys

stage, side = pathlib.Path(sys.argv[0]).stem.rsplit("-", 1)
out = pathlib.Path(sys.argv[2])
schema = 1 if stage == "semantic" and side == "reference" else 2
row = {
    "kind": "metadata",
    "schema_version": schema,
    "contract": f"world-{stage}",
    "producer": "openttd" if side == "reference" else "openttdrs",
    "width": 1,
    "height": 1,
    "tile_count": 1,
    "emitted_tile_count": 0,
    "region": {"min_x": 0, "min_y": 0, "max_x": 0, "max_y": 0},
    "save_sha256": "a" * 64,
}
out.write_text(json.dumps(row) + "\\n", encoding="utf-8")
"""


def fake_exporters(root: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for stage in ("raw", "semantic", "draw"):
        for side in ("reference", "candidate"):
            path = root / f"{stage}-{side}.py"
            path.write_text(FAKE_EXPORTER, encoding="utf-8")
            path.chmod(0o755)
            result[f"OPENTTDRS_WORLD_ORACLE_{stage.upper()}_{side.upper()}_EXPORT"] = str(path)
    return result


def main() -> int:
    syntax = subprocess.run(["bash", "-n", str(SCRIPT)], cwd=ROOT, text=True, capture_output=True)
    if syntax.returncode != 0:
        print(syntax.stdout, syntax.stderr, file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        sav = root / "fixture.sav"
        sav.write_bytes(b"fixture")
        output = root / "oracle"

        dry_run = run(
            str(sav),
            str(output),
            "--tile",
            "5,7",
            "--radius",
            "2",
            "--kind",
            "raw,draw",
            "--max-diffs",
            "3",
            "--dry-run",
        )
        expected = ("region=3,5,7,9", "tile=5,7", "stages=raw,draw", "max_diffs=3")
        if dry_run.returncode != 0 or any(item not in dry_run.stdout for item in expected):
            print(dry_run.stdout, dry_run.stderr, file=sys.stderr)
            return 1

        incompatible = run(
            str(sav), str(output), "--tile", "1,1", "--region", "0,0,1,1", "--dry-run"
        )
        if incompatible.returncode != 2 or "incompatibles" not in incompatible.stderr:
            print(incompatible.stdout, incompatible.stderr, file=sys.stderr)
            return 1

        bad_kind = run(str(sav), str(output), "--kind", "raster", "--dry-run")
        if bad_kind.returncode != 2 or "--kind admite" not in bad_kind.stderr:
            print(bad_kind.stdout, bad_kind.stderr, file=sys.stderr)
            return 1

        # Se usa raw válido y semántica que difiere sólo en schema_version.
        # Así se prueba el comportamiento que importa: el orquestador debe
        # devolver 1 y no ejecutar draw después de la primera frontera fallida.
        env = os.environ.copy()
        env.update(fake_exporters(root))
        divergent = run(str(sav), str(output), "--max-diffs", "3", env=env)
        combined = divergent.stdout + divergent.stderr
        semantic_report = output / "semantic" / "report.json"
        if (
            divergent.returncode != 1
            or "PARITY DIFF: primera frontera divergente=semantic" not in combined
            or not semantic_report.is_file()
            or (output / "draw").exists()
        ):
            print(combined, file=sys.stderr)
            return 1
        report = json.loads(semantic_report.read_text(encoding="utf-8"))
        if report["metadata_mismatches"][0]["field"] != "schema_version":
            print(json.dumps(report, ensure_ascii=False, indent=2), file=sys.stderr)
            return 1

    print("OK: compare_sav_world valida CLI y corta en la primera frontera divergente")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
