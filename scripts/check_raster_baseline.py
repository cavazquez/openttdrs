#!/usr/bin/env python3
"""Validate the immutable metadata of the current focused raster baseline (#355)."""

from __future__ import annotations

import json
import re
import sys
from datetime import date as Date
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "docs/parity/evidence/kale-189-126/baseline-2026-09-05.json"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
SCALES = {
    0.25: "In4x",
    0.5: "In2x",
    1.0: "Normal",
    2.0: "Out2x",
    4.0: "Out4x",
    8.0: "Out8x",
}


def valid_sha256(value: object) -> bool:
    return isinstance(value, str) and SHA256.fullmatch(value) is not None


def require_mapping(value: object, name: str, errors: list[str]) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    errors.append(f"{name} debe ser un objeto")
    return {}


def main() -> int:
    try:
        baseline = json.loads(BASELINE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FAIL: no se pudo leer raster baseline: {exc}", file=sys.stderr)
        return 1

    errors: list[str] = []
    if not isinstance(baseline, dict):
        errors.append("la raíz debe ser un objeto")
        baseline = {}
    if baseline.get("schema_version") != 1:
        errors.append("schema_version debe ser 1")
    if baseline.get("kind") != "focused-world-screenshot-baseline":
        errors.append("kind incorrecto")
    if baseline.get("status") != "different":
        errors.append("status debe declarar la diferencia observable")

    recorded_on = baseline.get("recorded_on")
    if not isinstance(recorded_on, str):
        errors.append("recorded_on debe ser una fecha")
    else:
        try:
            Date.fromisoformat(recorded_on)
        except ValueError:
            errors.append("recorded_on debe usar YYYY-MM-DD válido")

    fixture = require_mapping(baseline.get("fixture"), "fixture", errors)
    if fixture.get("path") != "save/Kale_TitleGame.sav":
        errors.append("fixture.path debe identificar Kale_TitleGame.sav")
    if not valid_sha256(fixture.get("sha256")):
        errors.append("fixture.sha256 debe ser SHA-256")

    reference = require_mapping(baseline.get("reference"), "reference", errors)
    if reference.get("openttd_version") != "15.3":
        errors.append("reference.openttd_version debe ser 15.3")
    for key in ("official_commit", "oracle_commit"):
        value = reference.get(key)
        if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{40}", value):
            errors.append(f"reference.{key} debe ser un hash Git de 40 hexadecimales")
    if not isinstance(reference.get("oracle_worktree"), str) or not reference["oracle_worktree"].strip():
        errors.append("reference.oracle_worktree debe describir la instrumentación")

    candidate = require_mapping(baseline.get("candidate"), "candidate", errors)
    candidate_commit = candidate.get("commit")
    if not isinstance(candidate_commit, str) or not re.fullmatch(r"[0-9a-f]{40}", candidate_commit):
        errors.append("candidate.commit debe ser un hash Git de 40 hexadecimales")

    capture = require_mapping(baseline.get("capture"), "capture", errors)
    if capture.get("profile") != "clean-static":
        errors.append("capture.profile debe ser clean-static")
    if capture.get("center") != [189, 126]:
        errors.append("capture.center debe ser [189, 126]")
    if capture.get("resolution") != [1280, 720]:
        errors.append("capture.resolution debe ser [1280, 720]")
    if capture.get("alignment_radius_px") != 8:
        errors.append("capture.alignment_radius_px debe ser 8")
    if capture.get("hotspot_cell_size_px") != 64:
        errors.append("capture.hotspot_cell_size_px debe ser 64")

    results = baseline.get("results")
    if not isinstance(results, list):
        errors.append("results debe ser una lista")
        results = []
    found: dict[float, dict[str, Any]] = {}
    for index, result in enumerate(results):
        result = require_mapping(result, f"results[{index}]", errors)
        scale = result.get("scale")
        if not isinstance(scale, (int, float)) or float(scale) not in SCALES:
            errors.append(f"results[{index}].scale no es un zoom soportado")
            continue
        scale = float(scale)
        if scale in found:
            errors.append(f"results repite escala {scale}")
        found[scale] = result
        if result.get("openttd_zoom") != SCALES[scale]:
            errors.append(f"results[{index}].openttd_zoom no coincide con la escala")
        translation = result.get("candidate_translation")
        if (
            not isinstance(translation, list)
            or len(translation) != 2
            or not all(isinstance(value, int) for value in translation)
        ):
            errors.append(f"results[{index}].candidate_translation debe tener dos enteros")
        changed = result.get("changed_pixels")
        total = result.get("total_pixels")
        ratio = result.get("changed_ratio")
        if not isinstance(changed, int) or not isinstance(total, int) or total <= 0 or not 0 <= changed <= total:
            errors.append(f"results[{index}] tiene conteos de píxeles inválidos")
        elif not isinstance(ratio, (int, float)) or abs(float(ratio) - changed / total) > 1e-12:
            errors.append(f"results[{index}].changed_ratio no coincide con los conteos")
        artifacts = require_mapping(result.get("artifacts"), f"results[{index}].artifacts", errors)
        for key in ("reference_sha256", "candidate_sha256", "diff_sha256"):
            if not valid_sha256(artifacts.get(key)):
                errors.append(f"results[{index}].artifacts.{key} debe ser SHA-256")

    if set(found) != set(SCALES):
        errors.append("results debe cubrir exactamente In4x, In2x, Normal, Out2x, Out4x y Out8x")
    elif found[1.0].get("candidate_translation") != [0, 0]:
        errors.append("la fila Normal del baseline debe registrar alineación [0, 0]")

    if errors:
        print("FAIL: raster baseline inválido", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("OK: raster baseline documentado con fixture, pins, candidata y seis zooms")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
