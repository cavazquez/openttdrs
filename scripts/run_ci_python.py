#!/usr/bin/env python3
"""Ejecuta los checks Python definidos en ``ci_python_manifest.json`` (#120).

Uso:
  python3 scripts/run_ci_python.py golden
  python3 scripts/run_ci_python.py py_compile
  python3 scripts/run_ci_python.py runs
  python3 scripts/run_ci_python.py all
"""

from __future__ import annotations

import json
import py_compile
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = Path(__file__).resolve().with_name("ci_python_manifest.json")


def load_manifest() -> dict:
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    for key in ("golden", "py_compile", "runs"):
        if key not in data or not isinstance(data[key], list):
            raise SystemExit(f"manifiesto inválido: falta lista '{key}' en {MANIFEST}")
    return data


def run_scripts(rel_paths: list[str]) -> None:
    for rel in rel_paths:
        path = ROOT / rel
        if not path.is_file():
            raise SystemExit(f"no existe: {rel}")
        print(f"  run {rel}", flush=True)
        subprocess.run([sys.executable, str(path)], cwd=ROOT, check=True)


def compile_scripts(rel_paths: list[str]) -> None:
    for rel in rel_paths:
        path = ROOT / rel
        if not path.is_file():
            raise SystemExit(f"no existe: {rel}")
        print(f"  py_compile {rel}", flush=True)
        py_compile.compile(str(path), doraise=True)


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in {"golden", "py_compile", "runs", "all"}:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    mode = argv[1]
    man = load_manifest()
    print(f"[ci-python] {mode} (desde {MANIFEST.relative_to(ROOT)})", flush=True)
    if mode in {"golden", "all"}:
        run_scripts(man["golden"])
    if mode in {"py_compile", "all"}:
        compile_scripts(man["py_compile"])
    if mode in {"runs", "all"}:
        run_scripts(man["runs"])
    print(f"[ci-python] {mode} OK", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
