#!/usr/bin/env python3
"""Contrato del corpus de regresión versionado para los fuzz targets."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "fuzz" / "regression-corpus"
ANCHORS = ROOT / "fuzz" / "regression-anchors"
MANIFEST = CORPUS / "manifest.json"
TARGETS = ("sav_load", "newgrf_parse", "net_message")
MAX_VERSIONED_BYTES = 256 * 1024
SEED_NAME = re.compile(r"[0-9a-f]{40}")


def fingerprint(directory: Path) -> tuple[int, int, str]:
    digest = hashlib.sha256()
    files = sorted(path for path in directory.rglob("*") if path.is_file())
    total = 0
    for path in files:
        relative = path.relative_to(directory).as_posix().encode()
        data = path.read_bytes()
        digest.update(relative)
        digest.update(b"\0")
        digest.update(hashlib.sha256(data).digest())
        total += len(data)
    return len(files), total, digest.hexdigest()


def main() -> int:
    errors: list[str] = []
    if not MANIFEST.is_file():
        errors.append("falta fuzz/regression-corpus/manifest.json")
        manifest: dict[str, object] = {}
    else:
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))

    if manifest.get("schema_version") != 1:
        errors.append("manifest: schema_version debe ser 1")
    if manifest.get("toolchain") != "nightly-2026-07-31":
        errors.append("manifest: el corpus debe declarar el nightly fijado")

    declared = manifest.get("targets")
    if not isinstance(declared, dict):
        errors.append("manifest: falta targets")
        declared = {}

    versioned_bytes = 0
    seed_count = 0
    for target in TARGETS:
        directory = CORPUS / target
        if not directory.is_dir():
            errors.append(f"corpus: falta {directory.relative_to(ROOT)}")
            continue
        paths = list(directory.rglob("*"))
        if any(path.is_symlink() for path in paths):
            errors.append(f"corpus: {target} contiene symlink")
        files = [path for path in paths if path.is_file()]
        if not files:
            errors.append(f"corpus: {target} está vacío")
            continue
        if any(not SEED_NAME.fullmatch(path.name) for path in files):
            errors.append(f"corpus: {target} contiene un nombre que no es hash SHA-1")

        count, size, digest = fingerprint(directory)
        seed_count += count
        versioned_bytes += size
        record = declared.get(target)
        if not isinstance(record, dict):
            errors.append(f"manifest: falta target {target}")
            continue
        if record.get("files") != count or record.get("bytes") != size:
            errors.append(f"manifest: conteo/tamaño no coincide para {target}")
        if record.get("sha256") != digest:
            errors.append(f"manifest: hash agregado no coincide para {target}")

    anchor = ANCHORS / "sav_load" / "mvp_openttd_rich.sav"
    if not anchor.is_file():
        errors.append("anchors: falta mvp_openttd_rich.sav")
    elif hashlib.sha256(anchor.read_bytes()).hexdigest() != (
        "0cc840af3f0d4c18e191a5c90ec83289d3026f537a3cfb66d414498d9e58bc9e"
    ):
        errors.append("anchors: hash inesperado de mvp_openttd_rich.sav")
    else:
        versioned_bytes += anchor.stat().st_size

    if versioned_bytes > MAX_VERSIONED_BYTES:
        errors.append(
            f"corpus+anchors exceden {MAX_VERSIONED_BYTES} bytes ({versioned_bytes})"
        )

    ignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    if "/fuzz/corpus/" not in ignore:
        errors.append(".gitignore debe conservar fuzz/corpus como área local mutable")

    if errors:
        print("FAIL: corpus de regresión de fuzz", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print(
        "OK: corpus fuzz versionado "
        f"({versioned_bytes} bytes, {seed_count} seeds + 1 anchor)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
