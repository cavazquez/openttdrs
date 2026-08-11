#!/usr/bin/env python3
"""Compara streams JSONL `world-semantic` de OpenTTD y openttdrs (#306).

Consume ambos archivos en orden fila-mayor. A diferencia de `world-raw`, el
resultado no solo señala la tesela: conserva el camino semántico exacto que
diverge (`details.other_end.x`, `details.road_type`, etc.) y un inventario de
campos/fallbacks para priorizar el siguiente arreglo de renderer.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterator


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

# Campos que cambian cómo se orienta, conecta o dibuja una tesela. Mantener
# este inventario explícito evita que IDs únicos (por ejemplo, `other_end.x`)
# conviertan el informe en una lista enorme en vez de una señal de diagnóstico.
ORIENTATION_PATHS = (
    "bridge_above_axis",
    "details.track_bits",
    "details.road_bits",
    "details.tram_bits",
    "details.crossing_road_axis",
    "details.crossing_rail_axis",
    "details.depot_direction",
    "details.rail_axis",
    "details.road_stop_bay_direction",
    "details.road_stop_drive_through_axis",
    "details.dock_direction",
    "details.ship_depot_axis",
    "details.ship_depot_part",
    "details.ship_depot_direction",
    "details.lock_direction",
    "details.direction",
)

# Variantes que seleccionan una familia de sprites. A diferencia de las
# orientaciones, aquí se cuentan tipos/graphics y no posiciones absolutas.
VARIANT_PATHS = (
    "details.rail_tile_type",
    "details.rail_type",
    "details.road_tile_type",
    "details.road_type",
    "details.tram_type",
    "details.station_type",
    "details.station_gfx",
    "details.water_tile_type",
    "details.bridge_type",
    "details.transport_type",
    "details.tree_type",
    "details.house_type",
    "details.object_id",
    "details.object_type",
)

ENTITY_PATHS = {
    "station": "details.station_id",
    "industry": "details.industry_id",
    "object": "details.object_id",
    "town": "details.town_id",
}


class StreamError(RuntimeError):
    """Un archivo no cumple la forma mínima del stream JSONL."""


@dataclass(frozen=True)
class Record:
    line: int
    value: dict[str, Any]


def path_value(value: dict[str, Any], path: str) -> Any:
    """Lee un campo con puntos sin confundir `null` con un valor inventado."""
    current: Any = value
    for part in path.split("."):
        if not isinstance(current, dict) or part not in current:
            return None
        current = current[part]
    return current


def histogram_key(value: Any) -> str:
    """Clave JSON estable para histogramas de escalares semánticos."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float, str)):
        return str(value)
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


