#!/usr/bin/env python3
"""Compara las muestras diarias del scheduler OpenTTD y openttdrs."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from validate_industry_trace import fail, validate_trace


METADATA_FIELDS = (
    "schema_version",
    "trace",
    "initial_sample_point",
    "day_sample_point",
    "max_days",
    "industry_type_count",
)

# El hook nativo puede adjuntar esta atribución por archivo/línea para ayudar a
# aislar el siguiente producer RNG. No forma parte del JSONL v1 que debe
# emitir el candidato: no hay una correspondencia uno a uno entre los paths
# C++ del oracle y el port Rust. Sólo se tolera en OpenTTD; cualquier extra en
# openttdrs sigue haciendo fallar la comparación.
NATIVE_SAMPLE_DIAGNOSTICS = frozenset(("random_calls",))


def read_rows(path: Path) -> list[dict[str, Any]]:
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer JSONL {path}: {exc}")
    if not rows or any(not isinstance(row, dict) for row in rows):
        fail(f"traza inválida {path}: cada fila debe ser objeto JSON")
    return rows


def first_difference(expected: Any, actual: Any, path: str) -> str | None:
    """Devuelve el primer campo distinto, conservando rutas legibles."""
    if type(expected) is not type(actual):
        return f"{path}: tipo OpenTTD={type(expected).__name__}, openttdrs={type(actual).__name__}"
    if isinstance(expected, dict):
        expected_keys = set(expected)
        actual_keys = set(actual)
        if expected_keys != actual_keys:
            missing = sorted(expected_keys - actual_keys)
            extra = sorted(actual_keys - expected_keys)
            return f"{path}: campos faltantes={missing}, extras={extra}"
        for key in sorted(expected):
            difference = first_difference(expected[key], actual[key], f"{path}.{key}")
            if difference is not None:
                return difference
        return None
    if isinstance(expected, list):
        if len(expected) != len(actual):
            return f"{path}: longitud OpenTTD={len(expected)}, openttdrs={len(actual)}"
        for index, (left, right) in enumerate(zip(expected, actual, strict=True)):
            difference = first_difference(left, right, f"{path}[{index}]")
            if difference is not None:
                return difference
        return None
    if expected != actual:
        return f"{path}: OpenTTD={expected!r}, openttdrs={actual!r}"
    return None


def contractual_sample(row: dict[str, Any], producer: str) -> dict[str, Any]:
    """Descarta diagnósticos explícitamente no contractuales del oracle."""
    if producer != "openttd":
        return row
    return {
        field: value
        for field, value in row.items()
        if field not in NATIVE_SAMPLE_DIAGNOSTICS
    }


def compare(native: Path, candidate: Path, days: int) -> None:
    validate_trace(native, days, "openttd")
    validate_trace(candidate, days, "openttdrs")
    native_rows = read_rows(native)
    candidate_rows = read_rows(candidate)

    native_metadata = native_rows[0]
    candidate_metadata = candidate_rows[0]
    for field in METADATA_FIELDS:
        difference = first_difference(
            native_metadata.get(field), candidate_metadata.get(field), f"metadata.{field}"
        )
        if difference is not None:
            fail(difference)

    native_samples = native_rows[1:]
    candidate_samples = candidate_rows[1:]
    if len(native_samples) != len(candidate_samples):
        fail(
            "cantidad de muestras: "
            f"OpenTTD={len(native_samples)}, openttdrs={len(candidate_samples)}"
        )
    for index, (expected, actual) in enumerate(zip(native_samples, candidate_samples, strict=True)):
        kind = expected.get("kind", f"sample-{index}")
        difference = first_difference(
            contractual_sample(expected, "openttd"),
            contractual_sample(actual, "openttdrs"),
            f"{kind}[{index}]",
        )
        if difference is not None:
            fail(difference)

    print(f"OK: scheduler industrial exacto · {days} días · {native} ↔ {candidate}")


def main() -> None:
    if len(sys.argv) != 4:
        fail(
            "uso: compare_industry_scheduler_traces.py "
            "<openttd.jsonl> <openttdrs.jsonl> <días>"
        )
    native = Path(sys.argv[1])
    candidate = Path(sys.argv[2])
    try:
        days = int(sys.argv[3])
    except ValueError:
        fail(f"días inválidos: {sys.argv[3]}")
    compare(native, candidate, days)


if __name__ == "__main__":
    main()
