#!/usr/bin/env python3
"""Contrato y smoke aislado del dry-run multiplataforma de release (#296, #577, #601, #602).

El workflow debe construir artefactos de los tres sistemas, probar el paquete
ya extraído y impedir una publicación por tag sin los gates previos. Además de
la comprobación estática, este test ejecuta el smoke contra un paquete mínimo
para cubrir extracción, assets, audio y el protocolo de cliente/dedicated.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
SMOKE = ROOT / "scripts" / "smoke_release_package.sh"
REPORT = ROOT / "scripts" / "write_release_report.py"
APT_PACKAGES = ROOT / ".github" / "apt-packages.txt"
sys.path.insert(0, str(ROOT / "scripts"))

from window_visual_regression import PngImage, write_png  # noqa: E402


def write_file(path: Path, content: str, *, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        path.chmod(path.stat().st_mode | stat.S_IXUSR)


def write_graphical_frame(path: Path, offset: int) -> None:
    width, height = 1280, 720
    row = bytearray()
    for x in range(width):
        band = ((x // 16) + offset) % 12
        row.extend(((band * 19) % 256, (band * 41) % 256, (band * 67) % 256, 255))
    write_png(path, PngImage(width, height, bytes(row) * height))


class ReleaseDryRunWorkflowTest(unittest.TestCase):
    def test_workflow_keeps_three_platform_dry_run_and_tag_gates(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        for marker in (
            "workflow_dispatch:",
            "fail-fast: false",
            "- os: ubuntu-22.04",
            "target: x86_64-unknown-linux-gnu",
            "platform: linux-x86_64",
            'glibc_floor: "2.35"',
            "- os: windows-2025",
            "target: x86_64-pc-windows-msvc",
            "platform: windows-x86_64",
            "- os: macos-15",
            "target: aarch64-apple-darwin",
            "platform: macos-arm64",
            "openttdrs-client --bin openttdrs-client",
            "openttdrs-net --bin openttdrs-dedicated",
            "./scripts/package_release.sh",
            "./scripts/check_linux_glibc_floor.sh",
            "./scripts/smoke_release_package.sh",
            "Smoke gráfico nativo del paquete extraído",
            'OPENTTDRS_RELEASE_GRAPHICAL_SMOKE: "1"',
            'OPENTTDRS_RELEASE_GRAPHICAL_TIMEOUT_SECONDS: "60"',
            "OPENTTDRS_RELEASE_CANDIDATE_SHA: ${{ github.sha }}",
            "OPENTTDRS_RELEASE_SMOKE_ARTIFACT_DIR",
            "release-${{ matrix.platform }}-graphics-smoke-",
            "scripts/write_release_report.py",
            "release-report-${{ needs.validate.outputs.version }}-${{ matrix.platform }}.json",
            "if: github.ref_type == 'tag'",
            "needs: [validate, build, openttd-validation]",
            '"check"',
            '"cargo check (macos-latest)"',
            '"cargo check (windows-latest)"',
            "gh release create",
            "mapfile -t release_assets",
            "find dist -maxdepth 1 -type f",
        ):
            self.assertIn(marker, workflow)
        self.assertNotIn("continue-on-error", workflow)
        self.assertNotIn('gh release upload "$GITHUB_REF_NAME" dist/*', workflow)
        self.assertNotIn('gh release create "$GITHUB_REF_NAME" dist/*', workflow)
        smoke = SMOKE.read_text(encoding="utf-8")
        for marker in (
            "xvfb-run",
            "OPENTTDRS_ASSET_ROOT",
            "outside-checkout",
            "XDG_CONFIG_HOME",
            "VK_ICD_FILENAMES",
            "lvp_icd*.json",
            "check_release_graphical_smoke.py",
            "smoke_release_graphical.py",
            "package.sha256",
        ):
            self.assertIn(marker, smoke)
        apt_packages = set(APT_PACKAGES.read_text(encoding="utf-8").split())
        self.assertTrue(
            {"mesa-vulkan-drivers", "xauth", "xvfb", "libxkbcommon-x11-0"}
            <= apt_packages
        )

    def test_smoke_accepts_a_complete_extracted_package_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "openttdrs-0.0.0-linux-x86_64"
            write_file(
                package / "openttdrs-client",
                "#!/usr/bin/env bash\n"
                "case \"${1:-}\" in\n"
                "  --check-assets) exit 0 ;;\n"
                "  --network-smoke) test -n \"${2:-}\"; exit 0 ;;\n"
                "  *) exit 64 ;;\n"
                "esac\n",
                executable=True,
            )
            write_file(
                package / "openttdrs-dedicated",
                "#!/usr/bin/env bash\nsleep 60\n",
                executable=True,
            )
            for relative in (
                "static/fonts/DejaVuSansMono.ttf",
                "assets/shaders/rail_glass_post_process.wgsl",
                "assets/opengfx/tiles/grass.png",
                "assets/opengfx/atlas/tiles_atlas_0.png",
                "assets/music/test.ogg",
                "assets/sounds/test.wav",
            ):
                write_file(package / relative, "fixture")

            archive = root / f"{package.name}.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                output.add(package, arcname=package.name)

            result = subprocess.run(
                ["bash", str(SMOKE), str(archive)],
                cwd=ROOT,
                env=os.environ | {"OPENTTDRS_RELEASE_SMOKE_PORT": "39001"},
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("Smoke de paquete OK", result.stdout)

    def test_smoke_rejects_missing_runtime_assets(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "openttdrs-0.0.0-missing-assets"
            write_file(package / "openttdrs-client", "#!/usr/bin/env bash\nexit 0\n", executable=True)
            write_file(package / "openttdrs-dedicated", "#!/usr/bin/env bash\nexit 0\n", executable=True)
            archive = root / f"{package.name}.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                output.add(package, arcname=package.name)

            result = subprocess.run(
                ["bash", str(SMOKE), str(archive)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Falta o está vacío", result.stderr)

    def test_graphical_smoke_uses_package_assets_and_keeps_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "openttdrs-0.0.0-linux-x86_64"
            fixture_dir = root / "fixtures"
            menu = fixture_dir / "menu.png"
            write_graphical_frame(menu, 0)
            write_file(
                package / "openttdrs-client",
                "#!/usr/bin/env bash\n"
                "set -eu\n"
                "case \"${1:-}\" in\n"
                "  --check-assets) exit 0 ;;\n"
                "  --network-smoke) test -n \"${2:-}\"; exit 0 ;;\n"
                "esac\n"
                "test \"$PWD\" != \"$OPENTTDRS_ASSET_ROOT\"\n"
                "test ! -e assets\n"
                "test ! -e reference\n"
                "test -f \"$OPENTTDRS_ASSET_ROOT/assets/opengfx/tiles/grass.png\"\n"
                "test -n \"${XDG_CONFIG_HOME:-}\"\n"
                "if [[ -n \"${OPENTTDRS_MAIN_MENU_SHOT:-}\" ]]; then\n"
                "  cp \"$MOCK_MENU\" \"$OPENTTDRS_MAIN_MENU_SHOT\"\n"
                "  echo 'main_menu_shot: menú localizado sin escenario guiado listo'\n"
                "  exit 0\n"
                "fi\n"
                "exit 64\n",
                executable=True,
            )
            write_file(
                package / "openttdrs-dedicated",
                "#!/usr/bin/env bash\nsleep 60\n",
                executable=True,
            )
            for relative in (
                "static/fonts/DejaVuSansMono.ttf",
                "assets/shaders/rail_glass_post_process.wgsl",
                "assets/opengfx/tiles/grass.png",
                "assets/opengfx/atlas/tiles_atlas_0.png",
                "assets/music/test.ogg",
                "assets/sounds/test.wav",
            ):
                write_file(package / relative, "fixture")

            tools = root / "tools"
            write_file(
                tools / "xvfb-run",
                "#!/usr/bin/env bash\n"
                "set -eu\n"
                "while [[ $# -gt 0 ]]; do\n"
                "  case \"$1\" in\n"
                "    -a) shift ;;\n"
                "    -s) shift 2 ;;\n"
                "    *) break ;;\n"
                "  esac\n"
                "done\n"
                "exec \"$@\"\n",
                executable=True,
            )
            write_file(tools / "xauth", "#!/usr/bin/env bash\nexit 0\n", executable=True)
            archive = root / f"{package.name}.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                output.add(package, arcname=package.name)

            artifact_dir = root / "evidence"
            env = os.environ | {
                "PATH": f"{tools}:{os.environ['PATH']}",
                "MOCK_MENU": str(menu),
                "OPENTTDRS_RELEASE_GRAPHICAL_SMOKE": "1",
                "OPENTTDRS_RELEASE_SMOKE_ARTIFACT_DIR": str(artifact_dir),
                "OPENTTDRS_RELEASE_SMOKE_PORT": "39002",
            }
            result = subprocess.run(
                ["bash", str(SMOKE), str(archive)],
                cwd=root,
                env=env,
                text=True,
                capture_output=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("Smoke gráfico Linux OK", result.stdout)
            for name in (
                "package.sha256",
                "check-assets.log",
                "dedicated.log",
                "network.log",
                "menu.log",
                "menu.png",
            ):
                self.assertTrue((artifact_dir / name).is_file(), name)
            self.assertIn(
                hashlib.sha256(archive.read_bytes()).hexdigest(),
                (artifact_dir / "package.sha256").read_text(encoding="utf-8"),
            )

    def test_report_binds_metadata_hashes_and_smoke_results(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "package.tar.gz"
            client = root / "openttdrs-client"
            dedicated = root / "openttdrs-dedicated"
            for path, content in (
                (archive, b"archive"),
                (client, b"client"),
                (dedicated, b"dedicated"),
            ):
                path.write_bytes(content)
            report_path = root / "release-report.json"
            subprocess.run(
                [
                    sys.executable,
                    str(REPORT),
                    "--version",
                    "0.1.0-alpha.1",
                    "--source-sha",
                    "f" * 40,
                    "--platform",
                    "linux-x86_64",
                    "--archive",
                    str(archive),
                    "--client",
                    str(client),
                    "--dedicated",
                    str(dedicated),
                    "--glibc-baseline",
                    "2.35",
                    "--output",
                    str(report_path),
                ],
                cwd=ROOT,
                check=True,
            )
            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["version"], "0.1.0-alpha.1")
            self.assertEqual(report["source_sha"], "f" * 40)
            self.assertEqual(report["platform"], "linux-x86_64")
            self.assertEqual(report["linux_glibc_baseline"], "2.35")
            self.assertEqual(
                report["hashes"]["archive"]["sha256"],
                hashlib.sha256(b"archive").hexdigest(),
            )
            self.assertEqual(set(report["smokes"].values()), {"passed"})


if __name__ == "__main__":
    unittest.main()
