#!/usr/bin/env python3
"""Verifica reproducibilidad de tablas Rust generadas (#119).

- No escribe en los ``*_generated.rs`` versionados.
- Piloto ``house_population``: regenera en memoria contra OpenTTD pin (#109).
- Piloto ``house_draw_data``: regenera si hay OpenGFX local; si no, valida
  ``output_sha256`` del manifiesto (CI sin assets).

Uso:
  python3 scripts/check_generated_tables.py --check
  python3 scripts/check_generated_tables.py --check --fetch-upstream
  python3 scripts/check_generated_tables.py --list
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "scripts" / "generated_tables_manifest.json"


def manifest_path() -> Path:
    import os

    override = os.environ.get("OPENTTDRS_GENERATED_TABLES_MANIFEST")
    return Path(override) if override else DEFAULT_MANIFEST


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def load_manifest() -> dict:
    path = manifest_path()
    return json.loads(path.read_text(encoding="utf-8"))


def fetch_upstream() -> None:
    script = ROOT / "scripts" / "fetch-openttd-reference.sh"
    print("[generated-tables] fetch OpenTTD pin (#109)...", flush=True)
    subprocess.run(["bash", str(script)], cwd=ROOT, check=True)


def opengfx_house_assets_present() -> bool:
    tiles = ROOT / "assets" / "opengfx" / "tiles"
    return tiles.is_dir() and any(tiles.glob("house_s*.png"))


def check_hash(entry: dict) -> int:
    out = ROOT / entry["output"]
    expected = entry.get("output_sha256")
    if not expected:
        print(f"FAIL {entry['id']}: falta output_sha256 en manifiesto", file=sys.stderr)
        return 1
    if not out.is_file():
        print(f"FAIL {entry['id']}: no existe {entry['output']}", file=sys.stderr)
        return 1
    actual = sha256_file(out)
    if actual != expected:
        print(f"DRIFT {entry['id']}: sha256 del output no coincide con el manifiesto.", file=sys.stderr)
        print(f"  archivo: {entry['output']}", file=sys.stderr)
        print(f"  esperado: {expected}", file=sys.stderr)
        print(f"  actual:   {actual}", file=sys.stderr)
        print(
            f"  Si regeneraste a propósito: actualizá output_sha256 en {manifest_path().name}",
            file=sys.stderr,
        )
        return 1
    print(f"OK {entry['id']}: output_sha256 coincide ({actual[:12]}…)")
    return 0


def check_regenerate(entry: dict) -> int:
    gen = ROOT / entry["generator"]
    cmd = [sys.executable, str(gen), "--check"]
    print(f"[generated-tables] {' '.join(cmd)}", flush=True)
    proc = subprocess.run(cmd, cwd=ROOT)
    if proc.returncode == 2:
        # skip semántico del generador (p.ej. sin assets)
        return 2
    return proc.returncode


def run_checks(*, fetch: bool) -> int:
    man = load_manifest()
    if fetch:
        fetch_upstream()

    rc = 0
    for entry in man["pilots"]:
        mode = entry.get("check", "regenerate")
        out = ROOT / entry["output"]
        if not out.is_file():
            print(f"FAIL {entry['id']}: falta {entry['output']}", file=sys.stderr)
            rc = 1
            continue

        if mode in {"regenerate", "regenerate_or_hash", "regenerate_if_assets"}:
            inputs_ok = all((ROOT / p).is_file() for p in entry.get("inputs", []))
            assets_ok = (not entry.get("requires_opengfx")) or opengfx_house_assets_present()
            can_regen = inputs_ok and assets_ok

            if mode == "regenerate":
                if not can_regen:
                    print(
                        f"FAIL {entry['id']}: faltan inputs. "
                        "Ejecutá ./scripts/fetch-openttd-reference.sh "
                        "o reintentá con --fetch-upstream",
                        file=sys.stderr,
                    )
                    rc = 1
                    continue
                if check_regenerate(entry) != 0:
                    rc = 1
                continue

            # regenerate_or_hash / regenerate_if_assets: hash siempre.
            if check_hash(entry) != 0:
                rc = 1
                continue
            if not can_regen:
                why = "sin OpenGFX local" if not assets_ok else "sin OpenTTD local"
                print(f"OK {entry['id']}: hash OK; regenerate omitido ({why})")
                continue
            sub = check_regenerate(entry)
            if sub == 2:
                print(f"OK {entry['id']}: hash OK; regenerate omitido por generador")
            elif sub != 0:
                rc = 1
        elif mode == "hash":
            if check_hash(entry) != 0:
                rc = 1
        else:
            print(f"FAIL {entry['id']}: check mode desconocido {mode!r}", file=sys.stderr)
            rc = 1

    if rc == 0:
        print("[generated-tables] OK (working tree no modificado)")
    return rc


def list_inventory() -> int:
    man = load_manifest()
    path = manifest_path()
    try:
        rel = path.relative_to(ROOT)
    except ValueError:
        rel = path
    print(f"Manifiesto: {rel}")
    print(f"Pilots ({len(man['pilots'])}):")
    for p in man["pilots"]:
        print(f"  - {p['id']}: {p['check']} ← {p['generator']} → {p['output']}")
    print(f"Inventario ({len(man['inventory'])}):")
    for item in man["inventory"]:
        mark = " [pilot]" if item.get("pilot") else ""
        print(f"  - {item['output']} ← {item['generator']}{mark}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="ejecutar checks piloto")
    parser.add_argument(
        "--fetch-upstream",
        action="store_true",
        help="clonar/actualizar OpenTTD al commit del pin #109 antes del check",
    )
    parser.add_argument("--list", action="store_true", help="listar inventario")
    args = parser.parse_args(argv)

    if args.list and not args.check:
        return list_inventory()
    if args.check:
        if args.list:
            list_inventory()
        return run_checks(fetch=args.fetch_upstream)
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
