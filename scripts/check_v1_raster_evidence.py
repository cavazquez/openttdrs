#!/usr/bin/env python3
"""Comprueba que la evidencia versionada del contrato V1-RAS siga íntegra."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/parity/evidence/kale-132-2/2026-09-20-1f170b07"
REPORT = EVIDENCE / "v1-raster-report.json"
CANDIDATE_COMMIT = "1f170b076ce4f79c961082277cf5a35877e5d930"
EXPECTED_CHECKS = {
    "changed_pixels": (437, 480),
    "pixels_over_channel_delta_64": (5, 24),
    "mean_absolute_rgba_channel_delta": (0.021086979166666665, 0.05),
    "outside_candidate_pixels": (0, 0),
}
EXPECTED_SCALES = [0.25, 0.5, 2.0, 4.0, 8.0]
PROVENANCE_SHA256 = {
    "save/Kale_TitleGame.sav": "584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69",
    "assets/opengfx/.graphics_mode": "b61e2c5b6fb62eb30de3d28f99c38e8b9ab73ff32a264366103a46fd24d719d3",
    "assets/opengfx/opengfx-8.0/ogfx1_base.grf": "f8201e87c7210493aaf525993013dc94dd4b7a801c4bb27df47544786bf8eaa9",
    "assets/opengfx/atlas/tiles_atlas_0.png": "cbb898a033c2dc95920c1ee10a3e1152bc95019f7abf2afd25aca9fc5dc8ce41",
    "assets/opengfx/.signal-src-8bpp/extract/opengfx-8.0/opengfx.obg": "e3ce8169ea9cf0fab624ad8b9b940684f3dfdf189f88bc79bbe62064fd3496d0",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def check_artifacts(value: Any, errors: list[str]) -> None:
    if isinstance(value, dict):
        path_value = value.get("path")
        expected_sha = value.get("sha256")
        if path_value is not None or expected_sha is not None:
            require(isinstance(path_value, str), "artefacto sin path", errors)
            require(isinstance(expected_sha, str), "artefacto sin sha256", errors)
            if isinstance(path_value, str) and isinstance(expected_sha, str):
                path = Path(path_value)
                require(not path.is_absolute(), f"ruta temporal/absoluta en evidencia: {path}", errors)
                target = ROOT / path
                try:
                    target.resolve().relative_to(EVIDENCE.resolve())
                except ValueError:
                    require(
                        PROVENANCE_SHA256.get(path.as_posix()) == expected_sha,
                        f"proveniencia no permitida o SHA distinto: {path}",
                        errors,
                    )
                else:
                    require(target.is_file(), f"falta artefacto: {path}", errors)
                    if target.is_file():
                        require(
                            sha256(target) == expected_sha,
                            f"SHA distinto para {path}",
                            errors,
                        )
        for child in value.values():
            check_artifacts(child, errors)
    elif isinstance(value, list):
        for child in value:
            check_artifacts(child, errors)


def main() -> int:
    errors: list[str] = []
    try:
        report = json.loads(REPORT.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FAIL: no se pudo leer evidencia V1-RAS: {exc}", file=sys.stderr)
        return 1
    if not isinstance(report, dict):
        print("FAIL: evidencia V1-RAS no es un objeto", file=sys.stderr)
        return 1

    require(report.get("schema_version") == 1, "schema_version debe ser 1", errors)
    require(report.get("kind") == "v1-raster-gate", "kind V1-RAS incorrecto", errors)
    require(report.get("status") == "passed", "la evidencia V1-RAS debe pasar", errors)
    pins = report.get("pins")
    require(isinstance(pins, dict), "faltan pins", errors)
    if isinstance(pins, dict):
        require(pins.get("candidate_commit") == CANDIDATE_COMMIT, "SHA candidata inesperada", errors)
        require(
            pins.get("openttd_official_commit") == "14ec60f248547d4d062a1160f0fc26d742319888",
            "pin oficial OpenTTD inesperado",
            errors,
        )
        require(
            pins.get("openttd_oracle_commit") == "c2661164bcb6cbf5ab97b56ccbee7506a3b26833",
            "pin del oracle OpenTTD inesperado",
            errors,
        )
    contract = report.get("contract")
    require(isinstance(contract, dict), "falta contrato", errors)
    if isinstance(contract, dict):
        require(contract.get("center") == [132, 2], "centro V1 incorrecto", errors)
        require(contract.get("resolution") == [800, 600], "resolución V1 incorrecta", errors)
        require(contract.get("profile") == "clean-static", "perfil V1 incorrecto", errors)
        require(contract.get("candidate_settle_frames") == 360, "settle V1 debe ser 360", errors)
        require(contract.get("sample_count_per_engine") == 3, "V1 exige tres muestras", errors)
        require(
            contract.get("alignment")
            == {
                "adaptive_crop": False,
                "candidate_translation": [0, 0],
                "masks": False,
                "search_radius_px": 0,
            },
            "V1 no permite alinear, máscaras ni recortes",
            errors,
        )
    normal = report.get("normal")
    require(isinstance(normal, dict), "falta resultado Normal", errors)
    if isinstance(normal, dict):
        checks = normal.get("checks")
        require(isinstance(checks, dict), "faltan checks Normal", errors)
        if isinstance(checks, dict):
            for name, (actual, maximum) in EXPECTED_CHECKS.items():
                check = checks.get(name)
                require(isinstance(check, dict), f"falta check {name}", errors)
                if isinstance(check, dict):
                    require(check.get("actual") == actual, f"resultado {name} inesperado", errors)
                    require(check.get("maximum") == maximum, f"límite {name} inesperado", errors)
                    require(check.get("passed") is True, f"check {name} no pasa", errors)
        samples = normal.get("samples")
        require(isinstance(samples, dict), "faltan muestras Normal", errors)
        if isinstance(samples, dict):
            for engine in ("reference", "candidate"):
                entries = samples.get(engine)
                require(isinstance(entries, list) and len(entries) == 3, f"faltan tres muestras {engine}", errors)
                if isinstance(entries, list) and len(entries) == 3:
                    hashes = {entry.get("sha256") for entry in entries if isinstance(entry, dict)}
                    require(len(hashes) == 1 and None not in hashes, f"muestras {engine} no son hash-idénticas", errors)
    diagnostics = report.get("diagnostics")
    require(isinstance(diagnostics, list), "faltan diagnósticos", errors)
    if isinstance(diagnostics, list):
        scales = [entry.get("scale") for entry in diagnostics if isinstance(entry, dict)]
        require(scales == EXPECTED_SCALES, "las cinco escalas diagnósticas no coinciden", errors)

    check_artifacts(report, errors)
    if errors:
        print("FAIL: evidencia V1-RAS inválida", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("OK: evidencia V1-RAS íntegra, hash-idéntica y dentro del presupuesto")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
