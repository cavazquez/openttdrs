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

    def run_gate(self, with_rg, stale_path=None, corrupt_cutoff_field=None):
        with tempfile.TemporaryDirectory(prefix="parity-docs-portability-") as directory:
            root = Path(directory)
            checker = (ROOT / CHECKER).read_text()
            scan_paths = re.search(r"SCAN_PATHS=\((.*?)\)", checker, re.S).group(1).split()
            paths = scan_paths + [str(CHECKER), "scripts/check_active_parity_backlog.py",
                                  "docs/parity/active-backlog.json"]
            for relative in paths:
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, target)
            if stale_path:
                with (root / stale_path).open("a") as stream:
                    stream.write("\nSIM_TICK_HZ = 5.0\n")
            if corrupt_cutoff_field:
                manifest_path = root / "docs/parity/active-backlog.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                values = {
                    "main_commit": "no-es-un-hash",
                    "main_commit_role": "",
                    "date": "2026-99-99",
                }
                manifest["cutoff"][corrupt_cutoff_field] = values[corrupt_cutoff_field]
                manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
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
                1 if stale_path or corrupt_cutoff_field else 0,
                output,
            )
            if not with_rg:
                self.assertIn("usando grep -E como fallback", output)
            if stale_path:
                self.assertIn("SIM_TICK_HZ = 5.0", output)
            if corrupt_cutoff_field:
                self.assertIn(f"cutoff.{corrupt_cutoff_field}", output)

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

    def test_invalid_cutoff_is_rejected(self):
        for field in ("main_commit", "main_commit_role", "date"):
            with self.subTest(field=field):
                self.run_gate(with_rg=True, corrupt_cutoff_field=field)


if __name__ == "__main__":
    unittest.main()
