#!/usr/bin/env python3
"""Genera un backlog accionable desde dos trazas ``world-draw`` (#307).

La comparación normal prueba el contrato y explica una región. Esta utilidad
resume el mapa entero: comprueba que ambos lados cubrieron exactamente las
mismas teselas, agrupa selecciones candidatas que no existen en OpenTTD y
prioriza los casos por familia, sprite, geometría y primera coordenada.

No convierte la cobertura parcial del renderer en una falsa igualdad de
píxeles: los comandos C++ no instrumentados por Rust se muestran como
``uncovered_reference`` y no cuentan como errores candidatos.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from compare_world_draw import (
    Stream,
    StreamError,
    geometry_signature,
    has_explicit_geometry,
    load_stream,
    matches_ordered_draw,
    palette,
    sprite_id,
    visual_draws,
)


@dataclass(frozen=True)
class FindingKey:
    kind: str
    role: str
    sprite: int


@dataclass
class Finding:
    count: int = 0
    first_coord: tuple[int, int] | None = None
    examples: list[tuple[int, int]] | None = None

    def add(self, coord: tuple[int, int], example_limit: int) -> None:
        self.count += 1
        if self.first_coord is None or (coord[1], coord[0]) < (
            self.first_coord[1],
            self.first_coord[0],
        ):
            self.first_coord = coord
        if self.examples is None:
            self.examples = []
        if len(self.examples) < example_limit and coord not in self.examples:
            self.examples.append(coord)


def coord(value: dict[str, Any]) -> tuple[int, int]:
    x, y = value.get("x"), value.get("y")
    if not isinstance(x, int) or not isinstance(y, int):
        raise StreamError("fila sin x/y enteros")
    return x, y


def coords(stream: Stream) -> set[tuple[int, int]]:
    return {coord(row.value) for row in stream.tiles}


def draw_groups(stream: Stream) -> dict[tuple[int, int], list[Any]]:
    grouped: dict[tuple[int, int], list[Any]] = defaultdict(list)
    for row in visual_draws(stream.draws):
        grouped[coord(row.value)].append(row)
    return grouped


def add_finding(
    findings: dict[FindingKey, Finding],
    key: FindingKey,
    where: tuple[int, int],
    example_limit: int,
) -> None:
    findings.setdefault(key, Finding()).add(where, example_limit)


def audit(reference: Stream, candidate: Stream, example_limit: int) -> dict[str, Any]:
    reference_coords = coords(reference)
    candidate_coords = coords(candidate)
    missing_candidate_tiles = sorted(reference_coords - candidate_coords, key=lambda item: (item[1], item[0]))
    extra_candidate_tiles = sorted(candidate_coords - reference_coords, key=lambda item: (item[1], item[0]))

    ref_groups = draw_groups(reference)
    candidate_groups = draw_groups(candidate)
    findings: dict[FindingKey, Finding] = {}
    role_totals: dict[str, Counter[str]] = defaultdict(Counter)
    uncovered_reference = Counter()

    for where in sorted(reference_coords | candidate_coords, key=lambda item: (item[1], item[0])):
        reference_here = ref_groups.get(where, [])
        candidate_here = candidate_groups.get(where, [])
        available = Counter(sprite_id(row) for row in reference_here)
        statuses: list[set[str]] = [set() for _ in candidate_here]

        for index, row in enumerate(candidate_here):
            value = row.value
            sid = sprite_id(row)
            if sid is None:
                continue
            if value.get("fallback") is True:
                statuses[index].add("fallback")
            if available[sid] == 0:
                statuses[index].add("missing_sprite")
            else:
                available[sid] -= 1
            if has_explicit_geometry(row):
                if any(
                    sprite_id(reference_row) == sid
                    and geometry_signature(reference_row) == geometry_signature(row)
                    for reference_row in reference_here
                ):
                    pass
                else:
                    statuses[index].add("missing_geometry")
            if palette(row) != 0:
                if any(
                    sprite_id(reference_row) == sid and palette(reference_row) == palette(row)
                    for reference_row in reference_here
                ):
                    pass
                else:
                    statuses[index].add("missing_palette")

        reference_index = 0
        for index, row in enumerate(candidate_here):
            while reference_index < len(reference_here) and not matches_ordered_draw(
                reference_here[reference_index], row, geometry=True
            ):
                reference_index += 1
            if reference_index == len(reference_here):
                statuses[index].add("missing_order")
                break
            reference_index += 1

        for index, row in enumerate(candidate_here):
            value = row.value
            role = str(value.get("role", "unknown"))
            sid = sprite_id(row)
            if sid is None:
                continue
            role_totals[role]["selected"] += 1
            if "missing_sprite" in statuses[index]:
                role_totals[role]["missing_sprite"] += 1
            else:
                role_totals[role]["id_matched"] += 1
            if has_explicit_geometry(row):
                role_totals[role]["geometry_explicit"] += 1
                if "missing_geometry" in statuses[index]:
                    role_totals[role]["missing_geometry"] += 1
                else:
                    role_totals[role]["geometry_matched"] += 1
            if palette(row) != 0:
                role_totals[role]["palette_explicit"] += 1
                if "missing_palette" in statuses[index]:
                    role_totals[role]["missing_palette"] += 1
                else:
                    role_totals[role]["palette_matched"] += 1
            if "missing_order" in statuses[index]:
                role_totals[role]["missing_order"] += 1
            else:
                role_totals[role]["order_matched"] += 1
            if "fallback" in statuses[index]:
                role_totals[role]["fallback"] += 1

            # Una misma selección puede incumplir ID, geometría y orden a la
            # vez. Para decidir qué atacar primero cuenta una sola vez y usa
            # la causa más informativa; los contadores por columna conservan
            # toda la evidencia técnica.
            if statuses[index]:
                role_totals[role]["divergent_draws"] += 1
                kind = next(
                    cause
                    for cause in (
                        "fallback",
                        "missing_sprite",
                        "missing_geometry",
                        "missing_palette",
                        "missing_order",
                    )
                    if cause in statuses[index]
                )
                add_finding(findings, FindingKey(kind, role, sid), where, example_limit)

        candidate_ids = Counter(sprite_id(row) for row in candidate_here)
        for sid, amount in (Counter(sprite_id(row) for row in reference_here) - candidate_ids).items():
            uncovered_reference[f"sprite={sid}"] += amount

    serialized_findings = []
    for key, finding in sorted(
        findings.items(),
        key=lambda item: (-item[1].count, item[0].kind, item[0].role, item[0].sprite),
    ):
        serialized_findings.append(
            {
                "kind": key.kind,
                "role": key.role,
                "sprite": key.sprite,
                "count": finding.count,
                "first_coord": list(finding.first_coord) if finding.first_coord else None,
                "examples": [list(value) for value in finding.examples or []],
            }
        )

    role_summary: list[dict[str, Any]] = []
    for role, values in sorted(
        role_totals.items(),
        key=lambda item: (-item[1]["divergent_draws"], -item[1]["selected"], item[0]),
    ):
        selected = values["selected"]
        role_summary.append(
            {
                "role": role,
                "selected": selected,
                "id_matched": values["id_matched"],
                "missing_sprite": values["missing_sprite"],
                "missing_geometry": values["missing_geometry"],
                "missing_order": values["missing_order"],
                "fallback": values["fallback"],
                "priority_count": values["divergent_draws"],
            }
        )

    return {
        "schema_version": 1,
        "contract": "world-draw-audit",
        "reference": str(reference.path),
        "candidate": str(candidate.path),
        "reference_tiles": len(reference_coords),
        "candidate_tiles": len(candidate_coords),
        "missing_candidate_tiles": [list(value) for value in missing_candidate_tiles[:example_limit]],
        "missing_candidate_tile_count": len(missing_candidate_tiles),
        "extra_candidate_tiles": [list(value) for value in extra_candidate_tiles[:example_limit]],
        "extra_candidate_tile_count": len(extra_candidate_tiles),
        "role_summary": role_summary,
        "findings": serialized_findings,
        "unmatched_draw_count": sum(values["divergent_draws"] for values in role_totals.values()),
        "uncovered_reference": [
            {"sprite": label, "count": count}
            for label, count in uncovered_reference.most_common(20)
        ],
    }


def markdown(report: dict[str, Any], limit: int) -> str:
    lines = [
        "# Auditoría `world-draw`",
        "",
        "Generado por `scripts/audit_world_draw.py`; no es una comparación de píxeles.",
        "",
        "## Cobertura",
        "",
        f"- OpenTTD: **{report['reference_tiles']}** teselas.",
        f"- openttdrs: **{report['candidate_tiles']}** teselas.",
        f"- Faltantes en openttdrs: **{report['missing_candidate_tile_count']}**.",
        f"- Extras en openttdrs: **{report['extra_candidate_tile_count']}**.",
        "",
        "## Familias priorizadas",
        "",
        "| Familia | Selecciones | Sprite ausente | Geometría | Orden | Fallback | Desvíos únicos |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for role in report["role_summary"]:
        if role["priority_count"]:
            lines.append(
                "| {role} | {selected} | {missing_sprite} | {missing_geometry} | {missing_order} | {fallback} | {priority_count} |".format(**role)
            )
    lines += ["", "## Casos concretos", "", "| Tipo | Familia | Sprite | Casos | Primera tesela | Ejemplos |", "| --- | --- | ---: | ---: | --- | --- |"]
    for finding in report["findings"][:limit]:
        first = finding["first_coord"]
        first_text = "—" if first is None else f"({first[0]},{first[1]})"
        examples = ", ".join(f"({x},{y})" for x, y in finding["examples"])
        lines.append(
            f"| {finding['kind']} | {finding['role']} | {finding['sprite']} | {finding['count']} | {first_text} | {examples} |"
        )
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="JSONL world-draw de OpenTTD")
    parser.add_argument("candidate", type=Path, help="JSONL world-draw de openttdrs")
    parser.add_argument("--json-out", type=Path, help="escribir informe JSON reproducible")
    parser.add_argument("--markdown-out", type=Path, help="escribir informe Markdown")
    parser.add_argument("--limit", type=int, default=30, help="máximo de casos y ejemplos (default: 30)")
    args = parser.parse_args(argv)
    if args.limit < 1:
        parser.error("--limit debe ser positivo")
    try:
        report = audit(load_stream(args.reference), load_stream(args.candidate), args.limit)
    except StreamError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    rendered = markdown(report, args.limit)
    print(rendered)
    if args.json_out:
        args.json_out.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    if args.markdown_out:
        args.markdown_out.write_text(rendered + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
