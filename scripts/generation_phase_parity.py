#!/usr/bin/env python3
"""Localiza la primera divergencia del generador procedural mismo-seed.

OpenTTD escribe `world-raw` directamente tras las fronteras
``GenerateClearTile``, pueblos, industrias, objetos y árboles. El candidato
ejecuta exactamente hasta cada una de esas fases; luego se comparan los diez
bytes de todas las teselas, RNG, pools de entidades y los intentos industriales;
las diferencias de teselas se agrupan en bloques 4×4. No es un
oráculo raster: una divergencia en esta herramienta identifica la fase que la
introdujo.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import random_map_parity as matrix


ROOT = Path(__file__).resolve().parents[1]
PHASES = ("landscape", "clear", "towns", "industries", "objects", "trees")
CLIMATE_CODES = {
    "temperate": 0,
    "arctic": 1,
    "tropic": 2,
    "toyland": 3,
}

GENERATION_STATE_FIELDS = (
    "random_state_0",
    "random_state_1",
    "town_count",
    "town_positions",
    "industry_count",
    "industry_positions",
    "industry_attempt_count",
    "industry_attempts",
    "object_count",
    "object_positions",
)

GENERATION_INDUSTRY_ATTEMPT_FIELDS = (
    ("ordinal", 0xFFFFFFFF),
    ("type", 0xFFFF),
    ("x", 0xFFFFFFFF),
    ("y", 0xFFFFFFFF),
    ("random_var8f", 0xFFFFFFFF),
    ("initial_random_bits", 0xFFFF),
    ("layout_index", 0xFFFFFFFF),
)

GENERATION_ENTITY_POOLS = (
    (
        "town",
        "town_count",
        "town_positions",
        (
            ("id", 0xFFFFFFFF),
            ("x", 0xFFFFFFFF),
            ("y", 0xFFFFFFFF),
            ("population", 0xFFFFFFFF),
            ("num_houses", 0xFFFF),
        ),
    ),
    (
        "industry",
        "industry_count",
        "industry_positions",
        (
            ("id", 0xFFFFFFFF),
            ("type", 0xFFFF),
            ("x", 0xFFFFFFFF),
            ("y", 0xFFFFFFFF),
            ("selected_layout", 0xFF),
            ("random", 0xFFFF),
            ("random_colour", 0xFF),
            ("counter", 0xFFFF),
            ("prod_level", 0xFF),
            ("town_id", 0xFFFFFFFF),
        ),
    ),
    (
        "object",
        "object_count",
        "object_positions",
        (
            ("id", 0xFFFFFFFF),
            ("type", 0xFFFF),
            ("x", 0xFFFFFFFF),
            ("y", 0xFFFFFFFF),
            ("width", 0xFFFF),
            ("height", 0xFFFF),
            ("view", 0xFF),
        ),
    ),
)


class GenerationPhaseError(RuntimeError):
    """El fixture por etapas no pudo completarse."""


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.stat().st_size == 0:
        raise GenerationPhaseError(f"no se generó {label}: {path}")


def parse_phases(value: str) -> tuple[str, ...]:
    phases = tuple(part.strip() for part in value.split(",") if part.strip())
    if not phases:
        raise GenerationPhaseError("--phases debe incluir al menos una fase")
    invalid = [phase for phase in phases if phase not in PHASES]
    if invalid:
        raise GenerationPhaseError(
            f"--phases contiene {', '.join(invalid)}; usar {', '.join(PHASES)}"
        )
    if len(set(phases)) != len(phases):
        raise GenerationPhaseError("--phases no puede repetir fases")
    expected = tuple(phase for phase in PHASES if phase in phases)
    if phases != expected:
        raise GenerationPhaseError("--phases debe respetar el orden del pipeline")
    return phases


def first_divergent_stage(comparisons: dict[str, dict[str, Any]]) -> str | None:
    for phase in PHASES:
        comparison = comparisons.get(phase)
        if comparison is not None and not comparison["exact_match"]:
            return phase
    return None


def _validate_generation_pool(
    metadata: dict[str, Any],
    source: str,
    label: str,
    count_field: str,
    positions_field: str,
    fields: tuple[tuple[str, int], ...],
) -> None:
    """Exige una secuencia de pool completa, válida, única y ordenada por ID."""
    count = metadata[count_field]
    positions = metadata[positions_field]
    if type(count) is not int or not 0 <= count <= 0xFFFFFFFF:
        raise GenerationPhaseError(f"{source}: {count_field} inválido")
    if not isinstance(positions, list) or len(positions) != count:
        raise GenerationPhaseError(f"{source}: {count_field} no corresponde a {positions_field}")
    previous_id = -1
    for index, entity in enumerate(positions):
        if not isinstance(entity, dict) or any(
            type(entity.get(field)) is not int or not 0 <= entity[field] <= maximum
            for field, maximum in fields
        ):
            raise GenerationPhaseError(f"{source}: entidad {label} inválida en índice {index}")
        entity_id = entity["id"]
        if entity_id <= previous_id:
            detail = "repetido" if entity_id == previous_id else "fuera de orden"
            raise GenerationPhaseError(
                f"{source}: ID {label} {detail} en índice {index}: {entity_id} tras {previous_id}"
            )
        previous_id = entity_id


def _validate_industry_attempts(metadata: dict[str, Any], source: str) -> None:
    """Exige la traza completa de intentos, incluido orden y resultado booleano."""
    count = metadata["industry_attempt_count"]
    attempts = metadata["industry_attempts"]
    if type(count) is not int or not 0 <= count <= 0xFFFFFFFF:
        raise GenerationPhaseError(f"{source}: industry_attempt_count inválido")
    if not isinstance(attempts, list) or len(attempts) != count:
        raise GenerationPhaseError(
            f"{source}: industry_attempt_count no corresponde a industry_attempts"
        )
    for index, attempt in enumerate(attempts):
        if not isinstance(attempt, dict) or any(
            type(attempt.get(field)) is not int or not 0 <= attempt[field] <= maximum
            for field, maximum in GENERATION_INDUSTRY_ATTEMPT_FIELDS
        ):
            raise GenerationPhaseError(
                f"{source}: intento industrial inválido en índice {index}"
            )
        if attempt["ordinal"] != index:
            raise GenerationPhaseError(
                f"{source}: ordinal industrial inválido en índice {index}: {attempt['ordinal']}"
            )
        if type(attempt.get("succeeded")) is not bool:
            raise GenerationPhaseError(
                f"{source}: resultado industrial inválido en índice {index}"
            )


def _first_pool_difference(
    reference: dict[str, Any], candidate: dict[str, Any], count_field: str, positions_field: str
) -> dict[str, Any] | None:
    for index in range(max(reference[count_field], candidate[count_field])):
        ref_entity = reference[positions_field][index] if index < reference[count_field] else None
        cand_entity = candidate[positions_field][index] if index < candidate[count_field] else None
        if ref_entity != cand_entity:
            return {"index": index, "reference": ref_entity, "candidate": cand_entity}
    return None


def compare_generation_state(reference: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    """No confundir tiles iguales con estado generador igual o no observado."""
    for source, metadata in (("reference", reference), ("candidate", candidate)):
        missing = [field for field in GENERATION_STATE_FIELDS if field not in metadata]
        if missing:
            raise GenerationPhaseError(
                f"{source}: falta estado de generación {missing}; reconstruir el exportador instrumentado"
            )
        for field in ("random_state_0", "random_state_1"):
            value = metadata[field]
            if type(value) is not int or not 0 <= value <= 0xFFFFFFFF:
                raise GenerationPhaseError(f"{source}: {field} inválido")
        for pool in GENERATION_ENTITY_POOLS:
            _validate_generation_pool(metadata, source, *pool)
        _validate_industry_attempts(metadata, source)
    differing = [
        field for field in GENERATION_STATE_FIELDS if reference[field] != candidate[field]
    ]
    first_differences = {
        label: _first_pool_difference(reference, candidate, count_field, positions_field)
        for label, count_field, positions_field, _ in GENERATION_ENTITY_POOLS
    }
    return {
        "exact_match": not differing,
        "compared_fields": list(GENERATION_STATE_FIELDS),
        "differing_fields": differing,
        "first_town_difference": first_differences["town"],
        "first_industry_difference": first_differences["industry"],
        "first_industry_attempt_difference": _first_pool_difference(
            reference,
            candidate,
            "industry_attempt_count",
            "industry_attempts",
        ),
        "first_object_difference": first_differences["object"],
    }


def include_generation_state(tile_comparison: dict[str, Any], reference: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    state = compare_generation_state(reference, candidate)
    return {
        **tile_comparison,
        "tiles_exact_match": tile_comparison["exact_match"],
        "generation_state": state,
        "exact_match": tile_comparison["exact_match"] and state["exact_match"],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-bin", type=Path, default=ROOT / "reference/openttd-upstream/build/openttd")
    parser.add_argument("--candidate-bin", type=Path)
    parser.add_argument(
        "--reference-commit",
        default=os.environ.get("OPENTTDRS_REFERENCE_COMMIT"),
        help="commit OpenTTD para metadata (default: manifiesto local)",
    )
    parser.add_argument("--size", type=int, default=64, help="lado del mapa (default: 64)")
    parser.add_argument("--seed", type=int, default=1_330_935_378, help="seed de OpenTTD")
    parser.add_argument(
        "--climate",
        choices=tuple(CLIMATE_CODES),
        default="temperate",
        help="clima de OpenTTD (default: temperate)",
    )
    parser.add_argument(
        "--amount-of-rivers",
        type=int,
        choices=range(4),
        help="cantidad de ríos de game_creation (0..3; default: OpenTTD)",
    )
    parser.add_argument(
        "--min-river-length",
        type=int,
        help="longitud mínima experta de ríos (2..255; default: OpenTTD)",
    )
    parser.add_argument(
        "--river-route-random",
        type=int,
        help="aleatoriedad experta de la ruta de ríos (1..255; default: OpenTTD)",
    )
    parser.add_argument(
        "--water-borders",
        type=int,
        help="máscara water_borders (0..16; default: OpenTTD)",
    )
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--phases", default=",".join(PHASES))
    parser.add_argument("--out-dir", type=Path, help="directorio para artefactos")
    parser.add_argument("--report", type=Path, help="ruta del informe JSON")
    parser.add_argument(
        "--require-exact",
        action="store_true",
        help="falla si alguna frontera comparada diverge",
    )
    return parser.parse_args()


def validate_args(args: argparse.Namespace) -> tuple[Path, Path, str, tuple[str, ...]]:
    if args.size < 64 or args.size > 4096 or args.size & (args.size - 1):
        raise GenerationPhaseError("--size debe ser una potencia de dos entre 64 y 4096")
    if args.timeout <= 0:
        raise GenerationPhaseError("--timeout debe ser positivo")
    for name, value, minimum, maximum in (
        ("--min-river-length", args.min_river_length, 2, 255),
        ("--river-route-random", args.river_route_random, 1, 255),
        ("--water-borders", args.water_borders, 0, 16),
    ):
        if value is not None and not minimum <= value <= maximum:
            raise GenerationPhaseError(f"{name} debe estar entre {minimum} y {maximum}")
    reference = args.reference_bin.resolve()
    if not reference.is_file():
        raise GenerationPhaseError(f"no existe el binario OpenTTD {reference}")
    candidate = matrix.ensure_candidate_binary(args.candidate_bin.resolve() if args.candidate_bin else None)
    return reference, candidate, args.reference_commit or matrix.load_manifest_commit(), parse_phases(args.phases)


def run_reference_generation(
    reference: Path,
    config: Path,
    seed: int,
    commit: str,
    stage_dir: Path,
    timeout: int,
    log: Path,
) -> dict[str, Path]:
    env = os.environ.copy()
    for key in (
        "OPENTTDRS_WORLD_RAW_OUT",
        "OPENTTDRS_WORLD_RAW_MIN_CALL",
        "OPENTTDRS_RANDOM_MAP_RAW_OUT",
        "OPENTTDRS_RANDOM_MAP_SAVE_OUT",
        "OPENTTDRS_TREE_PRE_SAVE_OUT",
        "OPENTTDRS_TREE_POST_SAVE_OUT",
        "OPENTTDRS_TREE_TRACE_OUT",
        "OPENTTDRS_AMOUNT_OF_RIVERS",
        "OPENTTDRS_MIN_RIVER_LENGTH",
        "OPENTTDRS_RIVER_ROUTE_RANDOM",
        "OPENTTDRS_WATER_BORDERS",
    ):
        env.pop(key, None)
    random_map_raw = stage_dir / "generation.after_startup.raw.jsonl"
    env.update(
        {
            "OPENTTDRS_GENERATION_STAGE_DIR": str(stage_dir),
            # El hook de mapa nuevo exporta en el primer tick y termina el
            # dedicated. Los snapshots ya se guardaron durante genworld.
            "OPENTTDRS_RANDOM_MAP_RAW_OUT": str(random_map_raw),
            "OPENTTDRS_RANDOM_MAP_SOURCE": f"generation-phase:{seed}",
            "OPENTTDRS_OPENTTD_COMMIT": commit,
        }
    )
    matrix.run_checked(
        [
            str(reference),
            "-X",
            "-c",
            str(config),
            "-I",
            "opengfx",
            "-v",
            "null",
            "-s",
            "null",
            "-m",
            "null",
            "-b",
            "null",
            "-D",
            "-G",
            str(seed),
            "-g",
        ],
        env,
        timeout,
        log,
    )
    raw = {phase: stage_dir / f"{phase}.reference.raw.jsonl" for phase in PHASES}
    for phase, path in raw.items():
        require_file(path, f"el world-raw de la fase {phase}")
    require_file(random_map_raw, "el world-raw de fin de arranque")
    return raw


def run_candidate_raw(
    candidate: Path,
    size: int,
    seed: int,
    climate: str,
    phase: str,
    commit: str,
    out: Path,
    timeout: int,
    log: Path,
    *,
    amount_of_rivers: int | None,
    min_river_length: int | None,
    river_route_random: int | None,
    water_borders: int | None,
) -> None:
    env = {**os.environ, "CARGO_NET_OFFLINE": "true"}
    for key in (
        "OPENTTDRS_AMOUNT_OF_RIVERS",
        "OPENTTDRS_MIN_RIVER_LENGTH",
        "OPENTTDRS_RIVER_ROUTE_RANDOM",
        "OPENTTDRS_WATER_BORDERS",
    ):
        env.pop(key, None)
    for key, value in (
        ("OPENTTDRS_AMOUNT_OF_RIVERS", amount_of_rivers),
        ("OPENTTDRS_MIN_RIVER_LENGTH", min_river_length),
        ("OPENTTDRS_RIVER_ROUTE_RANDOM", river_route_random),
        ("OPENTTDRS_WATER_BORDERS", water_borders),
    ):
        if value is not None:
            env[key] = str(value)
    matrix.run_checked(
        [
            str(candidate),
            "--generate",
            f"{size}x{size}",
            "--seed",
            str(seed),
            "--climate",
            climate,
            "--generate-until",
            phase,
            str(out),
            "--stage",
            "sav_map",
            "--openttd-commit",
            commit,
        ],
        env,
        timeout,
        log,
    )
    require_file(out, f"el world-raw candidato para {phase}")


def main() -> int:
    args = parse_args()
    try:
        reference, candidate, commit, phases = validate_args(args)
    except (GenerationPhaseError, matrix.MatrixError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    holder = tempfile.TemporaryDirectory(prefix="openttdrs-generation-phase-") if args.out_dir is None else None
    out_dir = Path(holder.name) if holder else args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.report.resolve() if args.report else out_dir / "report.json"
    report: dict[str, Any] = {
        "schema_version": 6,
        "contract": "generation-phase-parity",
        "reference": {"binary": str(reference), "commit": commit},
        "candidate": matrix.candidate_provenance(candidate, managed=args.candidate_bin is None),
        "size": args.size,
        "seed": args.seed,
        "climate": args.climate,
        "phases": list(phases),
        "block_size": 4,
        "generation_settings": {
            "amount_of_rivers": args.amount_of_rivers,
            "min_river_length": args.min_river_length,
            "river_route_random": args.river_route_random,
            "water_borders": args.water_borders,
        },
    }
    try:
        with tempfile.TemporaryDirectory(prefix="openttdrs-generation-phase-config-") as config_dir:
            config = Path(config_dir) / "openttd.cfg"
            matrix.write_config(
                config,
                args.size,
                climate=CLIMATE_CODES[args.climate],
                amount_of_rivers=args.amount_of_rivers,
                min_river_length=args.min_river_length,
                river_route_random=args.river_route_random,
                water_borders=args.water_borders,
            )
            reference_raw_by_phase = run_reference_generation(
                reference,
                config,
                args.seed,
                commit,
                out_dir,
                args.timeout,
                out_dir / "generation.reference.log",
            )
        comparisons: dict[str, dict[str, Any]] = {}
        for phase in phases:
            reference_raw = reference_raw_by_phase[phase]
            candidate_raw = out_dir / f"{phase}.candidate.raw.jsonl"
            run_candidate_raw(
                candidate,
                args.size,
                args.seed,
                args.climate,
                phase,
                commit,
                candidate_raw,
                args.timeout,
                out_dir / f"{phase}.candidate.log",
                amount_of_rivers=args.amount_of_rivers,
                min_river_length=args.min_river_length,
                river_route_random=args.river_route_random,
                water_borders=args.water_borders,
            )
            reference_metadata, reference_tiles = matrix.read_world_raw(reference_raw)
            candidate_metadata, candidate_tiles = matrix.read_world_raw(candidate_raw)
            if (reference_metadata["width"], reference_metadata["height"]) != (args.size, args.size):
                raise GenerationPhaseError(f"{phase}: OpenTTD no conserva {args.size}x{args.size}")
            comparison = matrix.compare_tiles(reference_tiles, candidate_tiles, args.size, args.size)
            comparison = include_generation_state(comparison, reference_metadata, candidate_metadata)
            comparisons[phase] = {
                "reference_raw": str(reference_raw),
                "reference_metadata": reference_metadata,
                "candidate_metadata": candidate_metadata,
                "reference_map_stats": matrix.summarize_map(reference_tiles),
                "candidate_map_stats": matrix.summarize_map(candidate_tiles),
                **comparison,
            }
            print(
                f"phase {phase}: {'OK' if comparison['exact_match'] else 'DIVERGE'} "
                f"tiles={comparison['tile_difference_count']} "
                f"blocks4={comparison['changed_block_count']}/{comparison['block_grid']['count']} "
                f"state={','.join(comparison['generation_state']['differing_fields']) or 'OK'}"
            )
        first = first_divergent_stage(comparisons)
        report.update(
            {
                "comparisons": comparisons,
                "first_divergent_stage": first,
                "exact_match": first is None,
            }
        )
        print(f"primera divergencia: {first or 'ninguna'}")
    except (GenerationPhaseError, matrix.MatrixError, OSError, subprocess.CalledProcessError) as error:
        report["error"] = str(error)
        print(f"ERROR: {error}", file=sys.stderr)
    finally:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"Reporte escrito en {report_path}")
        if holder:
            holder.cleanup()

    if "error" in report:
        return 2
    return 1 if args.require_exact and not report["exact_match"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
