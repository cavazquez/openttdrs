#!/usr/bin/env python3
"""Valida el corte y backlog activo de paridad (#292, #352)."""

from __future__ import annotations

import json
import re
import sys
from datetime import date as Date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PARIDAD.md"
MANIFEST = ROOT / "docs" / "parity" / "active-backlog.json"
START = "<!-- active-parity-backlog:start -->"
END = "<!-- active-parity-backlog:end -->"
SHA_PATTERN = re.compile(r"[0-9a-f]{7,40}\Z")
DATE_PATTERN = re.compile(r"\d{4}-\d{2}-\d{2}\Z")


def validate_cutoff(manifest: object, text: str) -> list[str]:
    """Comprueba una procedencia reproducible sin pedir que se auto-referencie."""
    if not isinstance(manifest, dict):
        return ["manifiesto raíz inválido"]
    cutoff = manifest.get("cutoff")
    if not isinstance(cutoff, dict):
        return ["falta objeto cutoff en docs/parity/active-backlog.json"]

    errors: list[str] = []
    date = cutoff.get("date")
    main_commit = cutoff.get("main_commit")
    commit_role = cutoff.get("main_commit_role")
    openttd_tag = cutoff.get("openttd_tag")
    openttd_commit = cutoff.get("openttd_commit")
    if not isinstance(date, str) or not DATE_PATTERN.fullmatch(date):
        errors.append("cutoff.date debe usar YYYY-MM-DD")
    else:
        try:
            Date.fromisoformat(date)
        except ValueError:
            errors.append("cutoff.date debe ser una fecha calendario válida")
    if not isinstance(main_commit, str) or not SHA_PATTERN.fullmatch(main_commit):
        errors.append("cutoff.main_commit debe ser un hash Git de 7 a 40 hexadecimales")
    if not isinstance(commit_role, str) or not commit_role.strip():
        errors.append("cutoff.main_commit_role debe explicar la política no autorreferencial")
    if not isinstance(openttd_tag, str) or not re.fullmatch(r"\d+\.\d+(?:\.\d+)?", openttd_tag):
        errors.append("cutoff.openttd_tag debe ser una versión OpenTTD")
    if not isinstance(openttd_commit, str) or not SHA_PATTERN.fullmatch(openttd_commit):
        errors.append("cutoff.openttd_commit debe ser un hash Git de 7 a 40 hexadecimales")

    if not errors:
        assert isinstance(date, str)
        assert isinstance(main_commit, str)
        assert isinstance(openttd_tag, str)
        assert isinstance(openttd_commit, str)
        if date not in text:
            errors.append(f"docs/PARIDAD.md no declara cutoff.date={date}")
        if f"`{main_commit}`" not in text:
            errors.append(f"docs/PARIDAD.md no declara cutoff.main_commit={main_commit}")
        if openttd_tag not in text or openttd_commit not in text:
            errors.append("docs/PARIDAD.md no declara el pin OpenTTD del cutoff")
    return errors


def main() -> int:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    text = DOC.read_text(encoding="utf-8")
    errors = validate_cutoff(manifest, text)
    if errors:
        print("FAIL: cutoff canónico inválido", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    try:
        active_block = text.split(START, 1)[1].split(END, 1)[0]
    except IndexError:
        print("FAIL: falta bloque delimitado de backlog activo en docs/PARIDAD.md", file=sys.stderr)
        return 1

    documented = {int(number) for number in re.findall(r"issues/(\d+)", active_block)}
    active_issues = manifest.get("active_issues")
    if (
        not isinstance(active_issues, list)
        or not all(isinstance(number, int) and number > 0 for number in active_issues)
        or len(set(active_issues)) != len(active_issues)
    ):
        print("FAIL: active_issues debe ser una lista de números positivos únicos", file=sys.stderr)
        return 1
    expected = set(active_issues)
    if documented != expected:
        print(
            "FAIL: backlog activo no coincide con docs/parity/active-backlog.json "
            f"(docs={sorted(documented)}, manifest={sorted(expected)})",
            file=sys.stderr,
        )
        return 1

    closed_delivery_range = manifest.get("closed_delivery_range")
    if (
        not isinstance(closed_delivery_range, list)
        or len(closed_delivery_range) != 2
        or not all(isinstance(number, int) for number in closed_delivery_range)
    ):
        print("FAIL: closed_delivery_range debe tener dos números enteros", file=sys.stderr)
        return 1
    low, high = closed_delivery_range
    closed_as_active = sorted(number for number in documented if low <= number <= high)
    if closed_as_active:
        print(f"FAIL: issues cerrados usados como backlog activo: {closed_as_active}", file=sys.stderr)
        return 1

    print(f"OK: backlog activo validado ({', '.join(f'#{n}' for n in sorted(expected))})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
