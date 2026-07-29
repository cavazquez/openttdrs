#!/usr/bin/env python3
"""Valida la estructura de chunks de un .sav (#66).

Por defecto comprueba los chunks de mapa (compatibles con saves OpenTTD).
Con --export exige además DATE y PLYR (formato del export openttdrs).

Uso:
  python3 scripts/validate_sav_export.py [ruta.sav]
  python3 scripts/validate_sav_export.py --export [ruta.sav]
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import parse_sav  # noqa: E402

REQUIRED_MAP = [
    "MAPS",
    "MAPT",
    "MAPH",
    "MAPO",
    "MAP2",
    "M3LO",
    "M3HI",
    "MAP5",
    "MAPE",
    "MAP7",
    "MAP8",
]
REQUIRED_EXPORT_EXTRA = ["CITY", "DATE", "PLYR"]
OPTIONAL_HINT = ("STNN", "INDY", "VEHS", "ORDL")


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--export"]
    export_mode = "--export" in sys.argv[1:]
    default = ROOT / "crates/openttdrs-core/tests/fixtures/demo_openttd.sav"
    path = Path(args[0]) if args else default
    if not path.is_file():
        print(f"FAIL: no existe {path}", file=sys.stderr)
        return 1
    raw = path.read_bytes()
    payload, version = parse_sav.decompress(raw)
    chunks = parse_sav.parse_chunks(payload)
    required = list(REQUIRED_MAP)
    if export_mode:
        required.extend(REQUIRED_EXPORT_EXTRA)
    missing = [c for c in required if c not in chunks]
    if missing:
        print(f"FAIL: chunks faltantes en {path}: {missing}", file=sys.stderr)
        print(f"  presentes: {sorted(chunks)}", file=sys.stderr)
        return 1
    present_opt = [c for c in OPTIONAL_HINT if c in chunks]
    mode = "export" if export_mode else "mapa"
    print(
        f"OK ({mode}): {path.name} v{version} — "
        f"{len(required)} obligatorios + opcionales {present_opt}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
