#!/usr/bin/env python3
"""
Emite JSON listo para usar como golden en verify_parse_sav_reference.py.

  python3 scripts/emit_parse_sav_golden.py tests/fixtures/stationlist-test.sav \\
    > tests/fixtures/parse_sav_stationlist_golden.json

Campos: save_version, dimensions, tile_type_counts, house (tiles, unique_m8, m8_histogram).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


def _load_parse_sav(repo_root: Path):
    script = repo_root / "scripts" / "parse_sav.py"
    spec = importlib.util.spec_from_file_location("parse_sav", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"No se pudo cargar {script}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("sav", type=Path, help="Archivo .sav")
    p.add_argument(
        "--fixture-name",
        default=None,
        help="Valor del campo \"fixture\" en el JSON (por defecto: nombre del archivo)",
    )
    args = p.parse_args()
    repo_root = Path.cwd()
    parse_sav = _load_parse_sav(repo_root)

    if not args.sav.is_file():
        print(f"No existe: {args.sav}", file=sys.stderr)
        return 1

    data = parse_sav.analyze_save(args.sav.read_bytes())
    fixture = args.fixture_name or args.sav.name
    out = {
        "fixture": fixture,
        **data,
    }
    print(json.dumps(out, indent=2, sort_keys=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
