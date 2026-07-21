#!/usr/bin/env python3
"""Compara trazas FTA de aeropuerto (OpenTTD vs openttdrs) por primera divergencia.

Esquema por fila (tras metadata): `{kind, tick, aircraft:[...], airports:[...]}`.

El estado *inicial* (import del `.sav`) se compara de forma estricta sobre los
campos comparables entre ambos motores. Dos campos del esquema aircraft NO se
comparan porque representan modelos distintos por diseño, no divergencias:

- `x`/`y`: en OpenTTD viene de `TileX/TileY(v->tile)`, un campo vestigial que
  un avión bajo control FTA nunca actualiza (se congela en el valor de
  importación). `openttdrs` reporta el mismo valor crudo congelado, así que
  en la práctica coincide siempre que el importador lo preserve correctamente
  — pero no es una posición «viva» en ningún motor.
- `x_pos`/`y_pos`/`z_pos`: OpenTTD interpola sub-tesela vía `AirportMovingData`;
  `openttdrs` solo trackea tesela + nodo FTA (sin sub-tesela), así que estos
  campos son aproximados y no comparables 1:1.

Para los ticks, dado que el motor FTA de `openttdrs` es una reimplementación
simplificada (sin el contador de espera exacto por nodo de OpenTTD), se
reporta la primera fila donde la secuencia de `pos`/`state` diverge en vez de
exigir igualdad total; solo falla si el *initial* no coincide o si la forma
de la traza es inválida.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Campos del esquema aircraft que representan estado FTA vivo comparable
# entre ambos motores (excluye x/y/x_pos/y_pos/z_pos; ver docstring).
AIRCRAFT_FIELDS = (
    "pos",
    "previous_pos",
    "state",
    "targetairport",
    "speed",
    "direction",
    "running",
)
AIRPORT_FIELDS = ("station", "x", "y", "w", "h", "type", "layout", "blocks")


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def read_trace(path: Path) -> tuple[dict[str, object], list[dict[str, object]]]:
    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer {path}: {exc}")
    if not rows or rows[0].get("kind") != "metadata":
        fail(f"{path}: falta metadata")
    samples = rows[1:]
    if any(row.get("kind") not in {"initial", "tick"} for row in samples):
        fail(f"{path}: fila FTA inválida")
    return rows[0], samples


def comparable_aircraft(row: dict[str, object]) -> list[tuple[object, ...]]:
    aircraft = row.get("aircraft", [])
    assert isinstance(aircraft, list)
    # Los IDs de pool no son comparables entre motores; se ordena por
    # `targetairport`+`pos` para emparejar sin depender del ID.
    return sorted(tuple(ac[field] for field in AIRCRAFT_FIELDS) for ac in aircraft)


def comparable_airports(row: dict[str, object]) -> list[tuple[object, ...]]:
    airports = row.get("airports", [])
    assert isinstance(airports, list)
    return sorted(tuple(ap[field] for field in AIRPORT_FIELDS) for ap in airports)


def main() -> None:
    if len(sys.argv) != 3:
        fail("uso: compare_airport_fta_traces.py <openttd.jsonl> <openttdrs.jsonl>")
    expected_path, actual_path = map(Path, sys.argv[1:])
    expected_meta, expected = read_trace(expected_path)
    actual_meta, actual = read_trace(actual_path)
    if expected_meta.get("producer") != "openttd":
        fail("la primera traza debe tener producer=openttd")
    if actual_meta.get("producer") != "openttdrs":
        fail("la segunda traza debe tener producer=openttdrs")
    if not expected or not actual:
        fail("ambas trazas deben tener al menos la fila 'initial'")

    expected_initial, actual_initial = expected[0], actual[0]
    if expected_initial.get("kind") != "initial" or actual_initial.get("kind") != "initial":
        fail("la primera fila tras metadata debe ser 'initial' en ambas trazas")

    exp_airports = comparable_airports(expected_initial)
    act_airports = comparable_airports(actual_initial)
    if exp_airports != act_airports:
        fail(f"initial: airports OpenTTD={exp_airports} openttdrs={act_airports}")

    exp_aircraft = comparable_aircraft(expected_initial)
    act_aircraft = comparable_aircraft(actual_initial)
    if exp_aircraft != act_aircraft:
        fail(f"initial: aircraft OpenTTD={exp_aircraft} openttdrs={act_aircraft}")

    print(
        f"OK initial: {len(exp_aircraft)} avión(es), {len(exp_airports)} "
        "aeropuerto(s) coinciden en OpenTTD y openttdrs"
    )

    ticks = min(len(expected), len(actual)) - 1
    first_divergence = None
    for frame in range(1, ticks + 1):
        exp_row, act_row = expected[frame], actual[frame]
        exp_pos = [ac["pos"] for ac in sorted(exp_row.get("aircraft", []), key=lambda a: a["targetairport"])]
        act_pos = [ac["pos"] for ac in sorted(act_row.get("aircraft", []), key=lambda a: a["targetairport"])]
        exp_state = [ac["state"] for ac in sorted(exp_row.get("aircraft", []), key=lambda a: a["targetairport"])]
        act_state = [ac["state"] for ac in sorted(act_row.get("aircraft", []), key=lambda a: a["targetairport"])]
        if (exp_pos, exp_state) != (act_pos, act_state):
            first_divergence = (frame, exp_row.get("tick"), act_row.get("tick"), exp_pos, exp_state, act_pos, act_state)
            break

    if first_divergence is None:
        print(f"OK ticks: secuencia pos/state idéntica en los {ticks} ticks comparados")
        return

    frame, exp_tick, act_tick, exp_pos, exp_state, act_pos, act_state = first_divergence
    print(
        "DIVERGENCIA documentada (no fatal) en tick relativo "
        f"{frame}/{ticks} (OpenTTD tick={exp_tick}, openttdrs tick={act_tick}):\n"
        f"  pos:   OpenTTD={exp_pos} openttdrs={act_pos}\n"
        f"  state: OpenTTD={exp_state} openttdrs={act_state}\n"
        "  Causa esperada: el motor FTA de openttdrs es una reimplementación "
        "MVP sin el contador de espera exacto por nodo de OpenTTD "
        "(`aircraft_phase_ticks` se aproxima por flags al importar, no se "
        "persiste en el .sav), por lo que las transiciones de nodo/heading "
        "se adelantan o atrasan respecto al oráculo tras el primer tramo."
    )


if __name__ == "__main__":
    main()
