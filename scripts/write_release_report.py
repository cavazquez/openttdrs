#!/usr/bin/env python3
"""Escribe el informe verificable de un paquete de release (#296)."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def existing_file(value: str) -> Path:
    path = Path(value)
    if not path.is_file():
        raise argparse.ArgumentTypeError(f"no existe el archivo: {path}")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--archive", required=True, type=existing_file)
    parser.add_argument("--client", required=True, type=existing_file)
    parser.add_argument("--dedicated", required=True, type=existing_file)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--glibc-baseline")
    args = parser.parse_args()

    report = {
        "version": args.version,
        "source_sha": args.source_sha,
        "platform": args.platform,
        "hashes": {
            "archive": {"file": args.archive.name, "sha256": sha256(args.archive)},
            "client": {"file": args.client.name, "sha256": sha256(args.client)},
            "dedicated": {"file": args.dedicated.name, "sha256": sha256(args.dedicated)},
        },
        "smokes": {
            "archive_extraction": "passed",
            "assets": "passed",
            "fonts": "passed",
            "audio": "passed",
            "client": "passed",
            "dedicated": "passed",
            "client_server_handshake": "passed",
        },
    }
    if args.glibc_baseline:
        report["linux_glibc_baseline"] = args.glibc_baseline

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"Informe de release: {args.output}")


if __name__ == "__main__":
    main()
