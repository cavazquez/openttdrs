#!/usr/bin/env python3
"""Run the real documentation gate with and without ripgrep (#333)."""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
CHECKER = Path("scripts/check_parity_docs_fresh.sh")


class ParityDocsPortabilityTest(unittest.TestCase):
    stale_sources = (
        "README.md",
        "docs/parity/continuous-work-plan.md",
        "docs/parity/random-map-issues.md",
        "docs/parity/random-map-matrix.md",
    )

    def run_gate(
        self,
        with_rg,
        stale_path=None,
        stale_text=None,
        corrupt_cutoff_field=None,
        corrupt_cutoff_value=None,
        corrupt_raster_field=None,
    ):
        with tempfile.TemporaryDirectory(prefix="parity-docs-portability-") as directory:
            root = Path(directory)
            checker = (ROOT / CHECKER).read_text()
            scan_paths = re.search(r"SCAN_PATHS=\((.*?)\)", checker, re.S).group(1).split()
            paths = scan_paths + [
                str(CHECKER),
                "scripts/check_active_parity_backlog.py",
                "scripts/check_raster_baseline.py",
                "docs/parity/active-backlog.json",
                "docs/parity/evidence/kale-189-126/baseline-2026-09-05.json",
            ]
            for relative in paths:
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, target)
            if stale_path:
                stale_text = stale_text or "SIM_TICK_HZ = 5.0"
                with (root / stale_path).open("a") as stream:
                    stream.write(f"\n{stale_text}\n")
            if corrupt_cutoff_field:
                manifest_path = root / "docs/parity/active-backlog.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                values = {
                    "main_commit": "no-es-un-hash",
                    "main_commit_role": "",
                    "date": "2026-99-99",
                }
                manifest["cutoff"][corrupt_cutoff_field] = (
                    corrupt_cutoff_value
                    if corrupt_cutoff_value is not None
                    else values[corrupt_cutoff_field]
                )
                manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            if corrupt_raster_field:
                baseline_path = root / "docs/parity/evidence/kale-189-126/baseline-2026-09-05.json"
                baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
                if corrupt_raster_field == "recorded_on":
                    baseline["recorded_on"] = "2026-99-99"
                elif corrupt_raster_field == "candidate_commit":
                    baseline["candidate"]["commit"] = "no-es-un-hash"
                elif corrupt_raster_field == "results":
                    baseline["results"] = []
                baseline_path.write_text(json.dumps(baseline), encoding="utf-8")
            binary_dir = root / "bin"
            binary_dir.mkdir()
            for tool in ["bash", "dirname", "grep", "python3"] + (["rg"] if with_rg else []):
                executable = shutil.which(tool)
                if executable is None:
                    self.skipTest(f"{tool} unavailable")
                (binary_dir / tool).symlink_to(executable)
            result = subprocess.run(
                [str(binary_dir / "bash"), str(root / CHECKER)],
                env={**os.environ, "PATH": str(binary_dir)},
                capture_output=True, text=True, timeout=30,
            )
            output = result.stdout + result.stderr
            self.assertEqual(
                result.returncode,
                1 if stale_path or corrupt_cutoff_field or corrupt_raster_field else 0,
                output,
            )
            if not with_rg:
                self.assertIn("usando grep -E como fallback", output)
            if stale_path:
                self.assertIn(stale_text, output)
            if corrupt_cutoff_field:
                self.assertIn(f"cutoff.{corrupt_cutoff_field}", output)
            if corrupt_raster_field:
                self.assertIn("raster baseline", output)

    def test_clean_docs_with_ripgrep(self):
        self.run_gate(with_rg=True)

    def test_clean_docs_without_ripgrep(self):
        self.run_gate(with_rg=False)

    def test_stale_docs_with_ripgrep(self):
        for stale_path in self.stale_sources:
            with self.subTest(stale_path=stale_path):
                self.run_gate(with_rg=True, stale_path=stale_path)

    def test_stale_docs_without_ripgrep(self):
        for stale_path in self.stale_sources:
            with self.subTest(stale_path=stale_path):
                self.run_gate(with_rg=False, stale_path=stale_path)

    def test_stale_newgrf_claims_are_rejected_with_both_searchers(self):
        stale_claims = (
            ("docs/parity/newgrf-action0-matrix.md", "FTA y callbacks **bloqueados**"),
            (
                "docs/parity/newgrf-action0-matrix.md",
                "AirportTiles/industria todavía usan las rutas legacy",
            ),
            (
                "docs/parity/newgrf-action0-matrix.md",
                "Falta estado independiente por cada tesela de una parada compuesta",
            ),
            (
                "docs/parity/newgrf-action0-matrix.md",
                "Una parada compuesta/importada todavía no conserva estado separado por tesela",
            ),
            (
                "docs/parity/newgrf-action0-matrix.md",
                "Restan vars BaseStation `60`–`65`/`69`",
            ),
            (
                "docs/parity/continuous-work-plan.md",
                "quedan los triggers FTA (carga/descarga/aterrizaje) ya conectados al scheduler",
            ),
            (
                "docs/parity/continuous-work-plan.md",
                "siguen pendientes sus propiedades de capacidad, velocidad, potencia, esfuerzo tractor y costes",
            ),
            (
                "docs/parity/continuous-work-plan.md",
                "round-trip, pero aún no alimentan los scopes ni se invalidan tras mutaciones",
            ),
            (
                "docs/parity/sav-compatibility.md",
                "OBID` se modela y se reconstruye desde el catálogo cuando no hay passthrough",
            ),
            (
                "docs/parity/sav-compatibility.md",
                "todavía no se aplica al cargador de overrides",
            ),
            (
                "docs/PLANIFICACION.md",
                "NGRF`/`GSET`/`ENGN`/`OBJS`/`SRND` y mappings asociados se conservan como passthrough",
            ),
            (
                "docs/PARIDAD.md",
                "PATS`/`OPTS`, `ENGN`, `OBJS`/`OBID` y `SRND` continúan como passthrough o subconjunto",
            ),
            ("docs/PARIDAD.md", "193.939 de 921.600 píxeles distintos"),
            (
                "docs/parity/evidence/kale-189-126/README.md",
                "213.552 de 921.600 píxeles distintos",
            ),
        )
        for with_rg in (True, False):
            for stale_path, stale_text in stale_claims:
                with self.subTest(with_rg=with_rg, stale_text=stale_text):
                    self.run_gate(
                        with_rg=with_rg,
                        stale_path=stale_path,
                        stale_text=stale_text,
                    )

    def test_invalid_cutoff_is_rejected(self):
        for field in ("main_commit", "main_commit_role", "date"):
            with self.subTest(field=field):
                self.run_gate(with_rg=True, corrupt_cutoff_field=field)

    def test_valid_but_undocumented_cutoff_hash_is_rejected(self):
        self.run_gate(
            with_rg=True,
            corrupt_cutoff_field="main_commit",
            corrupt_cutoff_value="deadbeef",
        )

    def test_rmap_parent_closures_are_rejected_with_both_searchers(self):
        stale_claims = (
            "| **RMAP-056** | selección de industrias | **Cerrado** | cierre falso |",
            "| **RMAP-082** | secuencia urbana | **Cerrado** | cierre falso |",
        )
        for with_rg in (True, False):
            for stale_text in stale_claims:
                with self.subTest(with_rg=with_rg, stale_text=stale_text):
                    self.run_gate(
                        with_rg=with_rg,
                        stale_path="docs/parity/random-map-issues.md",
                        stale_text=stale_text,
                    )

    def test_invalid_raster_baseline_is_rejected(self):
        for field in ("recorded_on", "candidate_commit", "results"):
            with self.subTest(field=field):
                self.run_gate(with_rg=True, corrupt_raster_field=field)


if __name__ == "__main__":
    unittest.main()
