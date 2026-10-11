#!/usr/bin/env python3
"""Reject a swapped atlas before deriving tiles with the compiled lookup."""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class PrepareReleaseAssetsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        script = self.repo / "scripts/prepare_release_assets.sh"
        script.parent.mkdir()
        shutil.copy2(ROOT / "scripts/prepare_release_assets.sh", script)
        self.script = script
        self.atlas = self.repo / "assets/opengfx/atlas/tiles_atlas_0.png"
        self.atlas.parent.mkdir(parents=True)
        self.atlas.write_bytes(b"versioned atlas")
        self.table = self.repo / "crates/openttdrs-client/src/sprites/tile_atlas_generated.rs"
        self.table.parent.mkdir(parents=True)
        self.table.write_text("versioned lookup\n")
        self.client = self.repo / "client"
        self.client.write_text(
            '#!/usr/bin/env bash\nset -eu\n'
            'test "$1" = --check-assets\n'
            'test "$PWD" = "$OPENTTDRS_ASSET_ROOT"\n'
            'touch invoked\nmkdir -p assets/opengfx/tiles\n'
            'printf tile > assets/opengfx/tiles/grass.png\n'
            'printf tile > assets/opengfx/tiles/vehicle_train_e.png\n'
        )
        self.client.chmod(0o755)
        env = os.environ | {
            "GIT_AUTHOR_NAME": "Release fixture", "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
            "GIT_COMMITTER_NAME": "Release fixture", "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
        }
        for args in (["init", "--quiet"], ["add", "."], ["commit", "--quiet", "-m", "fixture"]):
            subprocess.run(["git", *args], cwd=self.repo, env=env, check=True, capture_output=True)

    def prepare(self):
        return subprocess.run(
            ["bash", str(self.script), str(self.client)], cwd=self.repo / "scripts",
            text=True, capture_output=True, timeout=10,
        )

    def test_materializes_missing_tiles_without_changing_atlas(self):
        result = self.prepare()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((self.repo / "invoked").is_file())
        self.assertEqual(self.atlas.read_bytes(), b"versioned atlas")
        self.assertTrue((self.repo / "assets/opengfx/tiles/grass.png").is_file())

    def test_rejects_other_atlas_before_running_client(self):
        self.atlas.write_bytes(b"regenerated atlas from another graphics mode")
        result = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.repo / "invoked").exists())

    def test_rejects_changed_lookup_before_running_client(self):
        self.table.write_text("different lookup\n")
        result = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.repo / "invoked").exists())

    def test_rejects_incomplete_materialization(self):
        self.client.write_text('#!/usr/bin/env bash\nexit 0\n')
        result = self.prepare()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Falta tile materializado", result.stderr)


if __name__ == "__main__":
    unittest.main()