@dataclass
class SemanticInventory:
    """Resumen compacto de objetos, pendientes y decisiones de orientación."""

    classes: Counter[str] = field(default_factory=Counter)
    slopes: Counter[str] = field(default_factory=Counter)
    orientations: dict[str, Counter[str]] = field(
        default_factory=lambda: {path: Counter() for path in ORIENTATION_PATHS}
    )
    variants: dict[str, Counter[str]] = field(
        default_factory=lambda: {path: Counter() for path in VARIANT_PATHS}
    )
    topology: Counter[str] = field(default_factory=Counter)
    logical_entities: dict[str, set[str]] = field(
        default_factory=lambda: {name: set() for name in ENTITY_PATHS}
    )

    def add(self, record: Record) -> None:
        value = record.value
        semantic_class = value.get("class")
        if isinstance(semantic_class, str):
            self.classes[semantic_class] += 1
        else:
            semantic_class = "<missing>"
            self.classes[semantic_class] += 1

        tileh = value.get("tileh")
        if tileh is not None:
            self.slopes[histogram_key(tileh)] += 1

        for path, histogram in self.orientations.items():
            field_value = path_value(value, path)
            if field_value is not None:
                histogram[histogram_key(field_value)] += 1
        for path, histogram in self.variants.items():
            field_value = path_value(value, path)
            if field_value is not None:
                histogram[histogram_key(field_value)] += 1

        if semantic_class == "tunnel_bridge":
            other_end = path_value(value, "details.other_end")
            self.topology["tunnel_bridge.other_end.resolved" if other_end else "tunnel_bridge.other_end.unresolved"] += 1

        for entity, path in ENTITY_PATHS.items():
            field_value = path_value(value, path)
            if field_value is not None:
                self.logical_entities[entity].add(histogram_key(field_value))

    def as_json(self) -> dict[str, Any]:
        """Convierte `Counter`/sets a JSON ordenado para informes reproducibles."""
        return {
            "classes": dict(sorted(self.classes.items())),
            "slopes": dict(sorted(self.slopes.items(), key=lambda item: int(item[0]))),
            "orientations": {
                path: dict(sorted(histogram.items(), key=lambda item: item[0]))
                for path, histogram in self.orientations.items()
                if histogram
            },
            "variants": {
                path: dict(sorted(histogram.items(), key=lambda item: item[0]))
                for path, histogram in self.variants.items()
                if histogram
            },
            "topology": dict(sorted(self.topology.items())),
            "logical_entities": {
                entity: len(ids) for entity, ids in sorted(self.logical_entities.items()) if ids
            },
        }


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
        if record.value.get("kind") != "tile_semantic":
            raise StreamError(
                f"{path}:{record.line}: fila inesperada; se esperaba kind=tile_semantic"
            )
        yield record


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


def value_differences(reference: Any, candidate: Any, path: str = "") -> list[str]:
    """Devuelve caminos hoja distintos, incluyendo tipo y claves ausentes."""
    if type(reference) is not type(candidate):
        return [path or "<row>"]
    if isinstance(reference, dict):
        paths: list[str] = []
        for key in sorted(set(reference) | set(candidate)):
            child = f"{path}.{key}" if path else key
            if key not in reference or key not in candidate:
                paths.append(child)
            else:
                paths.extend(value_differences(reference[key], candidate[key], child))
        return paths
    if isinstance(reference, list):
        if len(reference) != len(candidate):
            return [path or "<row>"]
        paths = []
        for index, (left, right) in enumerate(zip(reference, candidate, strict=True)):
            paths.extend(value_differences(left, right, f"{path}[{index}]"))
        return paths
    return [] if reference == candidate else [path or "<row>"]


def coordinate(record: Record | None) -> dict[str, int] | None:
    if record is None:
        return None
    x = record.value.get("x")
    y = record.value.get("y")
    if isinstance(x, int) and isinstance(y, int):
        return {"x": x, "y": y}
    return None


def difference(
    classification: str,
    reference: Record | None,
    candidate: Record | None,
    field: str | None = None,
) -> dict[str, Any]:
    return {
        "classification": classification,
        "coordinate": coordinate(reference) or coordinate(candidate),
        "field": field,
        "reference_line": reference.line if reference else None,
        "candidate_line": candidate.line if candidate else None,
        "reference": reference.value if reference else None,
        "candidate": candidate.value if candidate else None,
    }


def matches_filter(
    reference: Record, candidate: Record, only_classes: set[str], where: set[tuple[int, int]]
) -> bool:
    if where:
        ref_coord = coordinate(reference)
        cand_coord = coordinate(candidate)
        if (ref_coord is None or (ref_coord["x"], ref_coord["y"]) not in where) and (
            cand_coord is None or (cand_coord["x"], cand_coord["y"]) not in where
        ):
            return False
    if only_classes:
        return (
            reference.value.get("class") in only_classes
            or candidate.value.get("class") in only_classes
        )
    return True


def matches_record_filter(record: Record, only_classes: set[str], where: set[tuple[int, int]]) -> bool:
    """Versión individual del filtro para inventariar ambos lados por separado."""
    if where:
        record_coordinate = coordinate(record)
        if (
            record_coordinate is None
            or (record_coordinate["x"], record_coordinate["y"]) not in where
        ):
            return False
    return not only_classes or record.value.get("class") in only_classes


