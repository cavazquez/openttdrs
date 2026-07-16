#!/usr/bin/env python3
"""Valida el manifiesto de referencia OpenTTD (#109)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs" / "parity" / "openttd-reference.json"
SHA1_RE = re.compile(r"^[0-9a-f]{40}$")

REQUIRED = (
    "schema_version",
    "name",
    "url",
    "commit",
    "tag",
    "pinned_at",
    "license_spdx",
    "clone_path",
    "update_policy",
)


def main() -> int:
    if not MANIFEST.is_file():
        print(f"FAIL: falta {MANIFEST}", file=sys.stderr)
        return 1
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    missing = [k for k in REQUIRED if k not in data]
    if missing:
        print(f"FAIL: campos ausentes: {missing}", file=sys.stderr)
        return 1
    if data["schema_version"] != 1:
        print(f"FAIL: schema_version inesperado: {data['schema_version']}", file=sys.stderr)
        return 1
    commit = data["commit"]
    if not SHA1_RE.match(commit):
        print(f"FAIL: commit no es SHA-1 de 40 hex: {commit}", file=sys.stderr)
        return 1
    if not str(data["url"]).startswith("https://"):
        print(f"FAIL: url debe ser https: {data['url']}", file=sys.stderr)
        return 1
    if data["clone_path"] != "reference/openttd-upstream":
        print(f"FAIL: clone_path inesperado: {data['clone_path']}", file=sys.stderr)
        return 1
    print(
        f"OK: OpenTTD reference tag={data['tag']} commit={commit} "
        f"license={data['license_spdx']} pinned_at={data['pinned_at']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
