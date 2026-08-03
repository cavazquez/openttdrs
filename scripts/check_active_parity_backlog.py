#!/usr/bin/env python3
"""Valida que el bloque de backlog activo no enlace issues cerrados (#292)."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "PARIDAD.md"
MANIFEST = ROOT / "docs" / "parity" / "active-backlog.json"
START = "<!-- active-parity-backlog:start -->"
END = "<!-- active-parity-backlog:end -->"


def main() -> int:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    text = DOC.read_text(encoding="utf-8")
    try:
        active_block = text.split(START, 1)[1].split(END, 1)[0]
    except IndexError:
        print("FAIL: falta bloque delimitado de backlog activo en docs/PARIDAD.md", file=sys.stderr)
        return 1

    documented = {int(number) for number in re.findall(r"issues/(\d+)", active_block)}
    expected = set(manifest["active_issues"])
    if documented != expected:
        print(
            "FAIL: backlog activo no coincide con docs/parity/active-backlog.json "
            f"(docs={sorted(documented)}, manifest={sorted(expected)})",
            file=sys.stderr,
        )
        return 1

    low, high = manifest["closed_delivery_range"]
    closed_as_active = sorted(number for number in documented if low <= number <= high)
    if closed_as_active:
        print(f"FAIL: issues cerrados usados como backlog activo: {closed_as_active}", file=sys.stderr)
        return 1

    print(f"OK: backlog activo validado ({', '.join(f'#{n}' for n in sorted(expected))})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