def add_unsupported(counter: Counter[str], record: Record) -> None:
    value = record.value
    if value.get("supported") is True and value.get("unsupported_reason") in (None, ""):
        return
    semantic_class = value.get("class", "<missing>")
    reason = value.get("unsupported_reason", "<missing>")
    counter[f"{semantic_class}:{reason}"] += 1


def compare(
    reference_path: Path,
    candidate_path: Path,
    max_diffs: int,
    strict_metadata: bool,
    only_classes: set[str],
    where: set[tuple[int, int]],
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
    field_counts: Counter[str] = Counter()
    reference_unsupported: Counter[str] = Counter()
    candidate_unsupported: Counter[str] = Counter()
    reference_inventory = SemanticInventory()
    candidate_inventory = SemanticInventory()
    tile_difference_count = 0
    field_difference_count = 0
    compared_tile_count = 0
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
            add_unsupported(reference_unsupported, reference)
            if matches_record_filter(reference, only_classes, where):
                reference_inventory.add(reference)
        if candidate is not None:
            candidate_count += 1
            add_unsupported(candidate_unsupported, candidate)
            if matches_record_filter(candidate, only_classes, where):
                candidate_inventory.add(candidate)
        if reference is None:
            tile_difference_count += 1
            item = difference("missing_reference_tile", None, candidate)
            if len(differences) < max_diffs:
                differences.append(item)
            continue
        if candidate is None:
            tile_difference_count += 1
            item = difference("missing_candidate_tile", reference, None)
            if len(differences) < max_diffs:
                differences.append(item)
            continue

        identity_difference = next(
            (
                field
                for field in IDENTITY_FIELDS
                if reference.value.get(field) != candidate.value.get(field)
            ),
            None,
        )
        if identity_difference is not None:
            tile_difference_count += 1
            field_difference_count += 1
            field_counts[identity_difference] += 1
            if len(differences) < max_diffs:
                differences.append(
                    difference("tile_order_or_coordinate", reference, candidate, identity_difference)
                )
            continue
        if not matches_filter(reference, candidate, only_classes, where):
            continue

        compared_tile_count += 1
        paths = value_differences(reference.value, candidate.value)
        if paths:
            tile_difference_count += 1
            field_difference_count += len(paths)
            field_counts.update(paths)
            if len(differences) < max_diffs:
                differences.append(
                    difference("semantic_field_mismatch", reference, candidate, paths[0])
                )

    stream_count_mismatches: list[dict[str, Any]] = []
    for label, actual, metadata_value in (
        ("reference", reference_count, reference_meta.value),
        ("candidate", candidate_count, candidate_meta.value),
    ):
        expected = metadata_value.get("emitted_tile_count")
        if expected != actual:
            stream_count_mismatches.append(
                {"stream": label, "expected": expected, "actual": actual}
            )

    reference_inventory_json = reference_inventory.as_json()
    candidate_inventory_json = candidate_inventory.as_json()

    return {
        "schema_version": 1,
        "contract": "world-semantic-compare",
        "reference_path": str(reference_path),
        "candidate_path": str(candidate_path),
        "filters": {
            "only": sorted(only_classes),
            "where": [{"x": x, "y": y} for x, y in sorted(where)],
        },
        "reference_metadata": reference_meta.value,
        "candidate_metadata": candidate_meta.value,
        "metadata_mismatches": metadata_mismatches,
        "stream_count_mismatches": stream_count_mismatches,
        "reference_tile_count": reference_count,
        "candidate_tile_count": candidate_count,
        "compared_tile_count": compared_tile_count,
        "tile_difference_count": tile_difference_count,
        "field_difference_count": field_difference_count,
        "field_difference_counts": dict(sorted(field_counts.items())),
        "reference_unsupported": dict(sorted(reference_unsupported.items())),
        "candidate_unsupported": dict(sorted(candidate_unsupported.items())),
        "reference_inventory": reference_inventory_json,
        "candidate_inventory": candidate_inventory_json,
        "inventory_mismatches": value_differences(
            reference_inventory_json, candidate_inventory_json, "inventory"
        ),
        "differences": differences,
        "first_divergence": differences[0] if differences else None,
    }


def parse_coordinate(value: str) -> tuple[int, int]:
    parts = value.split(",")
    if len(parts) != 2:
        raise argparse.ArgumentTypeError("usar x,y")
    try:
        x, y = (int(part, 10) for part in parts)
    except ValueError as error:
        raise argparse.ArgumentTypeError("x e y deben ser enteros") from error
    if x < 0 or y < 0:
        raise argparse.ArgumentTypeError("x e y deben ser no negativos")
    return x, y


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="stream JSONL de OpenTTD / referencia")
    parser.add_argument("candidate", type=Path, help="stream JSONL de openttdrs")
    parser.add_argument(
        "--max-diffs",
        type=int,
        default=20,
        help="máximo de teselas divergentes detalladas (default: 20)",
    )
    parser.add_argument("--json-report", type=Path, help="escribe el informe JSON en esta ruta")
    parser.add_argument(
        "--strict-metadata",
        action="store_true",
        help="también falla por tick, commit, ruta fuente o versión distintos",
    )
    parser.add_argument(
        "--only",
        action="append",
        default=[],
        metavar="CLASE[,CLASE]",
        help="restringe el diff a familias, p. ej. railway,tunnel_bridge",
    )
    parser.add_argument(
        "--where",
        action="append",
        default=[],
        type=parse_coordinate,
        metavar="X,Y",
        help="restringe el diff a una tesela; puede repetirse",
    )
    parser.add_argument(
        "--show-inventory",
        action="store_true",
        help="muestra el inventario de familias, objetos y orientaciones del filtro",
    )
    args = parser.parse_args()
    if args.max_diffs <= 0:
        parser.error("--max-diffs debe ser positivo")
    args.only_classes = {
        semantic_class.strip() for group in args.only for semantic_class in group.split(",") if semantic_class.strip()
    }
    return args


