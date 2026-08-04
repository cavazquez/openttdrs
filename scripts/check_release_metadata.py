#!/usr/bin/env python3
"""Valida que versión, notas, lockfile y automatización de release no diverjan."""

from __future__ import annotations

import os
from pathlib import Path
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

    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    notes = (ROOT / "RELEASE_NOTES.md").read_text(encoding="utf-8")
    notices = (ROOT / "THIRD_PARTY_ASSETS.md").read_text(encoding="utf-8")
    workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
    main_rs = (ROOT / "crates/openttdrs-client/src/main.rs").read_text(encoding="utf-8")
    network_smoke = (
        ROOT / "crates/openttdrs-client/src/network/smoke.rs"
    ).read_text(encoding="utf-8")

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

    for script_name in (
        "package_release.sh",
        "smoke_release_package.sh",
        "check_linux_glibc_floor.sh",
    ):
        script = ROOT / "scripts" / script_name
        require(os.access(script, os.X_OK), f"{script_name} no es ejecutable")
        subprocess.run(["bash", "-n", str(script)], check=True, cwd=ROOT)
    print(f"release metadata OK: v{version}")


if __name__ == "__main__":
    main()
