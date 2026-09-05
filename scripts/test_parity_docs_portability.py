#!/usr/bin/env python3
"""Run the real documentation gate with and without ripgrep (#333)."""

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
    def run_gate(self, with_rg, stale):
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
            if stale:
                with (root / "README.md").open("a") as stream:
                    stream.write("\nSIM_TICK_HZ = 5.0\n")
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
            self.assertEqual(result.returncode, 1 if stale else 0, output)
            if not with_rg:
                self.assertIn("usando grep -E como fallback", output)
            if stale:
                self.assertIn("SIM_TICK_HZ = 5.0", output)

    def test_clean_docs_with_ripgrep(self):
        self.run_gate(with_rg=True, stale=False)

    def test_clean_docs_without_ripgrep(self):
        self.run_gate(with_rg=False, stale=False)

    def test_stale_docs_with_ripgrep(self):
        self.run_gate(with_rg=True, stale=True)

    def test_stale_docs_without_ripgrep(self):
        self.run_gate(with_rg=False, stale=True)


if __name__ == "__main__":
    unittest.main()