def human_difference(item: dict[str, Any]) -> str:
    coordinate_value = item.get("coordinate")
    where = "sin coordenada"
    if coordinate_value:
        where = f"x={coordinate_value['x']}, y={coordinate_value['y']}"
    field = item.get("field")
    if field:
        return f"{item['classification']} en {where}, {field}"
    return f"{item['classification']} en {where}"


def compact_histogram(values: dict[str, int]) -> str:
    return ", ".join(f"{key}={count}" for key, count in values.items())


def print_inventory(inventory: dict[str, Any]) -> None:
    """Imprime un resumen humano, estable y suficientemente corto para consola."""
    print("  inventario (OpenTTD = openttdrs cuando no hay divergencias):")
    for section in ("classes", "logical_entities", "topology"):
        values = inventory.get(section, {})
        if values:
            print(f"    {section}: {compact_histogram(values)}")
    for section in ("orientations", "variants"):
        values = inventory.get(section, {})
        for field, histogram in values.items():
            print(f"    {field}: {compact_histogram(histogram)}")


def main() -> int:
    args = parse_args()
    try:
        report = compare(
            args.reference,
            args.candidate,
            args.max_diffs,
            args.strict_metadata,
            args.only_classes,
            set(args.where),
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
            "OK: world-semantic equivalente — "
            f"{report['compared_tile_count']} teselas comparadas"
        )
        if args.show_inventory:
            print_inventory(report["candidate_inventory"])
        return 0

    print("FAIL: world-semantic diverge")
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
    if report["field_difference_counts"]:
        top = sorted(
            report["field_difference_counts"].items(), key=lambda item: (-item[1], item[0])
        )[:5]
        print("  campos más frecuentes: " + ", ".join(f"{name}={count}" for name, count in top))
    if report["candidate_unsupported"]:
        print(f"  fallbacks openttdrs: {report['candidate_unsupported']}")
    if args.show_inventory:
        print_inventory(report["candidate_inventory"])
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
