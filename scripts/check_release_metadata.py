#!/usr/bin/env python3
"""Valida que versión, notas, lockfile y automatización de release no diverjan."""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"release metadata: {message}")


def main() -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    version = cargo["workspace"]["package"]["version"]
    require("-alpha." in version, f"la versión preparada no es alpha: {version}")

    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    workspace_packages = {
        package["name"]: package["version"]
        for package in lock["package"]
        if package["name"] in {"openttdrs-core", "openttdrs-client", "openttdrs-net"}
    }
    require(len(workspace_packages) == 3, "faltan crates del workspace en Cargo.lock")
    require(
        set(workspace_packages.values()) == {version},
        f"Cargo.lock no coincide con {version}: {workspace_packages}",
    )

    fuzz_lock = tomllib.loads((ROOT / "fuzz" / "Cargo.lock").read_text(encoding="utf-8"))
    fuzz_workspace_packages = {
        package["name"]: package["version"]
        for package in fuzz_lock["package"]
        if package["name"] in {"openttdrs-core", "openttdrs-net"}
    }
    require(
        len(fuzz_workspace_packages) == 2,
        "faltan crates del workspace en fuzz/Cargo.lock",
    )
    require(
        set(fuzz_workspace_packages.values()) == {version},
        f"fuzz/Cargo.lock no coincide con {version}: {fuzz_workspace_packages}",
    )

    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    notes = (ROOT / "RELEASE_NOTES.md").read_text(encoding="utf-8")
    notices = (ROOT / "THIRD_PARTY_ASSETS.md").read_text(encoding="utf-8")
    workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    main_rs = (ROOT / "crates/openttdrs-client/src/main.rs").read_text(encoding="utf-8")
    network_smoke = (
        ROOT / "crates/openttdrs-client/src/network/smoke.rs"
    ).read_text(encoding="utf-8")
    package_smoke = (ROOT / "scripts" / "smoke_release_package.sh").read_text(encoding="utf-8")
    package_builder = (ROOT / "scripts" / "package_release.sh").read_text(encoding="utf-8")
    snap_recipe = (ROOT / "snap" / "snapcraft.yaml").read_text(encoding="utf-8")
    snap_smoke = (ROOT / "scripts" / "smoke_snap_package.sh").read_text(encoding="utf-8")

    snap_version = re.search(r'^version:\s*"([^"]+)"\s*$', snap_recipe, re.MULTILINE)
    require(snap_version is not None, "snapcraft.yaml no declara version")
    require(
        snap_version.group(1) == version,
        f"snapcraft.yaml no coincide con {version}: {snap_version.group(1)}",
    )

    require(f"## [{version}]" in changelog, "falta la versión en CHANGELOG.md")
    require(f"# openttdrs {version}" in notes, "RELEASE_NOTES.md tiene otra versión")
    for asset in ("OpenGFX", "OpenSFX", "OpenMSX", "DejaVu"):
        require(asset in notices, f"falta atribución de {asset}")
    for marker in (
        "tags:",
        '"v*"',
        "--prerelease",
        "package_release.sh",
        "smoke_release_package.sh",
        "check_linux_glibc_floor.sh",
        "write_release_report.py",
    ):
        require(marker in workflow, f"release.yml no contiene {marker!r}")
    require("--check-assets" in main_rs, "el binario no ofrece smoke --check-assets")
    require(
        "parse_handshake_smoke" in main_rs and "--network-smoke" in network_smoke,
        "el binario no ofrece smoke --network-smoke",
    )
    for marker in (
        "assets/shaders/rail_glass_post_process.wgsl",
        "OPENTTDRS_RELEASE_GRAPHICAL_SMOKE",
        "check_release_graphical_smoke.py",
    ):
        require(
            marker in package_smoke,
            f"smoke_release_package.sh no cubre {marker!r}",
        )
    require(
        "assets/shaders/rail_glass_post_process.wgsl" in package_builder,
        "package_release.sh no incluye el shader requerido por el cliente",
    )
    materialize_tiles = 'OPENTTDRS_ASSET_ROOT="$CRAFT_PART_SRC"'
    require(
        materialize_tiles in snap_recipe
        and "target/release/openttdrs-client --check-assets" in snap_recipe,
        "snapcraft.yaml no materializa los tiles derivados antes del empaquetado",
    )
    require(
        snap_recipe.index(materialize_tiles)
        < snap_recipe.index(
            'cp -a "$CRAFT_PART_SRC/assets" "$CRAFT_PART_INSTALL/assets"'
        ),
        "snapcraft.yaml copia assets antes de materializar los tiles",
    )
    for marker in (
        "unsquashfs",
        "openttdrs-launch",
        "chmod -R a-w",
        "SNAP_USER_COMMON",
        "OPENTTDRS_MAIN_MENU_SHOT",
    ):
        require(marker in snap_smoke, f"smoke_snap_package.sh no cubre {marker!r}")

    for script_name in (
        "package_release.sh",
        "smoke_release_package.sh",
        "smoke_snap_package.sh",
        "check_linux_glibc_floor.sh",
    ):
        script = ROOT / "scripts" / script_name
        require(os.access(script, os.X_OK), f"{script_name} no es ejecutable")
        subprocess.run(["bash", "-n", str(script)], check=True, cwd=ROOT)
    print(f"release metadata OK: v{version}")


if __name__ == "__main__":
    main()
