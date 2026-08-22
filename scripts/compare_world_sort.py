#!/usr/bin/env python3
"""Diagnostica el orden global post-sort de OpenTTD contra `world-draw` (#323).

`world-draw` comprueba qué selecciona cada `draw_tile_proc`, pero no ejecuta
`ViewportSortParentSprites`. El stream opt-in `world-sort` registra ese vector
de padres *después* del sorter oficial. Este comparador normaliza los padres
sortables del candidato mediante su sprite, paleta y caja de mundo, y pregunta
si su orden de emisión es una subsecuencia del orden final de OpenTTD.

No convierte esa respuesta en una afirmación de paridad raster: el candidato
todavía puede tener pivotes, clipping, atlas o profundidad Bevy distintos. Su
valor es dar el primer padre que el compositor debe reordenar, o el primer
padre que falta antes de investigar el framebuffer.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any


class StreamError(RuntimeError):
    """El JSONL no respeta el contrato mínimo de world-sort/world-draw."""


@dataclass(frozen=True)
class Row:
    line: int
    value: dict[str, Any]


@dataclass(frozen=True, order=True)
class ParentIdentity:
    """Identidad estable de un padre antes de atlas o coordenadas de pantalla."""

    sprite: int
    palette: int
    bounds: tuple[int, int, int, int, int, int]

    def describe(self) -> str:
        xmin, ymin, zmin, xmax, ymax, zmax = self.bounds
        return (
            f"sprite={self.sprite} palette={self.palette} "
            f"bounds=({xmin},{ymin},{zmin})..({xmax},{ymax},{zmax})"
        )

    def json(self) -> dict[str, object]:
        xmin, ymin, zmin, xmax, ymax, zmax = self.bounds
        return {
            "sprite": self.sprite,
            "palette": self.palette,
            "world_bounds": {
                "xmin": xmin,
                "ymin": ymin,
                "zmin": zmin,
                "xmax": xmax,
                "ymax": ymax,
                "zmax": zmax,
            },
        }


@dataclass(frozen=True)
class ReferenceParent:
    final_ordinal: int
    parent_id: int
    identity: ParentIdentity
    row: Row


@dataclass(frozen=True)
class CandidateParent:
    input_ordinal: int
    identity: ParentIdentity
    x: int
    y: int
    draw_ordinal: int
    row: Row

    def describe(self) -> str:
        return (
            f"input={self.input_ordinal} tile=({self.x},{self.y}) "
            f"draw={self.draw_ordinal} {self.identity.describe()}"
        )

    def json(self, expected_final_ordinal: int | None = None) -> dict[str, object]:
        value: dict[str, object] = {
            "input_ordinal": self.input_ordinal,
            "tile": {"x": self.x, "y": self.y},
            "draw_ordinal": self.draw_ordinal,
            **self.identity.json(),
        }
        if expected_final_ordinal is not None:
            value["expected_final_ordinal"] = expected_final_ordinal
        return value


@dataclass
class WorldSort:
    path: Path
    metadata: Row
    parents: list[ReferenceParent]
    children: list[Row]
    complete: Row


@dataclass
class CandidateWorldDraw:
    path: Path
    metadata: Row
    parents: list[CandidateParent]
    complete: Row
    geometry_errors: list[str]


def is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def integer(row: Row, field: str, value: object) -> int:
    if not is_int(value):
        raise StreamError(f"{row.line}: {field} debe ser entero")
    return int(value)


def read_rows(path: Path) -> list[Row]:
    try:
        source = path.open(encoding="utf-8")
    except OSError as error:
        raise StreamError(f"no se pudo leer {path}: {error}") from error
    rows: list[Row] = []
    with source:
        for number, raw in enumerate(source, start=1):
            text = raw.strip()
            if not text:
                continue
            try:
                value = json.loads(text)
            except json.JSONDecodeError as error:
                raise StreamError(f"{path}:{number}: JSON inválido: {error.msg}") from error
            if not isinstance(value, dict):
                raise StreamError(f"{path}:{number}: cada fila debe ser un objeto JSON")
            rows.append(Row(number, value))
    if not rows:
        raise StreamError(f"{path}: stream vacío")
    return rows


def metadata(rows: list[Row], path: Path, contract: str, producer: str) -> tuple[Row, Row]:
    first, last = rows[0], rows[-1]
    if first.value.get("kind") != "metadata":
        raise StreamError(f"{path}:{first.line}: la primera fila debe ser kind=metadata")
    if first.value.get("contract") != contract:
        raise StreamError(f"{path}:{first.line}: se esperaba contract={contract!r}")
    if first.value.get("producer") != producer:
        raise StreamError(f"{path}:{first.line}: se esperaba producer={producer!r}")
    if last.value.get("kind") != "complete":
        raise StreamError(f"{path}:{last.line}: falta fila final kind=complete")
    return first, last


def sprite_id(row: Row) -> int:
    sprite = row.value.get("sprite")
    if not isinstance(sprite, dict):
        raise StreamError(f"{row.line}: sprite debe ser un objeto")
    return integer(row, "sprite.id", sprite.get("id"))


def palette(row: Row, *, resolved: bool = False) -> int:
    value = row.value.get("resolved_palette") if resolved else None
    if value is None:
        value = row.value.get("palette")
    return integer(row, "palette", value)


def bounds_from_sort(row: Row) -> ParentIdentity:
    raw = row.value.get("world_bounds")
    if not isinstance(raw, dict):
        raise StreamError(f"{row.line}: parent.world_bounds debe ser un objeto")
    fields = ("xmin", "ymin", "zmin", "xmax", "ymax", "zmax")
    values = tuple(integer(row, f"world_bounds.{field}", raw.get(field)) for field in fields)
    return ParentIdentity(sprite_id(row), palette(row), values)  # type: ignore[arg-type]


def bounds_from_candidate(row: Row) -> ParentIdentity:
    world = row.value.get("world")
    bounds = row.value.get("bounds")
    if not isinstance(world, dict) or not isinstance(bounds, dict):
        raise StreamError(f"{row.line}: parent candidato requiere world y bounds explícitos")
    x = integer(row, "world.x", world.get("x"))
    y = integer(row, "world.y", world.get("y"))
    z = integer(row, "world.z", world.get("z"))
    ox = integer(row, "bounds.ox", bounds.get("ox"))
    oy = integer(row, "bounds.oy", bounds.get("oy"))
    oz = integer(row, "bounds.oz", bounds.get("oz"))
    ex = integer(row, "bounds.ex", bounds.get("ex"))
    ey = integer(row, "bounds.ey", bounds.get("ey"))
    ez = integer(row, "bounds.ez", bounds.get("ez"))
    xmin, ymin, zmin = x + ox, y + oy, z + oz
    # Es exactamente la asignación de AddSortableSpriteToDraw. Un extent cero
    # puede dejar max < min; el sorter oficial también admite esa caja fina.
    return ParentIdentity(
        sprite_id(row),
        palette(row, resolved=True),
        (xmin, ymin, zmin, xmin + ex - 1, ymin + ey - 1, zmin + ez - 1),
    )


def load_world_sort(path: Path) -> WorldSort:
    rows = read_rows(path)
    head, tail = metadata(rows, path, "world-sort", "openttd")
    if head.value.get("stage") != "post_viewport_sprite_sorter":
        raise StreamError(f"{path}:{head.line}: stage no es post_viewport_sprite_sorter")
    if head.value.get("sorter") != "ViewportSortParentSprites":
        raise StreamError(f"{path}:{head.line}: sorter no es ViewportSortParentSprites")

    begin: Row | None = None
    parents: list[ReferenceParent] = []
    children: list[Row] = []
    seen_parent_ids: set[int] = set()
    for row in rows[1:-1]:
        kind = row.value.get("kind")
        if kind == "sort_begin":
            if begin is not None or parents or children:
                raise StreamError(f"{path}:{row.line}: sort_begin fuera de lugar")
            begin = row
            continue
        if begin is None:
            raise StreamError(f"{path}:{row.line}: falta sort_begin antes de {kind!r}")
        if kind == "parent":
            final_ordinal = integer(row, "final_ordinal", row.value.get("final_ordinal"))
            parent_id = integer(row, "parent_id", row.value.get("parent_id"))
            if final_ordinal != len(parents):
                raise StreamError(
                    f"{path}:{row.line}: final_ordinal={final_ordinal}, se esperaba {len(parents)}"
                )
            if parent_id in seen_parent_ids:
                raise StreamError(f"{path}:{row.line}: parent_id duplicado {parent_id}")
            seen_parent_ids.add(parent_id)
            parents.append(ReferenceParent(final_ordinal, parent_id, bounds_from_sort(row), row))
        elif kind == "child":
            children.append(row)
        else:
            raise StreamError(f"{path}:{row.line}: fila inesperada kind={kind!r}")
    if begin is None:
        raise StreamError(f"{path}: falta kind=sort_begin")
    expected_parents = integer(begin, "sort_begin.parents", begin.value.get("parents"))
    expected_children = integer(begin, "sort_begin.children", begin.value.get("children"))
    if expected_parents != len(parents) or expected_children != len(children):
        raise StreamError(
            f"{path}:{tail.line}: sort_begin dice parents={expected_parents}, children={expected_children}; "
            f"se leyeron parents={len(parents)}, children={len(children)}"
        )
    if (
        integer(tail, "complete.parents", tail.value.get("parents")) != len(parents)
        or integer(tail, "complete.children", tail.value.get("children")) != len(children)
    ):
        raise StreamError(f"{path}:{tail.line}: complete no coincide con sort_begin")

    parent_by_id = {parent.parent_id: parent for parent in parents}
    child_ordinal: defaultdict[int, int] = defaultdict(int)
    for child in children:
        parent_id = integer(child, "child.parent_id", child.value.get("parent_id"))
        final_ordinal = integer(
            child, "child.final_parent_ordinal", child.value.get("final_parent_ordinal")
        )
        if parent_id not in parent_by_id:
            raise StreamError(f"{path}:{child.line}: child refiere parent_id ausente {parent_id}")
        if parent_by_id[parent_id].final_ordinal != final_ordinal:
            raise StreamError(
                f"{path}:{child.line}: child final_parent_ordinal no coincide con parent_id={parent_id}"
            )
        ordinal = integer(child, "child.child_ordinal", child.value.get("child_ordinal"))
        if ordinal != child_ordinal[parent_id]:
            raise StreamError(
                f"{path}:{child.line}: child_ordinal={ordinal}, se esperaba {child_ordinal[parent_id]}"
            )
        child_ordinal[parent_id] += 1
    return WorldSort(path, head, parents, children, tail)


def load_candidate_world_draw(path: Path) -> CandidateWorldDraw:
    rows = read_rows(path)
    head, tail = metadata(rows, path, "world-draw", "openttdrs")
    expected_draws = integer(tail, "complete.draws", tail.value.get("draws"))
    actual_draws = sum(row.value.get("kind") == "draw" for row in rows[1:-1])
    if expected_draws != actual_draws:
        raise StreamError(
            f"{path}:{tail.line}: complete dice draws={expected_draws}; se leyeron {actual_draws}"
        )

    parents: list[CandidateParent] = []
    geometry_errors: list[str] = []
    for row in rows[1:-1]:
        if row.value.get("kind") != "draw":
            continue
        if row.value.get("primitive") not in {"sortable", "empty_bounds"}:
            continue
        try:
            identity = bounds_from_candidate(row)
            x = integer(row, "x", row.value.get("x"))
            y = integer(row, "y", row.value.get("y"))
            draw_ordinal = integer(row, "ordinal", row.value.get("ordinal"))
        except StreamError as error:
            geometry_errors.append(f"candidate_parent_invalid: {error}")
            continue
        parents.append(CandidateParent(len(parents), identity, x, y, draw_ordinal, row))
    return CandidateWorldDraw(path, head, parents, tail, geometry_errors)


def metadata_differences(reference: WorldSort, candidate: CandidateWorldDraw) -> list[str]:
    differences: list[str] = []
    for field in ("schema_version", "width", "height", "region"):
        if reference.metadata.value.get(field) != candidate.metadata.value.get(field):
            differences.append(f"metadata_mismatch: {field}")
    left_hash = reference.metadata.value.get("save_sha256")
    right_hash = candidate.metadata.value.get("save_sha256")
    if left_hash and right_hash and left_hash != right_hash:
        differences.append("metadata_mismatch: save_sha256")
    return differences


def count_descriptions(values: Counter[ParentIdentity]) -> dict[str, int]:
    return {identity.describe(): amount for identity, amount in sorted(values.items())}


def compare(
    reference: WorldSort,
    candidate: CandidateWorldDraw,
    strict_reference: bool,
    max_diffs: int,
) -> tuple[list[str], dict[str, object], dict[str, object] | None, Counter[ParentIdentity], Counter[ParentIdentity]]:
    failures = metadata_differences(reference, candidate)
    failures.extend(candidate.geometry_errors)
    ref_counts = Counter(parent.identity for parent in reference.parents)
    candidate_counts = Counter(parent.identity for parent in candidate.parents)
    uncovered_reference = ref_counts - candidate_counts
    unmatched_candidate = candidate_counts - ref_counts

    for identity, amount in unmatched_candidate.items():
        failures.append(f"candidate_parent_missing_in_reference: {identity.describe()} x{amount}")
    if strict_reference:
        for identity, amount in uncovered_reference.items():
            failures.append(f"reference_parent_missing_in_candidate: {identity.describe()} x{amount}")
    if reference.parents and not candidate.parents:
        failures.append("candidate_parent_stream_empty: OpenTTD emitió padres sortables")

    final_ordinals: defaultdict[ParentIdentity, deque[int]] = defaultdict(deque)
    for parent in reference.parents:
        final_ordinals[parent.identity].append(parent.final_ordinal)
    matched: list[tuple[CandidateParent, int]] = []
    for parent in candidate.parents:
        available = final_ordinals[parent.identity]
        if available:
            matched.append((parent, available.popleft()))

    first_inversion: dict[str, object] | None = None
    furthest: tuple[CandidateParent, int] | None = None
    for parent, expected in matched:
        if furthest is not None and expected < furthest[1]:
            first_inversion = {
                "before": furthest[0].json(furthest[1]),
                "after": parent.json(expected),
            }
            failures.append(
                "candidate_order_inversion: "
                f"{parent.describe()} espera final={expected}, pero se emite después de "
                f"final={furthest[1]} ({furthest[0].describe()})"
            )
            break
        if furthest is None or expected > furthest[1]:
            furthest = (parent, expected)

    moved = [parent for parent in reference.parents if parent.parent_id != parent.final_ordinal]
    summary: dict[str, object] = {
        "reference_parents": len(reference.parents),
        "reference_children": len(reference.children),
        "reference_reordered_parents": len(moved),
        "candidate_parent_candidates": len(candidate.parents),
        "matched_parent_candidates": len(matched),
        "uncovered_reference_parents": sum(uncovered_reference.values()),
        "unmatched_candidate_parents": sum(unmatched_candidate.values()),
    }
    if moved:
        summary["first_reference_reorder"] = {
            "final_ordinal": moved[0].final_ordinal,
            "parent_id": moved[0].parent_id,
            **moved[0].identity.json(),
        }
    if len(failures) > max_diffs:
        omitted = len(failures) - max_diffs
        failures = failures[:max_diffs] + [f"… {omitted} diferencias adicionales omitidas"]
    return failures, summary, first_inversion, uncovered_reference, unmatched_candidate


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="JSONL world-sort de OpenTTD")
    parser.add_argument("candidate", type=Path, help="JSONL world-draw de openttdrs")
    parser.add_argument("--strict-reference", action="store_true", help="falla también si OpenTTD tiene padres aún no instrumentados por el candidato")
    parser.add_argument("--max-diffs", type=int, default=20)
    parser.add_argument("--json-report", type=Path, help="escribe el diagnóstico estructurado")
    args = parser.parse_args(argv)
    if args.max_diffs < 1:
        parser.error("--max-diffs debe ser positivo")

    try:
        reference = load_world_sort(args.reference)
        candidate = load_candidate_world_draw(args.candidate)
        failures, summary, first_inversion, uncovered, unmatched = compare(
            reference, candidate, args.strict_reference, args.max_diffs
        )
    except StreamError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    report = {
        "schema_version": 1,
        "kind": "world-sort-compare",
        "status": "equivalent" if not failures else "different",
        "reference": {"path": str(reference.path), "metadata": reference.metadata.value},
        "candidate": {"path": str(candidate.path), "metadata": candidate.metadata.value},
        "options": {"strict_reference": args.strict_reference},
        "summary": summary,
        "first_inversion": first_inversion,
        "first_divergence": failures[0] if failures else None,
        "uncovered_reference": count_descriptions(uncovered),
        "unmatched_candidate": count_descriptions(unmatched),
        "failures": failures,
    }
    if args.json_report:
        args.json_report.parent.mkdir(parents=True, exist_ok=True)
        args.json_report.write_text(
            json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    print(
        "world-sort: "
        f"{len(reference.parents)} padres + {len(reference.children)} children OpenTTD; "
        f"{len(candidate.parents)} padres candidatos"
    )
    print(
        "Padres candidatos vinculados al orden final: "
        f"{summary['matched_parent_candidates']}"
    )
    if summary["reference_reordered_parents"]:
        print(
            "Padres que el sorter oficial movió respecto de inserción: "
            f"{summary['reference_reordered_parents']}"
        )
    if uncovered:
        print(
            "Padres de referencia aún sin candidato equivalente: "
            f"{sum(uncovered.values())}"
        )
    if failures:
        for failure in failures:
            print(f"DIFF: {failure}")
        return 1
    print("OK: los padres candidatos son una subsecuencia del orden final de OpenTTD")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
