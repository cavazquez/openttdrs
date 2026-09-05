#!/usr/bin/env python3
"""Contrato del gate OpenTTD 15.3 de release (#294).

Evita que el workflow de release vuelva a aceptar un `SKIP` local: debe
recuperar el binario oficial fijado, ejecutar la matriz estricta y publicar
sus logs/SAV incluso cuando el job falla.
"""

from __future__ import annotations

import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
MATRIX = ROOT / "scripts" / "validate_sav_openttd_matrix.sh"
LOAD = ROOT / "scripts" / "validate_sav_openttd.sh"
ROUNDTRIP = ROOT / "scripts" / "roundtrip_sav_openttd.sh"


def expect(text: str, needle: str, errors: list[str], source: Path) -> None:
    if needle not in text:
        errors.append(f"{source.relative_to(ROOT)}: falta {needle!r}")


def main() -> int:
    errors: list[str] = []
    workflow = WORKFLOW.read_text(encoding="utf-8")
    matrix = MATRIX.read_text(encoding="utf-8")
    load = LOAD.read_text(encoding="utf-8")
    roundtrip = ROUNDTRIP.read_text(encoding="utf-8")

    for needle in (
        "openttd-validation:",
        'OPENTTD_VERSION: "15.3"',
        # The gate requires caching, not a stale action major after Dependabot.
        "uses: actions/cache@",
        "curl --fail --location --retry 3 --retry-all-errors",
        "sha256sum --check --strict",
        'mkdir -p "$RUNNER_TEMP/openttd-validation/baseset"',
        "./scripts/validate_sav_openttd_matrix.sh",
        "OPENTTD: ${{ runner.temp }}/openttd-validation/openttd",
        "name: openttd-15.3-sav-validation",
        "if: always()",
        "path: artifacts/openttd-validation",
        "needs: [validate, build, openttd-validation]",
    ):
        expect(workflow, needle, errors, WORKFLOW)

    for needle in (
        "export OPENTTDRS_REQUIRE_OPENTTD=1",
        "mvp_openttd_load.sav",
        "mvp_openttd_stations.sav",
        "mvp_openttd_train.sav",
        "mvp_openttd_rich.sav",
        "mvp_openttd_ship.sav",
        "demo_openttd.sav",
        "ROUNDTRIP_FIXTURES=(mvp_openttd_rich.sav)",
        "summary.tsv",
        "openttd_version",
    ):
        expect(matrix, needle, errors, MATRIX)

    for script in (LOAD, ROUNDTRIP):
        text = load if script == LOAD else roundtrip
        expect(text, 'OPENTTDRS_REQUIRE_OPENTTD:-0', errors, script)
        expect(text, "OpenTTD requerido pero no encontrado", errors, script)

    if "continue-on-error" in workflow:
        errors.append(f"{WORKFLOW.relative_to(ROOT)}: el gate oficial no puede usar continue-on-error")

    if errors:
        print("FAIL: contrato de validación OpenTTD 15.3", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print("OK: release exige OpenTTD 15.3, matriz SAV sin SKIP y artefactos reproducibles")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
