#!/usr/bin/env python3
"""Detecta drift entre el manifiesto Python CI y el workflow (#120).

Comprueba que:
- los paths del manifiesto existen;
- ``ci.yml`` invoca ``./scripts/check.sh ci-python`` y ``tnbp`` (sin listas duplicadas);
- ``check.sh`` delega al runner del manifiesto.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "scripts" / "ci_python_manifest.json"
CHECK_SH = ROOT / "scripts" / "check.sh"
CI_YML = ROOT / ".github" / "workflows" / "ci.yml"

# Invocaciones directas que no deben reaparecer en ci.yml (listas duplicadas).
FORBIDDEN_INLINE = (
    "verify_parse_sav_reference.py",
    "verify_parse_sav_water_m5.py",
    "verify_parse_sav_rail_m5.py",
    "validate_sav_export.py",
    "python3 -m py_compile",
    "test_openttd_reference_manifest.py",
    "test_compare_snapshots_mutation.py",
)


def main() -> int:
    errors: list[str] = []
    man = json.loads(MANIFEST.read_text(encoding="utf-8"))
    for key in ("golden", "py_compile", "runs"):
        for rel in man[key]:
            if not (ROOT / rel).is_file():
                errors.append(f"manifiesto {key}: falta {rel}")

    check = CHECK_SH.read_text(encoding="utf-8")
    if "run_ci_python.py" not in check:
        errors.append("check.sh no invoca scripts/run_ci_python.py")
    if not re.search(r"\bci-python\)", check):
        errors.append("check.sh no expone el modo ci-python")

    yml = CI_YML.read_text(encoding="utf-8")
    if "scripts/check.sh ci-python" not in yml:
        errors.append("ci.yml no invoca ./scripts/check.sh ci-python")
    if "scripts/check.sh tnbp" not in yml:
        errors.append("ci.yml no invoca ./scripts/check.sh tnbp")

    for needle in FORBIDDEN_INLINE:
        for i, line in enumerate(yml.splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            if needle in line:
                errors.append(f"ci.yml:{i} lista Python duplicada ({needle!r})")

    if "cargo doc" not in yml:
        errors.append("ci.yml perdió el step rustdoc (excepción documentada GHA-only)")
    if "cargo audit" not in yml:
        errors.append("ci.yml perdió cargo audit (excepción documentada GHA-only)")
    if "cargo deny check" not in yml:
        errors.append("ci.yml perdió cargo deny (excepción documentada GHA-only)")

    if errors:
        print("FAIL: drift CI local/remoto (#120)", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1
    print("OK: manifiesto Python CI alineado con check.sh y ci.yml")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
