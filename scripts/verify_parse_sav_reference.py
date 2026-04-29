#!/usr/bin/env python3
"""
Comprueba que un .sav produce las mismas estadísticas que un golden JSON.

Uso (desde la raíz del repo):

  python3 scripts/verify_parse_sav_reference.py \\
      tests/fixtures/stationlist-test.sav \\
      tests/fixtures/parse_sav_stationlist_golden.json

Salida: código 0 si coincide; distinta de 0 si hay diferencias (mensaje en stderr).
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


def _deep_diff(expected: dict, actual: dict, path: str = "") -> list[str]:
    errors: list[str] = []
    for key in expected:
        p = f"{path}.{key}" if path else key
        if key not in actual:
            errors.append(f"Falta clave en resultado: {p}")
            continue
        ev, av = expected[key], actual[key]
        if isinstance(ev, dict) and isinstance(av, dict):
            errors.extend(_deep_diff(ev, av, p))
        elif ev != av:
            errors.append(f"{p}: esperado {ev!r}, obtenido {av!r}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "sav",
        type=Path,
        nargs="?",
        default=Path("tests/fixtures/stationlist-test.sav"),
        help="Ruta al savegame .sav",
    )
    parser.add_argument(
        "golden",
        type=Path,
        nargs="?",
        default=Path("tests/fixtures/parse_sav_stationlist_golden.json"),
        help="JSON golden generado con emit_parse_sav_golden.py",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Raíz del repo (por defecto: directorio de trabajo actual)",
    )
    args = parser.parse_args()
    repo_root = args.repo_root or Path.cwd()

    if not args.sav.is_file():
        print(f"ERROR: no existe el save: {args.sav}", file=sys.stderr)
        return 1
    if not args.golden.is_file():
        print(f"ERROR: no existe el golden: {args.golden}", file=sys.stderr)
        return 1

    parse_sav = _load_parse_sav(repo_root)
    raw = args.sav.read_bytes()
    actual_full = parse_sav.analyze_save(raw)

    # Corpus: tipos de chunk en disco (nibble bajo del byte `m`) deben ser 0–4
    # (CH_RIFF … CH_SPARSE_TABLE). CH_READONLY (5) y >5 no aparecen en saves normales.
    data, _ver = parse_sav.decompress(raw)
    trace: list[tuple[str, int]] = []
    parse_sav.parse_chunks(data, chunk_type_trace=trace)
    bad = [(n, t) for n, t in trace if t > 4]
    if bad:
        print(
            "verify_parse_sav_reference: chunk_type fuera de rango 0–4:",
            bad[:20],
            file=sys.stderr,
        )
        return 1
    readonly_hits = [n for n, t in trace if t == 5]
    if readonly_hits:
        print(
            "verify_parse_sav_reference: CH_READONLY (5) en fixture (inesperado):",
            readonly_hits[:20],
            file=sys.stderr,
        )
        return 1

    golden_obj = json.loads(args.golden.read_text(encoding="utf-8"))
    expected = {
        "save_version": golden_obj["save_version"],
        "dimensions": golden_obj["dimensions"],
        "tile_type_counts": golden_obj["tile_type_counts"],
        "house": golden_obj["house"],
    }
    actual = {
        "save_version": actual_full["save_version"],
        "dimensions": actual_full["dimensions"],
        "tile_type_counts": actual_full["tile_type_counts"],
        "house": actual_full["house"],
    }

    errs = _deep_diff(expected, actual)
    if errs:
        print("verify_parse_sav_reference: diferencias respecto al golden:", file=sys.stderr)
        for e in errs:
            print(f"  {e}", file=sys.stderr)
        return 1

    print(f"OK: {args.sav.name} coincide con {args.golden.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
