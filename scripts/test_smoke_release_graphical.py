#!/usr/bin/env python3
"""Regresiones del smoke gráfico nativo de paquetes Windows/macOS (#601, #602)."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SMOKE = ROOT / "scripts" / "smoke_release_graphical.py"
sys.path.insert(0, str(ROOT / "scripts"))

from window_visual_regression import PngImage, write_png


def write_file(path: Path, content: str, *, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        path.chmod(path.stat().st_mode | stat.S_IXUSR)


def write_menu(path: Path, offset: int) -> None:
    width, height = 1280, 720
    row = bytearray()
    for x in range(width):
        band = ((x // 16) + offset) % 12
        row.extend(((band * 19) % 256, (band * 41) % 256, (band * 67) % 256, 255))
    write_png(path, PngImage(width, height, bytes(row) * height))


def invoke(root: Path, client: Path, package: Path, archive: Path, artifacts: Path, *, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SMOKE),
            "--client",
            str(client),
            "--package-root",
            str(package),
            "--archive",
            str(archive),
            "--candidate-sha",
            "a" * 40,
            "--artifact-dir",
            str(artifacts),
            "--timeout-seconds",
            "5",
        ],
        cwd=root,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


class NativeGraphicalSmokeTest(unittest.TestCase):
    def test_two_locales_use_only_packaged_assets_and_write_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "openttdrs-package"
            (package / "assets").mkdir(parents=True)
            archive = root / "openttdrs-package.zip"
            archive.write_bytes(b"package fixture")
            frames = root / "frames"
            write_menu(frames / "menu-es.png", 0)
            write_menu(frames / "menu-en.png", 4)
            client = package / "openttdrs-client"
            write_file(
                client,
                "#!/usr/bin/env bash\n"
                "set -eu\n"
                "test \"$OPENTTDRS_ASSET_ROOT\" = \"$FAKE_PACKAGE_ROOT\"\n"
                "test \"$PWD\" != \"$OPENTTDRS_ASSET_ROOT\"\n"
                "test ! -e assets\n"
                "test -n \"${XDG_CONFIG_HOME:-}\"\n"
                "case \"$OPENTTDRS_LANGUAGE\" in es|en) ;; *) exit 7 ;; esac\n"
                "cp \"$FAKE_FRAMES/menu-$OPENTTDRS_LANGUAGE.png\" \"$OPENTTDRS_MAIN_MENU_SHOT\"\n"
                "echo 'main_menu_shot: menú localizado sin escenario guiado listo'\n",
                executable=True,
            )
            artifacts = root / "evidence"
            env = os.environ | {
                "FAKE_PACKAGE_ROOT": str(package),
                "FAKE_FRAMES": str(frames),
            }
            result = invoke(root, client, package, archive, artifacts, env=env)

            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads((artifacts / "graphical-smoke.json").read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "passed")
            self.assertEqual([entry["language"] for entry in report["languages"]], ["es", "en"])
            self.assertTrue(all(entry["status"] == "passed" for entry in report["languages"]))
            self.assertEqual(
                report["package"]["sha256"],
                hashlib.sha256(archive.read_bytes()).hexdigest(),
            )
            for name in ("menu-es.log", "menu-en.log", "menu-es.png", "menu-en.png"):
                self.assertTrue((artifacts / name).is_file(), name)

    def test_missing_capture_is_a_failure_not_a_skip(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "openttdrs-package"
            (package / "assets").mkdir(parents=True)
            archive = root / "openttdrs-package.tar.gz"
            archive.write_bytes(b"package fixture")
            client = package / "openttdrs-client"
            write_file(
                client,
                "#!/usr/bin/env bash\n"
                "echo 'main_menu_shot: menú localizado sin escenario guiado listo'\n",
                executable=True,
            )
            artifacts = root / "evidence"
            result = invoke(root, client, package, archive, artifacts, env=os.environ.copy())

            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            report = json.loads((artifacts / "graphical-smoke.json").read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "failed")
            self.assertTrue(all(entry["status"] == "failed" for entry in report["languages"]))
            self.assertTrue(
                all("no produjo la captura" in entry.get("error", "") for entry in report["languages"])
            )


if __name__ == "__main__":
    unittest.main()
