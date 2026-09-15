#!/usr/bin/env python3
"""Resume las bandas del sorter de una captura nativa (#326).

El trace ``world-screenshot-sort`` tiene una lista de parents por cada llamada
de ``ViewportDoDraw``. Una misma secuencia combinada puede cambiar de primer
sprite cuando el primer recorte no alcanza esa banda; comparar sólo una lista
global confunde esa promoción con un asset ausente. Este informe conserva las
dos identidades: la exacta (sprite + bounds) y la caja de mundo compartida.
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any


class TraceError(RuntimeError):
    """El trace no cumple el contrato mínimo de captura segmentada."""


Bounds = tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class Parent:
    segment: int
    final_ordinal: int
    sprite: int
    bounds: Bounds


@dataclass(frozen=True)
class Segment:
    ordinal: int
    virtual: dict[str, int]
    parents: tuple[Parent, ...]
    children: int


@dataclass(frozen=True)
class ScreenshotSortTrace:
    path: Path
    metadata: dict[str, Any]
    segments: tuple[Segment, ...]
    complete: dict[str, Any]

    @property
    def parents(self) -> tuple[Parent, ...]:
        return tuple(parent for segment in self.segments for parent in segment.parents)


def require_int(value: object, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TraceError(f"{label} debe ser entero")
    return value


def require_object(value: object, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise TraceError(f"{label} debe ser un objeto")
    return value


def bounds_from_parent(row: dict[str, Any], label: str) -> Bounds:
    raw = require_object(row.get("world_bounds"), f"{label}.world_bounds")
    fields = ("xmin", "ymin", "zmin", "xmax", "ymax", "zmax")
    return tuple(require_int(raw.get(field), f"{label}.world_bounds.{field}") for field in fields)  # type: ignore[return-value]


def read_rows(path: Path) -> list[dict[str, Any]]:
    try:
        raw_lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise TraceError(f"no se pudo leer {path}: {error}") from error
    rows: list[dict[str, Any]] = []
    for line_number, raw in enumerate(raw_lines, start=1):
        if not raw.strip():
            continue
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as error:
            raise TraceError(f"{path}:{line_number}: JSON inválido: {error.msg}") from error
        rows.append(require_object(value, f"{path}:{line_number}"))
    if not rows:
        raise TraceError(f"{path}: trace vacío")
    return rows


def load_trace(path: Path) -> ScreenshotSortTrace:
    rows = read_rows(path)
    metadata = rows[0]
    if metadata.get("kind") != "metadata":
        raise TraceError(f"{path}: la primera fila debe ser kind=metadata")
    if metadata.get("contract") != "world-screenshot-sort":
        raise TraceError(f"{path}: se esperaba contract=world-screenshot-sort")
    if metadata.get("producer") != "openttd":
        raise TraceError(f"{path}: se esperaba producer=openttd")
    if metadata.get("stage") != "post_viewport_sprite_sorter":
        raise TraceError(f"{path}: stage no es post_viewport_sprite_sorter")

    complete = rows[-1]
    if complete.get("kind") != "complete":
        raise TraceError(f"{path}: falta la fila final kind=complete")

    segments: list[Segment] = []
    current: dict[str, Any] | None = None
    for row in rows[1:-1]:
        kind = row.get("kind")
        if kind == "segment":
            if current is not None:
                expected = require_int(current["row"].get("parents"), "segment.parents")
                if expected != len(current["parents"]):
                    raise TraceError(
                        f"{path}: segment {current['ordinal']} declara {expected} parents, "
                        f"se leyeron {len(current['parents'])}"
                    )
                segments.append(
                    Segment(
                        current["ordinal"],
                        current["virtual"],
                        tuple(current["parents"]),
                        current["children"],
                    )
                )
            ordinal = require_int(row.get("ordinal"), "segment.ordinal")
            if ordinal != len(segments):
                raise TraceError(
                    f"{path}: segment.ordinal={ordinal}, se esperaba {len(segments)}"
                )
            virtual = require_object(row.get("virtual"), "segment.virtual")
            virtual_values = {
                field: require_int(virtual.get(field), f"segment.virtual.{field}")
                for field in ("left", "top", "width", "height")
            }
            current = {
                "row": row,
                "ordinal": ordinal,
                "virtual": virtual_values,
                "children": require_int(row.get("children"), "segment.children"),
                "parents": [],
            }
            continue

        if kind != "parent" or current is None:
            raise TraceError(f"{path}: fila inesperada kind={kind!r}")
        final_ordinal = require_int(row.get("final_ordinal"), "parent.final_ordinal")
        parents: list[Parent] = current["parents"]
        if final_ordinal != len(parents):
            raise TraceError(
                f"{path}: parent.final_ordinal={final_ordinal}, se esperaba {len(parents)}"
            )
        raw_image = require_int(row.get("image"), "parent.image")
        # ParentSpriteToDraw puede transportar flags de imagen en los bits altos.
        sprite = raw_image & 0x3FFFFFFF
        parents.append(
            Parent(
                current["ordinal"],
                final_ordinal,
                sprite,
                bounds_from_parent(row, "parent"),
            )
        )

    if current is not None:
        expected = require_int(current["row"].get("parents"), "segment.parents")
        if expected != len(current["parents"]):
            raise TraceError(
                f"{path}: segment {current['ordinal']} declara {expected} parents, "
                f"se leyeron {len(current['parents'])}"
            )
        segments.append(
            Segment(
                current["ordinal"],
                current["virtual"],
                tuple(current["parents"]),
                current["children"],
            )
        )

    expected_segments = require_int(complete.get("segments"), "complete.segments")
    expected_parents = require_int(complete.get("parents"), "complete.parents")
    actual_parents = sum(len(segment.parents) for segment in segments)
    if expected_segments != len(segments) or expected_parents != actual_parents:
        raise TraceError(
            f"{path}: complete declara segments={expected_segments}, parents={expected_parents}; "
            f"se leyeron segments={len(segments)}, parents={actual_parents}"
        )
    if not segments:
        raise TraceError(f"{path}: no hay segmentos")
    return ScreenshotSortTrace(path, metadata, tuple(segments), complete)


def load_candidate(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise TraceError(f"no se pudo leer candidata {path}: {error}") from error
    candidate = require_object(value, "candidata")
    if candidate.get("contract") != "openttdrs-viewport-sort":
        raise TraceError(f"{path}: se esperaba contract=openttdrs-viewport-sort")
    if candidate.get("stage") != "post_viewport_sprite_sorter":
        raise TraceError(f"{path}: stage no es post_viewport_sprite_sorter")
    parents = candidate.get("parents")
    if not isinstance(parents, list):
        raise TraceError(f"{path}: parents debe ser una lista")
    local_proxies = candidate.get("local_proxies", [])
    if not isinstance(local_proxies, list):
        raise TraceError(f"{path}: local_proxies debe ser una lista cuando está presente")
    return candidate


def candidate_sort_entries(
    candidate: dict[str, Any],
) -> list[tuple[str, int, dict[str, Any]]]:
    entries: list[tuple[str, int, dict[str, Any]]] = []
    for field in ("parents", "local_proxies"):
        for index, raw in enumerate(candidate.get(field, [])):
            entries.append((field, index, require_object(raw, f"candidate.{field}[{index}]")))
    return entries


def candidate_identities(candidate: dict[str, Any]) -> tuple[set[tuple[int, Bounds]], set[Bounds]]:
    identities: set[tuple[int, Bounds]] = set()
    bounds: set[Bounds] = set()
    for field, index, parent in candidate_sort_entries(candidate):
        label = f"candidate.{field}[{index}]"
        sprite = require_int(parent.get("sprite_id"), f"{label}.sprite_id")
        identity = (sprite, bounds_from_parent(parent, label))
        identities.add(identity)
        bounds.add(identity[1])
    return identities, bounds


def analyze(
    trace: ScreenshotSortTrace, candidate: dict[str, Any] | None = None
) -> dict[str, Any]:
    parents = trace.parents
    by_bounds: defaultdict[Bounds, defaultdict[int, list[int]]] = defaultdict(
        lambda: defaultdict(list)
    )
    for parent in parents:
        by_bounds[parent.bounds][parent.sprite].append(parent.segment)

    promotions = []
    for bounds, sprites in sorted(by_bounds.items()):
        if len(sprites) < 2:
            continue
        promotions.append(
            {
                "world_bounds": dict(
                    zip(("xmin", "ymin", "zmin", "xmax", "ymax", "zmax"), bounds)
                ),
                "sprites": [
                    {"sprite_id": sprite, "segments": sorted(set(segments))}
                    for sprite, segments in sorted(sprites.items())
                ],
                "extra_sprite_variants": len(sprites) - 1,
            }
        )

    report: dict[str, Any] = {
        "schema_version": 1,
        "kind": "viewport-sort-bands-analysis",
        "status": "diagnostic",
        "reference": {
            "path": str(trace.path),
            "metadata": trace.metadata,
            "segments": [
                {
                    "ordinal": segment.ordinal,
                    "virtual": segment.virtual,
                    "parents": len(segment.parents),
                    "children": segment.children,
                }
                for segment in trace.segments
            ],
        },
        "summary": {
            "segments": len(trace.segments),
            "parents": len(parents),
            "unique_parent_identities": len({(p.sprite, p.bounds) for p in parents}),
            "unique_world_bounds": len(by_bounds),
            "promoted_world_bounds": len(promotions),
            "extra_sprite_variants": sum(item["extra_sprite_variants"] for item in promotions),
        },
        "promotions": promotions,
    }

    if candidate is not None:
        candidate_identities_set, candidate_bounds = candidate_identities(candidate)
        reference_identities = {(parent.sprite, parent.bounds) for parent in parents}
        reference_bounds = set(by_bounds)
        same_bounds_different_sprite = sorted(
            (sprite, bounds)
            for sprite, bounds in reference_identities - candidate_identities_set
            if bounds in candidate_bounds
        )
        report["candidate"] = {
            "parents": len(candidate["parents"]),
            "local_proxies": len(candidate.get("local_proxies", [])),
            "effective_parents": len(candidate["parents"])
            + len(candidate.get("local_proxies", [])),
            "unique_parent_identities": len(candidate_identities_set),
            "unique_world_bounds": len(candidate_bounds),
            "reference_identities_not_in_candidate": len(reference_identities - candidate_identities_set),
            "reference_bounds_not_in_candidate": len(reference_bounds - candidate_bounds),
            "candidate_bounds_not_in_reference": len(candidate_bounds - reference_bounds),
            "same_bounds_different_sprite": len(same_bounds_different_sprite),
        }
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="trace JSONL world-screenshot-sort de OpenTTD")
    parser.add_argument(
        "--candidate-sort",
        type=Path,
        help="documento JSON de OPENTTDRS_VIEWPORT_SORT_TRACE_OUT",
    )
    parser.add_argument("--json-report", type=Path, help="escribe el informe estructurado")
    args = parser.parse_args(argv)

    try:
        trace = load_trace(args.reference)
        candidate = load_candidate(args.candidate_sort) if args.candidate_sort else None
        report = analyze(trace, candidate)
    except TraceError as error:
        parser.error(str(error))

    if args.json_report:
        args.json_report.parent.mkdir(parents=True, exist_ok=True)
        args.json_report.write_text(
            json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    summary = report["summary"]
    print(
        "viewport-sort-bands: "
        f"{summary['segments']} segmentos, {summary['parents']} parents, "
        f"{summary['promoted_world_bounds']} cajas promovidas, "
        f"{summary['extra_sprite_variants']} variantes extra"
    )
    candidate_summary = report.get("candidate")
    if candidate_summary:
        print(
            "candidata: "
            f"{candidate_summary['parents']} parents + "
            f"{candidate_summary['local_proxies']} proxies locales "
            f"({candidate_summary['effective_parents']} efectivos), "
            f"{candidate_summary['same_bounds_different_sprite']} con misma caja y sprite distinto, "
            f"{candidate_summary['reference_bounds_not_in_candidate']} cajas de referencia ausentes"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
