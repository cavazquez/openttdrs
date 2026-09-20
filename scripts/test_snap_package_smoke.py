#!/usr/bin/env python3
"""Contrato del build y smoke read-only del artefacto Snap (#582, #583)."""

from __future__ import annotations

import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SNAPCRAFT = ROOT / "snap" / "snapcraft.yaml"
LAUNCHER = ROOT / "snap" / "local" / "openttdrs-launch"
SMOKE = ROOT / "scripts" / "smoke_snap_package.sh"
sys.path.insert(0, str(ROOT / "scripts"))

from window_visual_regression import PngImage, write_png  # noqa: E402


def write_file(path: Path, content: str, *, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        path.chmod(path.stat().st_mode | stat.S_IXUSR)


def write_menu(path: Path) -> None:
    width, height = 1280, 720
    row = bytearray()
    for x in range(width):
        band = (x // 16) % 12
        row.extend(((band * 19) % 256, (band * 41) % 256, (band * 67) % 256, 255))
    write_png(path, PngImage(width, height, bytes(row) * height))


def build_fake_snap(root: Path, *, include_grass: bool) -> None:
    write_file(
        root / "bin" / "openttdrs-client",
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "test \"$OPENTTDRS_ASSET_ROOT\" = \"$SNAP\"\n"
        "test \"$SNAP_USER_COMMON\" != \"$SNAP\"\n"
        "test -d \"$SNAP_USER_COMMON/save\"\n"
        "case \"${1:-}\" in\n"
        "  --check-assets)\n"
        "    test -f \"$SNAP/assets/opengfx/tiles/grass.png\"\n"
        "    echo \"Assets OK: $OPENTTDRS_ASSET_ROOT\"\n"
        "    ;;\n"
        "  *)\n"
        "    test -n \"${OPENTTDRS_MAIN_MENU_SHOT:-}\"\n"
        "    cp \"$MOCK_MENU\" \"$OPENTTDRS_MAIN_MENU_SHOT\"\n"
        "    echo 'main_menu_shot: menú localizado sin escenario guiado listo'\n"
        "    ;;\n"
        "esac\n",
        executable=True,
    )
    launcher = root / "bin" / "openttdrs-launch"
    launcher.parent.mkdir(parents=True, exist_ok=True)
    launcher.write_text(LAUNCHER.read_text(encoding="utf-8"), encoding="utf-8")
    launcher.chmod(launcher.stat().st_mode | stat.S_IXUSR)

    for relative in (
        "static/fonts/DejaVuSansMono.ttf",
        "assets/shaders/rail_glass_post_process.wgsl",
        "assets/opengfx/atlas/tiles_atlas_0.png",
        "assets/music/test.ogg",
        "assets/sounds/test.wav",
    ):
        write_file(root / relative, "fixture")
    if include_grass:
        write_file(root / "assets/opengfx/tiles/grass.png", "fixture")
    write_file(root / "assets/linked-target", "fixture")
    (root / "assets/linked-asset").symlink_to("linked-target")


def fake_toolchain(root: Path) -> Path:
    tools = root / "tools"
    write_file(
        tools / "unsquashfs",
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "test \"$1\" = -q\n"
        "test \"$2\" = -d\n"
        "mkdir -p \"$3\"\n"
        "cp -a \"$FAKE_SNAP_SOURCE/.\" \"$3/\"\n",
        executable=True,
    )
    write_file(
        tools / "xvfb-run",
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
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
    return tools


class SnapPackageSmokeTest(unittest.TestCase):
    def test_snapcraft_materializes_tiles_before_copying_assets(self) -> None:
        recipe = SNAPCRAFT.read_text(encoding="utf-8")
        materialize = (
            'OPENTTDRS_ASSET_ROOT="$CRAFT_PART_SRC" \\\n'
            "        target/release/openttdrs-client --check-assets"
        )
        self.assertIn(materialize, recipe)
        copy_assets = 'cp -a "$CRAFT_PART_SRC/assets" "$CRAFT_PART_INSTALL/assets"'
        self.assertIn(copy_assets, recipe)
        self.assertLess(recipe.index(materialize), recipe.index(copy_assets))

    def test_smoke_exercises_launcher_with_read_only_assets_and_fresh_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fake_snap = root / "fake.snap"
            fake_snap.write_bytes(b"fixture")
            snap_source = root / "snap-source"
            build_fake_snap(snap_source, include_grass=True)
            tools = fake_toolchain(root)
            menu = root / "menu.png"
            write_menu(menu)

            env = os.environ | {
                "PATH": f"{tools}:{os.environ['PATH']}",
                "FAKE_SNAP_SOURCE": str(snap_source),
                "MOCK_MENU": str(menu),
                "OPENTTDRS_ASSET_ROOT": "",
            }
            # El script busca Lavapipe en la ruta normal. El test evita tocar
            # el host y sólo cubre el contrato de extracción/launcher; si el
            # host no tiene Lavapipe, se salta a la prueba estática de receta.
            if not list(Path("/usr/share/vulkan/icd.d").glob("lvp_icd*.json")):
                self.skipTest("Lavapipe no está instalado en este host")
            result = subprocess.run(
                ["bash", str(SMOKE), str(fake_snap)],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("Smoke Snap OK", result.stdout)

    def test_smoke_rejects_a_snap_without_derived_tiles(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fake_snap = root / "missing.snap"
            fake_snap.write_bytes(b"fixture")
            snap_source = root / "snap-source"
            build_fake_snap(snap_source, include_grass=False)
            tools = fake_toolchain(root)
            env = os.environ | {
                "PATH": f"{tools}:{os.environ['PATH']}",
                "FAKE_SNAP_SOURCE": str(snap_source),
                "OPENTTDRS_ASSET_ROOT": "",
            }
            result = subprocess.run(
                ["bash", str(SMOKE), str(fake_snap)],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                timeout=30,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Falta o está vacío", result.stderr)

    def test_shell_scripts_are_syntax_valid_and_executable(self) -> None:
        self.assertTrue(os.access(SMOKE, os.X_OK), "smoke_snap_package.sh debe ser ejecutable")
        subprocess.run(["bash", "-n", str(SMOKE)], cwd=ROOT, check=True)


if __name__ == "__main__":
    unittest.main()
