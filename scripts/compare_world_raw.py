#!/usr/bin/env python3
"""Compara streams JSONL `world-raw` de OpenTTD y openttdrs (#305).

El comparador no carga el mapa completo: consume ambas secuencias en orden
fila-mayor y señala la primera coordenada/byte que diverge. También valida que
la cabecera y la cantidad de filas concuerden con el contrato.
"""
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterator


RAW_FIELDS = ("height", "type", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8")
IDENTITY_FIELDS = ("index", "x", "y")
HARD_METADATA_FIELDS = (
    "schema_version",
    "contract",
    "width",
    "height",
    "tile_count",
    "emitted_tile_count",
    "region",
)
SOFT_METADATA_FIELDS = ("tick", "climate", "openttd_commit", "source_path", "save_version")


class StreamError(RuntimeError):
    """Un archivo no cumple la forma mínima del stream JSONL."""


@dataclass(frozen=True)
class Record:
    line: int
    value: dict[str, Any]


def records(path: Path) -> Iterator[Record]:
    try:
        with path.open(encoding="utf-8") as source:
            for line_number, line in enumerate(source, start=1):
                line = line.strip()
                if not line:
                    continue
                try:
                    value = json.loads(line)
                except json.JSONDecodeError as error:
                    raise StreamError(f"{path}:{line_number}: JSON inválido: {error.msg}") from error
                if not isinstance(value, dict):
                    raise StreamError(f"{path}:{line_number}: cada línea debe ser un objeto JSON")
                yield Record(line_number, value)
    except OSError as error:
        raise StreamError(f"no se pudo leer {path}: {error}") from error


def metadata(stream: Iterator[Record], path: Path) -> Record:
    try:
        record = next(stream)
    except StopIteration as error:
        raise StreamError(f"{path}: stream vacío; falta metadata") from error
    if record.value.get("kind") != "metadata":
        raise StreamError(f"{path}:{record.line}: la primera fila debe tener kind=metadata")
    return record


def tile_records(stream: Iterator[Record], path: Path) -> Iterator[Record]:
    for record in stream:
        if record.value.get("kind") != "tile_raw":
            raise StreamError(
                f"{path}:{record.line}: fila inesperada; se esperaba kind=tile_raw"
            )
        yield record


def difference(
    classification: str,
    reference: Record | None,
    candidate: Record | None,
    field: str | None = None,
) -> dict[str, Any]:
    coordinate: dict[str, int] | None = None
    for record in (reference, candidate):
        if record is None:
            continue
        value = record.value
        if isinstance(value.get("x"), int) and isinstance(value.get("y"), int):
            coordinate = {"x": value["x"], "y": value["y"]}
            break
    result: dict[str, Any] = {
        "classification": classification,
        "coordinate": coordinate,
        "field": field,
        "reference_line": reference.line if reference else None,
        "candidate_line": candidate.line if candidate else None,
        "reference": reference.value if reference else None,
        "candidate": candidate.value if candidate else None,
    }
    return result


def tile_difference(reference: Record, candidate: Record) -> dict[str, Any] | None:
    for field in IDENTITY_FIELDS:
        if reference.value.get(field) != candidate.value.get(field):
            return difference("tile_order_or_coordinate", reference, candidate, field)
    for field in RAW_FIELDS:
        if field not in reference.value or field not in candidate.value:
            return difference("missing_raw_field", reference, candidate, field)
        if reference.value[field] != candidate.value[field]:
            return difference("raw_field_mismatch", reference, candidate, field)
    return None


def metadata_differences(
    reference: dict[str, Any], candidate: dict[str, Any], strict: bool
) -> list[dict[str, Any]]:
    differences: list[dict[str, Any]] = []
    if reference.get("kind") != "metadata" or candidate.get("kind") != "metadata":
        differences.append(
            {
                "field": "kind",
                "reference": reference.get("kind"),
                "candidate": candidate.get("kind"),
            }
        )
    for field in HARD_METADATA_FIELDS:
        if reference.get(field) != candidate.get(field):
            differences.append(
                {
                    "field": field,
                    "reference": reference.get(field),
                    "candidate": candidate.get(field),
                }
            )
    reference_hash = reference.get("save_sha256")
    candidate_hash = candidate.get("save_sha256")
    if reference_hash and candidate_hash and reference_hash != candidate_hash:
        differences.append(
            {
                "field": "save_sha256",
                "reference": reference_hash,
                "candidate": candidate_hash,
            }
        )
    if strict:
        for field in SOFT_METADATA_FIELDS:
            if reference.get(field) != candidate.get(field):
                differences.append(
                    {
                        "field": field,
                        "reference": reference.get(field),
                        "candidate": candidate.get(field),
                    }
                )
    return differences


def decimal_and_hex(value: Any, field: str | None) -> str:
    if not isinstance(value, int):
        return repr(value)
    if field in {"m2", "m8"}:
        return f"{value} (0x{value:04x})"
    if field in {"height", "type", "m1", "m3", "m4", "m5", "m6", "m7"}:
        return f"{value} (0x{value:02x})"
    return str(value)


def human_difference(item: dict[str, Any]) -> str:
    coordinate = item.get("coordinate")
    where = "sin coordenada"
    if coordinate:
        where = f"x={coordinate['x']}, y={coordinate['y']}"
    field = item.get("field")
    reference = item.get("reference")
    candidate = item.get("candidate")
    if field and isinstance(reference, dict) and isinstance(candidate, dict):
        return (
            f"{item['classification']} en {where}, {field}: "
            f"OpenTTD={decimal_and_hex(reference.get(field), field)}, "
            f"openttdrs={decimal_and_hex(candidate.get(field), field)}"
        )
    return f"{item['classification']} en {where}"


def compare(
    reference_path: Path, candidate_path: Path, max_diffs: int, strict_metadata: bool
) -> dict[str, Any]:
    reference_stream = records(reference_path)
    candidate_stream = records(candidate_path)
    reference_meta = metadata(reference_stream, reference_path)
    candidate_meta = metadata(candidate_stream, candidate_path)
    metadata_mismatches = metadata_differences(
        reference_meta.value, candidate_meta.value, strict_metadata
    )

    reference_tiles = tile_records(reference_stream, reference_path)
    candidate_tiles = tile_records(candidate_stream, candidate_path)
    differences: list[dict[str, Any]] = []
    total_differences = 0
    reference_count = 0
    candidate_count = 0

    while True:
        try:
            reference = next(reference_tiles)
        except StopIteration:
            reference = None
        try:
            candidate = next(candidate_tiles)
        except StopIteration:
            candidate = None
        if reference is None and candidate is None:
            break
        if reference is not None:
            reference_count += 1
        if candidate is not None:
            candidate_count += 1
        if reference is None:
            item = difference("missing_reference_tile", None, candidate)
        elif candidate is None:
            item = difference("missing_candidate_tile", reference, None)
        else:
            item = tile_difference(reference, candidate)
        if item is not None:
            total_differences += 1
            if len(differences) < max_diffs:
                differences.append(item)

    stream_count_mismatches: list[dict[str, Any]] = []
    for label, actual, meta in (
        ("reference", reference_count, reference_meta.value),
        ("candidate", candidate_count, candidate_meta.value),
    ):
        expected = meta.get("emitted_tile_count")
        if expected != actual:
            stream_count_mismatches.append(
                {"stream": label, "expected": expected, "actual": actual}
            )

    return {
        "schema_version": 1,
        "contract": "world-raw-compare",
        "reference_path": str(reference_path),
        "candidate_path": str(candidate_path),
        "reference_metadata": reference_meta.value,
        "candidate_metadata": candidate_meta.value,
        "metadata_mismatches": metadata_mismatches,
        "stream_count_mismatches": stream_count_mismatches,
        "reference_tile_count": reference_count,
        "candidate_tile_count": candidate_count,
        "tile_difference_count": total_differences,
        "differences": differences,
        "first_divergence": differences[0] if differences else None,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="stream JSONL de OpenTTD / referencia")
    parser.add_argument("candidate", type=Path, help="stream JSONL de openttdrs")
    parser.add_argument(
        "--max-diffs",
        type=int,
        default=20,
        help="máximo de diferencias detalladas a retener (default: 20)",
    )
    parser.add_argument("--json-report", type=Path, help="escribe el informe JSON en esta ruta")
    parser.add_argument(
        "--strict-metadata",
        action="store_true",
        help="también falla por commit, ruta fuente o versión de save distintos",
    )
    args = parser.parse_args()
    if args.max_diffs <= 0:
        parser.error("--max-diffs debe ser positivo")
    return args


def main() -> int:
    args = parse_args()
    try:
        report = compare(
            args.reference,
            args.candidate,
            args.max_diffs,
            args.strict_metadata,
        )
    except StreamError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    if args.json_report:
        args.json_report.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

    failed = bool(
        report["metadata_mismatches"]
        or report["stream_count_mismatches"]
        or report["tile_difference_count"]
    )
    if not failed:
        print(
            "OK: world-raw equivalente — "
            f"{report['reference_tile_count']} teselas comparadas en orden fila-mayor"
        )
        return 0

    print("FAIL: world-raw diverge")
    if report["metadata_mismatches"]:
        first = report["metadata_mismatches"][0]
        print(
            f"  metadata.{first['field']}: "
            f"OpenTTD={first['reference']!r}, openttdrs={first['candidate']!r}"
        )
    if report["stream_count_mismatches"]:
        first = report["stream_count_mismatches"][0]
        print(
            f"  {first['stream']}: metadata esperaba {first['expected']} filas, "
            f"se leyeron {first['actual']}"
        )
    if report["first_divergence"]:
        print(f"  primera tesela: {human_difference(report['first_divergence'])}")
        remaining = report["tile_difference_count"] - 1
        if remaining > 0:
            print(f"  (+{remaining} diferencia(s) de tesela; se detallan hasta {args.max_diffs})")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
