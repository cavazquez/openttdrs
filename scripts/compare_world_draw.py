#!/usr/bin/env python3
"""Compara la selección visual `world-draw` de OpenTTD y openttdrs (#307).

El C++ registra *todos* los comandos reales de ``draw_tile_proc``. El cliente
Rust, durante la primera iteración, sólo instrumenta las familias que tienen
anomalías activas (árboles, riel, catenaria, túneles, puentes y depósito
naval). Por eso la comparación es de selección contenida:

* falla si una tesela/ID del candidato no existe en el oráculo;
* falla si el candidato reporta un fallback;
* informa, sin fallar por defecto, comandos del oráculo aún no cubiertos.

Cuando la cobertura llegue a todos los spawners puede activarse
``--strict-reference`` para exigir igualdad de selección visual por tesela.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


EMPTY_PRIMITIVES = {"empty_bounds", "combine_start", "combine_end"}
METADATA_FIELDS = ("schema_version", "contract", "width", "height", "region")


class StreamError(RuntimeError):
    """El stream no cumple el contrato JSONL mínimo de world-draw."""


@dataclass(frozen=True)
class Row:
    line: int
    value: dict[str, Any]


@dataclass
class Stream:
    path: Path
    metadata: Row
    complete: Row
    tiles: list[Row]
    foundations: list[Row]
    draws: list[Row]


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
    return rows


def load_stream(path: Path) -> Stream:
    rows = read_rows(path)
    if not rows:
        raise StreamError(f"{path}: stream vacío")
    metadata = rows[0]
    if metadata.value.get("kind") != "metadata":
        raise StreamError(f"{path}:{metadata.line}: la primera fila debe ser kind=metadata")
    complete = rows[-1]
    if complete.value.get("kind") != "complete":
        raise StreamError(f"{path}:{complete.line}: falta fila final kind=complete")

    tiles: list[Row] = []
    foundations: list[Row] = []
    draws: list[Row] = []
    for row in rows[1:-1]:
        kind = row.value.get("kind")
        if kind == "tile":
            tiles.append(row)
        elif kind == "foundation":
            foundations.append(row)
        elif kind == "draw":
            draws.append(row)
        else:
            raise StreamError(f"{path}:{row.line}: fila inesperada kind={kind!r}")

    expected_tiles = complete.value.get("tiles")
    expected_draws = complete.value.get("draws")
    if expected_tiles != len(tiles) or expected_draws != len(draws):
        raise StreamError(
            f"{path}:{complete.line}: complete dice tiles={expected_tiles}, draws={expected_draws}; "
            f"se leyeron tiles={len(tiles)}, draws={len(draws)}"
        )
    return Stream(path, metadata, complete, tiles, foundations, draws)


def coord(row: Row) -> tuple[int, int]:
    x, y = row.value.get("x"), row.value.get("y")
    if not isinstance(x, int) or not isinstance(y, int):
        raise StreamError(f"fila {row.line}: faltan x/y enteros")
    return x, y


def sprite_id(row: Row) -> int | None:
    sprite = row.value.get("sprite")
    if not isinstance(sprite, dict):
        return None
    sid = sprite.get("id")
    return sid if isinstance(sid, int) else None


def geometry_signature(row: Row) -> tuple[Any, Any, Any, Any]:
    """Campos que distinguen un comando correcto pero colocado incorrectamente.

    La primitiva es necesaria para los children: ``world=null`` y ``bounds=null``
    por sí solos no permiten distinguir un ``AddChildSpriteScreen`` de un
    ground mal trazado.
    """
    value = row.value
    return (
        value.get("primitive"),
        value.get("world"),
        value.get("offset"),
        value.get("bounds"),
    )


def palette(row: Row) -> int:
    """Paleta lógica de la traza; los registros viejos omiten/usan cero."""
    value = row.value.get("palette", 0)
    return value if isinstance(value, int) else 0


def has_explicit_geometry(row: Row) -> bool:
    """El candidato sólo exige geometría donde ya la instrumentó.

    ``geometry_explicit`` cubre children de fundación: su geometría relevante
    es ``world=null`` + offset de pantalla, por lo que no tienen bounds. Los
    streams previos siguen siendo válidos mediante el fallback de ``bounds``.
    """
    return row.value.get("geometry_explicit") is True or isinstance(row.value.get("bounds"), dict)


def visual_draws(rows: Iterable[Row]) -> list[Row]:
    return [
        row
        for row in rows
        if row.value.get("primitive") not in EMPTY_PRIMITIVES and sprite_id(row) is not None
    ]


def metadata_differences(reference: Stream, candidate: Stream) -> list[str]:
    left, right = reference.metadata.value, candidate.metadata.value
    differences = [
        field for field in METADATA_FIELDS if left.get(field) != right.get(field)
    ]
    left_hash, right_hash = left.get("save_sha256"), right.get("save_sha256")
    if left_hash and right_hash and left_hash != right_hash:
        differences.append("save_sha256")
    if left.get("producer") != "openttd":
        differences.append("reference.producer")
    if right.get("producer") != "openttdrs":
        differences.append("candidate.producer")
    return differences


def tiles_by_coord(rows: list[Row]) -> dict[tuple[int, int], Row]:
    result: dict[tuple[int, int], Row] = {}
    for row in rows:
        key = coord(row)
        if key in result:
            raise StreamError(f"fila {row.line}: tesela duplicada x={key[0]}, y={key[1]}")
        result[key] = row
    return result


def draws_by_coord(rows: list[Row]) -> dict[tuple[int, int], list[Row]]:
    result: dict[tuple[int, int], list[Row]] = defaultdict(list)
    for row in rows:
        result[coord(row)].append(row)
    return result


def foundation_signature(row: Row) -> tuple[Any, ...]:
    """Campos calculados por `DrawFoundation` antes de seleccionar un sprite."""
    value = row.value
    return tuple(
        value.get(field)
        for field in (
            "foundation",
            "foundation_tileh",
            "foundation_base_z",
            "sprite_block",
            "has_nw",
            "has_ne",
            "nw_w_here",
            "nw_n_here",
            "nw_w_neighbour",
            "nw_n_neighbour",
            "ne_e_here",
            "ne_n_here",
            "ne_e_neighbour",
            "ne_n_neighbour",
        )
    )


def focused(coord_value: tuple[int, int], where: set[tuple[int, int]]) -> bool:
    return not where or coord_value in where


def format_coord(value: tuple[int, int]) -> str:
    return f"x={value[0]}, y={value[1]}"


def compare(
    reference: Stream,
    candidate: Stream,
    where: set[tuple[int, int]],
    max_diffs: int,
    strict_reference: bool,
    geometry: bool,
    foundations: bool,
) -> tuple[
    list[str],
    Counter[str],
    Counter[str],
    Counter[str],
    Counter[str],
    dict[str, str],
    dict[str, str],
    dict[str, Counter[str]],
]:
    failures: list[str] = []
    summary: Counter[str] = Counter()
    uncovered: Counter[str] = Counter()
    unmatched_candidate: Counter[str] = Counter()
    unmatched_geometry: Counter[str] = Counter()
    unmatched_candidate_example: dict[str, str] = {}
    unmatched_geometry_example: dict[str, str] = {}
    by_role: defaultdict[str, Counter[str]] = defaultdict(Counter)

    metadata = metadata_differences(reference, candidate)
    for field in metadata:
        failures.append(f"metadata_mismatch: {field}")

    ref_tiles = tiles_by_coord(reference.tiles)
    cand_tiles = tiles_by_coord(candidate.tiles)
    all_coords = sorted(set(ref_tiles) | set(cand_tiles), key=lambda item: (item[1], item[0]))
    for key in all_coords:
        if not focused(key, where):
            continue
        left, right = ref_tiles.get(key), cand_tiles.get(key)
        if left is None:
            failures.append(f"missing_reference_tile en {format_coord(key)}")
            continue
        if right is None:
            failures.append(f"missing_candidate_tile en {format_coord(key)}")
            continue
        for field in ("index", "tile_type"):
            if left.value.get(field) != right.value.get(field):
                failures.append(
                    f"tile_{field}_mismatch en {format_coord(key)}: "
                    f"OpenTTD={left.value.get(field)!r}, openttdrs={right.value.get(field)!r}"
                )

    ref_draws = draws_by_coord(visual_draws(reference.draws))
    cand_draws = draws_by_coord(visual_draws(candidate.draws))
    for key in sorted(set(ref_draws) | set(cand_draws), key=lambda item: (item[1], item[0])):
        if not focused(key, where):
            continue
        reference_here = ref_draws.get(key, [])
        candidate_here = cand_draws.get(key, [])
        ref_ids = Counter(sprite_id(row) for row in reference_here)
        cand_ids = Counter(sprite_id(row) for row in candidate_here)
        # La contención es multiconjunto, no sólo de pertenencia. Dos capas
        # candidatas con el mismo ID no pueden ambas justificar la única
        # llamada equivalente del oráculo; de otro modo los duplicados de
        # puentes/catenaria quedan invisibles para este contrato.
        available_ref_ids = ref_ids.copy()
        summary["reference_visual_draws"] += len(reference_here)
        summary["candidate_selected_draws"] += len(candidate_here)

        for row in candidate_here:
            sid = sprite_id(row)
            assert sid is not None
            role = str(row.value.get("role", "unknown"))
            role_summary = by_role[role]
            role_summary["selected"] += 1
            if row.value.get("fallback") is True:
                role_summary["fallback"] += 1
                failures.append(
                    f"candidate_fallback en {format_coord(key)}: "
                    f"{row.value.get('role')} sprite={sid}"
                )
            if available_ref_ids[sid] == 0:
                role_summary["missing"] += 1
                label = f"role={row.value.get('role')} sprite={sid}"
                unmatched_candidate[label] += 1
                unmatched_candidate_example.setdefault(label, format_coord(key))
                failures.append(
                    f"candidate_sprite_missing_in_reference en {format_coord(key)}: "
                    f"{row.value.get('role')} sprite={sid}"
                )
            else:
                available_ref_ids[sid] -= 1
                role_summary["ids_contained"] += 1
                summary["candidate_ids_contained"] += 1
            # Paleta cero significa "no instrumentada / sin modificador" y
            # mantiene la compatibilidad de los spawners que todavía no
            # registran recolor. Para una paleta explícita (PBS, compañía),
            # sí exigimos la misma decisión del oráculo.
            if palette(row) != 0:
                role_summary["palette_explicit"] += 1
                matching_palette = any(
                    sprite_id(reference_row) == sid and palette(reference_row) == palette(row)
                    for reference_row in reference_here
                )
                if matching_palette:
                    role_summary["palette_matched"] += 1
                    summary["candidate_palettes_matched"] += 1
                else:
                    failures.append(
                        f"candidate_palette_missing_in_reference en {format_coord(key)}: "
                        f"{row.value.get('role')} sprite={sid} palette={palette(row)}"
                    )
            if geometry and has_explicit_geometry(row):
                role_summary["geometry_explicit"] += 1
                matching = any(
                    sprite_id(reference_row) == sid
                    and geometry_signature(reference_row) == geometry_signature(row)
                    for reference_row in reference_here
                )
                if matching:
                    role_summary["geometry_matched"] += 1
                    summary["candidate_geometry_matched"] += 1
                else:
                    label = f"role={row.value.get('role')} sprite={sid}"
                    unmatched_geometry[label] += 1
                    unmatched_geometry_example.setdefault(label, format_coord(key))
                    failures.append(
                        f"candidate_geometry_missing_in_reference en {format_coord(key)}: "
                        f"{row.value.get('role')} sprite={sid} "
                        f"geometry={geometry_signature(row)!r}"
                    )

        missing_from_candidate = ref_ids - cand_ids
        for sid, amount in missing_from_candidate.items():
            uncovered[f"sprite={sid}"] += amount
            if strict_reference:
                failures.append(
                    f"reference_sprite_missing_in_candidate en {format_coord(key)}: "
                    f"sprite={sid} x{amount}"
                )

    if foundations:
        ref_foundations = draws_by_coord(reference.foundations)
        cand_foundations = draws_by_coord(candidate.foundations)
        for key in sorted(set(cand_foundations), key=lambda item: (item[1], item[0])):
            if not focused(key, where):
                continue
            expected = {foundation_signature(row) for row in ref_foundations.get(key, [])}
            for row in cand_foundations[key]:
                signature = foundation_signature(row)
                if signature in expected:
                    summary["candidate_foundations_matched"] += 1
                else:
                    failures.append(
                        f"candidate_foundation_missing_in_reference en {format_coord(key)}: "
                        f"signature={signature!r}"
                    )

    if len(failures) > max_diffs:
        omitted = len(failures) - max_diffs
        failures = failures[:max_diffs] + [f"… {omitted} diferencias adicionales omitidas"]
    return (
        failures,
        summary,
        uncovered,
        unmatched_candidate,
        unmatched_geometry,
        unmatched_candidate_example,
        unmatched_geometry_example,
        dict(by_role),
    )


def parse_where(values: list[str]) -> set[tuple[int, int]]:
    out: set[tuple[int, int]] = set()
    for raw in values:
        try:
            x_text, y_text = raw.split(",", 1)
            out.add((int(x_text), int(y_text)))
        except ValueError as error:
            raise argparse.ArgumentTypeError("--where usa x,y") from error
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="JSONL de OpenTTD C++")
    parser.add_argument("candidate", type=Path, help="JSONL de openttdrs")
    parser.add_argument("--where", action="append", default=[], metavar="X,Y")
    parser.add_argument("--max-diffs", type=int, default=20)
    parser.add_argument("--strict-reference", action="store_true")
    parser.add_argument(
        "--report-unmatched",
        type=int,
        metavar="N",
        help="mostrar hasta N grupos de selecciones/geometrías candidatas sin correspondencia",
    )
    parser.add_argument(
        "--geometry",
        action="store_true",
        help="validar world/offset/bounds de los sprites candidatos que los informan",
    )
    parser.add_argument(
        "--foundations",
        action="store_true",
        help="comparar decisiones de fundación (pendiente, altura y bloque de paredes)",
    )
    parser.add_argument(
        "--by-role",
        action="store_true",
        help="desglosar por familia las selecciones, geometrías y paletas candidatas",
    )
    args = parser.parse_args(argv)
    if args.max_diffs < 1:
        parser.error("--max-diffs debe ser positivo")
    if args.report_unmatched is not None and args.report_unmatched < 1:
        parser.error("--report-unmatched debe ser positivo")

    try:
        reference = load_stream(args.reference)
        candidate = load_stream(args.candidate)
        where = parse_where(args.where)
        (
            failures,
            summary,
            uncovered,
            unmatched_candidate,
            unmatched_geometry,
            unmatched_candidate_example,
            unmatched_geometry_example,
            by_role,
        ) = compare(
            reference,
            candidate,
            where,
            args.max_diffs,
            args.strict_reference,
            args.geometry,
            args.foundations,
        )
    except StreamError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print(
        "world-draw: "
        f"{len(reference.tiles)} teselas OpenTTD, {len(candidate.tiles)} openttdrs; "
        f"{summary['reference_visual_draws']} comandos visuales de referencia, "
        f"{summary['candidate_selected_draws']} selecciones candidatas"
    )
    print(f"IDs candidatos contenidos en OpenTTD: {summary['candidate_ids_contained']}")
    if args.geometry:
        print(
            "Geometrías candidatas explícitas contenidas en OpenTTD: "
            f"{summary['candidate_geometry_matched']}"
        )
    if summary["candidate_palettes_matched"]:
        print(
            "Paletas candidatas explícitas contenidas en OpenTTD: "
            f"{summary['candidate_palettes_matched']}"
        )
    if args.foundations:
        print(
            "Decisiones de fundación candidatas contenidas en OpenTTD: "
            f"{summary['candidate_foundations_matched']}"
        )
    if args.by_role:
        print("Cobertura candidata por familia:")
        for role, counts in sorted(
            by_role.items(), key=lambda item: (-item[1]["selected"], item[0])
        ):
            fields = [
                f"selecciones={counts['selected']}",
                f"IDs={counts['ids_contained']}",
            ]
            if args.geometry:
                fields.append(
                    "geometrías="
                    f"{counts['geometry_matched']}/{counts['geometry_explicit']}"
                )
            if counts["palette_explicit"]:
                fields.append(
                    "paletas="
                    f"{counts['palette_matched']}/{counts['palette_explicit']}"
                )
            if counts["missing"] or counts["fallback"]:
                fields.append(
                    f"sin_oráculo={counts['missing']} fallback={counts['fallback']}"
                )
            print(f"  {role}: " + ", ".join(fields))
    if uncovered:
        examples = ", ".join(
            f"{label}×{amount}" for label, amount in uncovered.most_common(8)
        )
        print(
            "Comandos de referencia aún sin selección candidata equivalente: "
            f"{sum(uncovered.values())} ({examples})"
        )
    if args.report_unmatched:
        if unmatched_candidate:
            examples = ", ".join(
                f"{label}×{amount} ({unmatched_candidate_example[label]})"
                for label, amount in unmatched_candidate.most_common(args.report_unmatched)
            )
            print(
                "Selecciones candidatas sin sprite equivalente en OpenTTD: "
                f"{sum(unmatched_candidate.values())} ({examples})"
            )
        if args.geometry and unmatched_geometry:
            examples = ", ".join(
                f"{label}×{amount} ({unmatched_geometry_example[label]})"
                for label, amount in unmatched_geometry.most_common(args.report_unmatched)
            )
            print(
                "Geometrías candidatas sin equivalente en OpenTTD: "
                f"{sum(unmatched_geometry.values())} ({examples})"
            )
    if failures:
        for failure in failures:
            print(f"DIFF: {failure}")
        return 1
    print("OK: selección candidata contenida en el draw proc de OpenTTD")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
