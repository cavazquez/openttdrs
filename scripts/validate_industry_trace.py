#!/usr/bin/env python3
"""Valida el contrato JSONL del scheduler diario de industrias de OpenTTD."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def require_int(value: Any, field: str, *, minimum: int = 0) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        fail(f"{field} debe ser entero >= {minimum}")
    return value


def require_keys(row: dict[str, Any], fields: tuple[str, ...], kind: str) -> None:
    missing = [field for field in fields if field not in row]
    if missing:
        fail(f"fila {kind} sin campos: {', '.join(missing)}")


def validate_builddata(values: Any) -> None:
    if not isinstance(values, list) or len(values) != 240:
        fail("builddata debe contener las 240 filas ITBL")
    for index, entry in enumerate(values):
        if not isinstance(entry, dict):
            fail("fila ITBL inválida")
        require_keys(
            entry,
            ("type", "probability", "min_number", "target_count", "max_wait", "wait_count"),
            "ITBL",
        )
        if require_int(entry["type"], "ITBL.type") != index:
            fail("ITBL debe conservar el orden nativo por tipo")
        for field in ("probability", "min_number", "target_count", "max_wait", "wait_count"):
            require_int(entry[field], f"ITBL.{field}")


def validate_industries(values: Any) -> None:
    if not isinstance(values, list):
        fail("industries debe ser lista")
    previous_id = -1
    for industry in values:
        if not isinstance(industry, dict):
            fail("industria inválida")
        require_keys(
            industry,
            (
                "id",
                "type",
                "x",
                "y",
                "prod_level",
                "counter",
                "random",
                "selected_layout",
                "construction_type",
            ),
            "industry",
        )
        industry_id = require_int(industry["id"], "industry.id")
        if industry_id <= previous_id:
            fail("industries debe estar ordenada estrictamente por ID")
        previous_id = industry_id
        for field in ("type", "prod_level", "counter", "random", "selected_layout", "construction_type"):
            require_int(industry[field], f"industry.{field}")
        for field in ("x", "y"):
            if industry[field] is not None:
                require_int(industry[field], f"industry.{field}")


def validate_actions(values: Any, change_loop: int) -> None:
    if not isinstance(values, list) or len(values) != change_loop:
        fail("actions debe tener exactamente una fila por change_loop")
    for ordinal, action in enumerate(values):
        if not isinstance(action, dict):
            fail("acción diaria inválida")
        require_keys(
            action,
            ("ordinal", "creation_percent", "branch", "industry", "foundation_type", "foundation_succeeded"),
            "action",
        )
        if require_int(action["ordinal"], "action.ordinal") != ordinal:
            fail("actions debe conservar el ordinal nativo")
        percent = require_int(action["creation_percent"], "action.creation_percent")
        if not 3 <= percent <= 9:
            fail("action.creation_percent debe estar entre 3 y 9")
        branch = action["branch"]
        if branch not in {"foundation", "production"}:
            fail("action.branch debe ser foundation o production")
        if action["industry"] is not None:
            require_int(action["industry"], "action.industry")
        if branch == "production":
            if action["foundation_type"] is not None or action["foundation_succeeded"] is not None:
                fail("una acción production no puede declarar resultado de fundación")
            continue
        if action["industry"] is not None:
            fail("una acción foundation no selecciona industria existente")
        foundation_type = action["foundation_type"]
        succeeded = action["foundation_succeeded"]
        if foundation_type is None or succeeded is None:
            if foundation_type is not None or succeeded is not None:
                fail("resultado de foundation incompleto")
            continue
        require_int(foundation_type, "action.foundation_type")
        if not isinstance(succeeded, bool):
            fail("action.foundation_succeeded debe ser bool")


def validate_sample(row: dict[str, Any], *, is_initial: bool) -> int:
    kind = "initial" if is_initial else "day"
    require_keys(
        row,
        (
            "kind",
            "tick",
            "calendar",
            "economy",
            "random_state",
            "industry_daily_change_counter",
            "industry_daily_increment",
            "wanted_inds",
            "change_loop",
            "builddata",
            "industries",
            "actions",
        ),
        kind,
    )
    require_int(row["tick"], f"{kind}.tick")
    for clock_name in ("calendar", "economy"):
        clock = row[clock_name]
        if not isinstance(clock, dict):
            fail(f"{kind}.{clock_name} debe ser objeto")
        require_keys(clock, ("date", "year", "month"), f"{kind}.{clock_name}")
        require_int(clock["date"], f"{kind}.{clock_name}.date", minimum=-2_147_483_648)
        require_int(clock["year"], f"{kind}.{clock_name}.year", minimum=-2_147_483_648)
        month = require_int(clock["month"], f"{kind}.{clock_name}.month")
        if month > 11:
            fail(f"{kind}.{clock_name}.month debe estar entre 0 y 11")
    random_state = row["random_state"]
    if not isinstance(random_state, dict):
        fail(f"{kind}.random_state debe ser objeto")
    require_keys(random_state, ("state_0", "state_1"), f"{kind}.random_state")
    require_int(random_state["state_0"], f"{kind}.random_state.state_0")
    require_int(random_state["state_1"], f"{kind}.random_state.state_1")
    for field in ("industry_daily_change_counter", "industry_daily_increment", "wanted_inds"):
        require_int(row[field], f"{kind}.{field}")
    change_loop = require_int(row["change_loop"], f"{kind}.change_loop")
    if row["industry_daily_change_counter"] >= 1 << 16:
        fail(f"{kind}.industry_daily_change_counter debe conservar sólo la fracción 16.16")
    validate_builddata(row["builddata"])
    validate_industries(row["industries"])
    validate_actions(row["actions"], 0 if is_initial else change_loop)
    return change_loop


def validate_trace(path: Path, expected_days: int, expected_producer: str | None = None) -> None:
    """Valida una traza ya identificada por su productor esperado."""
    if expected_days <= 0:
        fail("días esperados debe ser positivo")

    try:
        rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"no se puede leer JSONL {path}: {exc}")
    if not rows:
        fail("traza vacía")

    metadata, *samples = rows
    if metadata.get("kind") != "metadata":
        fail("primera fila debe ser metadata")
    if metadata.get("schema_version") != 1 or metadata.get("trace") != "industry_scheduler":
        fail("metadata del scheduler de industrias inesperada")
    producer = metadata.get("producer")
    if producer not in {"openttd", "openttdrs"}:
        fail("metadata producer debe ser openttd u openttdrs")
    if expected_producer is not None and producer != expected_producer:
        fail(f"metadata producer debe ser {expected_producer}")
    if metadata.get("initial_sample_point") != "after_load_game":
        fail("punto inicial del scheduler inesperado")
    if metadata.get("day_sample_point") != "after_industry_daily_timer":
        fail("punto diario del scheduler inesperado")
    if require_int(metadata.get("max_days"), "metadata.max_days") != expected_days:
        fail("metadata.max_days no coincide con días esperados")
    if require_int(metadata.get("industry_type_count"), "metadata.industry_type_count") != 240:
        fail("metadata.industry_type_count debe ser 240")

    if not samples or samples[0].get("kind") != "initial":
        fail("la primera muestra debe ser initial")
    initial = samples[0]
    validate_sample(initial, is_initial=True)
    days = samples[1:]
    if len(days) != expected_days or any(day.get("kind") != "day" for day in days):
        fail(f"se esperaban {expected_days} muestras day")

    previous_tick = require_int(initial["tick"], "initial.tick")
    previous_economy_date = require_int(initial["economy"]["date"], "initial.economy.date", minimum=-2_147_483_648)
    for day in days:
        validate_sample(day, is_initial=False)
        tick = require_int(day["tick"], "day.tick")
        if tick <= previous_tick:
            fail("los ticks diarios deben aumentar")
        previous_tick = tick
        economy_date = require_int(day["economy"]["date"], "day.economy.date", minimum=-2_147_483_648)
        if economy_date != previous_economy_date + 1:
            fail("la fecha económica debe avanzar una jornada por muestra")
        previous_economy_date = economy_date

    print(f"OK: {path} · {expected_days} días · {producer}")


def main() -> None:
    if len(sys.argv) not in (3, 4):
        fail("uso: validate_industry_trace.py <traza.jsonl> <días esperados> [openttd|openttdrs]")
    path = Path(sys.argv[1])
    try:
        expected_days = int(sys.argv[2])
    except ValueError:
        fail(f"días inválidos: {sys.argv[2]}")
    expected_producer = sys.argv[3] if len(sys.argv) == 4 else None
    validate_trace(path, expected_days, expected_producer)


if __name__ == "__main__":
    main()
