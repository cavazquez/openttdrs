#!/usr/bin/env python3
"""Compara ``GenerateTrees`` de OpenTTD y openttdrs desde la misma frontera.

El oráculo guarda un `.sav` inmediatamente antes y después de
``GenerateTrees`` dentro de OpenTTD. El candidato retoma el estado RNG de
``DATE`` del primero, reproduce sólo árboles y compara los diez bytes de cada
tesela con el segundo. También compara cada llamada admitida por sustrato a
``PlaceTree``
para señalar la primera regla que desfasó el RNG, sin requerir una captura
raster del mapa completo.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import random_map_parity as matrix


ROOT = Path(__file__).resolve().parents[1]
TRACE_METADATA_FIELDS = (
    "schema_version",
    "contract",
    "trace",
    "stage",
    "climate",
    "random_state",
    "width",
    "height",
)
TRACE_PLACEMENT_FIELDS = ("ordinal", "origin", "x", "y", "random", "parent")
CLIMATES = {
    "temperate": 0,
    "arctic": 1,
    "tropic": 2,
    "toyland": 3,
}


class TreePhaseError(RuntimeError):
    """El fixture de árboles no se pudo generar o no cumple el contrato."""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_tree_trace(path: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Carga el contrato JSONL de colocaciones de ``GenerateTrees``."""
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise TreePhaseError(f"no se pudo leer la traza {path}: {error}") from error
    if not lines:
        raise TreePhaseError(f"{path}: traza vacía")
    try:
        metadata = json.loads(lines[0])
    except json.JSONDecodeError as error:
        raise TreePhaseError(f"{path}:1: metadata JSON inválida") from error
    if (
        not isinstance(metadata, dict)
        or metadata.get("kind") != "metadata"
        or metadata.get("schema_version") != 1
        or metadata.get("contract") != "tree-generation-trace"
        or metadata.get("trace") != "tree_placements"
        or metadata.get("stage") != "GenerateTrees"
    ):
        raise TreePhaseError(f"{path}: metadata no declara tree-generation-trace v1")

    placements: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines[1:], start=2):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise TreePhaseError(f"{path}:{line_number}: JSON inválido") from error
        if not isinstance(value, dict) or value.get("kind") != "tree_placement":
            raise TreePhaseError(f"{path}:{line_number}: se esperaba kind=tree_placement")
        expected_ordinal = len(placements)
        if value.get("ordinal") != expected_ordinal:
            raise TreePhaseError(
                f"{path}:{line_number}: ordinal {value.get('ordinal')!r}; "
                f"se esperaba {expected_ordinal}"
            )
        if value.get("origin") not in {"group", "random", "same_height", "rainforest"}:
            raise TreePhaseError(f"{path}:{line_number}: origen de árbol inválido")
        if not all(isinstance(value.get(field), int) for field in ("x", "y", "random")):
            raise TreePhaseError(f"{path}:{line_number}: faltan coordenadas/RNG enteros")
        parent = value.get("parent")
        if parent is not None and (
            not isinstance(parent, dict)
            or not isinstance(parent.get("x"), int)
            or not isinstance(parent.get("y"), int)
        ):
            raise TreePhaseError(f"{path}:{line_number}: parent inválido")
        placements.append({field: value.get(field) for field in TRACE_PLACEMENT_FIELDS})
    return metadata, placements


def compare_tree_traces(
    reference: tuple[dict[str, Any], list[dict[str, Any]]],
    candidate: tuple[dict[str, Any], list[dict[str, Any]]],
) -> dict[str, Any]:
    """Compara metadata estructural y filas de colocación, en orden exacto."""
    reference_metadata, reference_rows = reference
    candidate_metadata, candidate_rows = candidate
    metadata_differences = [
        {
            "field": field,
            "reference": reference_metadata.get(field),
            "candidate": candidate_metadata.get(field),
        }
        for field in TRACE_METADATA_FIELDS
        if reference_metadata.get(field) != candidate_metadata.get(field)
    ]
    first_difference: dict[str, Any] | None = None
    difference_count = 0
    row_count = max(len(reference_rows), len(candidate_rows))
    for ordinal in range(row_count):
        ref = reference_rows[ordinal] if ordinal < len(reference_rows) else None
        cand = candidate_rows[ordinal] if ordinal < len(candidate_rows) else None
        if ref == cand:
            continue
        difference_count += 1
        if first_difference is None:
            changed_fields = (
                [field for field in TRACE_PLACEMENT_FIELDS if ref and cand and ref.get(field) != cand.get(field)]
                if ref is not None and cand is not None
                else ["row_presence"]
            )
            first_difference = {
                "ordinal": ordinal,
                "fields": changed_fields,
                "reference": ref,
                "candidate": cand,
            }
    return {
        "metadata_differences": metadata_differences,
        "reference_placement_count": len(reference_rows),
        "candidate_placement_count": len(candidate_rows),
        "placement_difference_count": difference_count,
        "first_difference": first_difference,
        "exact_match": not metadata_differences and difference_count == 0,
    }


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.stat().st_size == 0:
        raise TreePhaseError(f"no se generó {label}: {path}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-bin", type=Path, default=ROOT / "reference/openttd-upstream/build/openttd")
    parser.add_argument("--candidate-bin", type=Path)
    parser.add_argument(
        "--reference-commit",
        default=os.environ.get("OPENTTDRS_REFERENCE_COMMIT"),
        help="commit del checkout OpenTTD (default: docs/parity/openttd-reference.json)",
    )
    parser.add_argument("--size", type=int, default=64, help="lado del mapa (default: 64)")
    parser.add_argument("--seed", type=int, default=1_330_935_378, help="seed de OpenTTD")
    parser.add_argument(
        "--climate",
        choices=tuple(CLIMATES),
        default="temperate",
        help="landscape de OpenTTD (default: temperate)",
    )
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--out-dir", type=Path, help="directorio para artefactos del fixture")
    parser.add_argument("--report", type=Path, help="ruta del informe JSON")
    return parser.parse_args()


def validate_args(args: argparse.Namespace) -> tuple[Path, Path, str]:
    if args.size < 64 or args.size > 4096 or args.size & (args.size - 1):
        raise TreePhaseError("--size debe ser una potencia de dos entre 64 y 4096")
    if args.timeout <= 0:
        raise TreePhaseError("--timeout debe ser positivo")
    reference = args.reference_bin.resolve()
    if not reference.is_file():
        raise TreePhaseError(f"no existe el binario OpenTTD {reference}")
    candidate = matrix.ensure_candidate_binary(args.candidate_bin.resolve() if args.candidate_bin else None)
    return reference, candidate, args.reference_commit or matrix.load_manifest_commit()


def run_reference_generation(
    reference: Path,
    config: Path,
    seed: int,
    commit: str,
    out_dir: Path,
    timeout: int,
) -> tuple[Path, Path, Path]:
    pre_save = out_dir / "trees.pre.sav"
    post_save = out_dir / "trees.post.sav"
    trace = out_dir / "trees.reference.trace.jsonl"
    generated_raw = out_dir / "generation.after_startup.raw.jsonl"
    log = out_dir / "generation.reference.log"
    env = os.environ.copy()
    for key in (
        "OPENTTDRS_WORLD_RAW_OUT",
        "OPENTTDRS_WORLD_RAW_MIN_CALL",
        "OPENTTDRS_SNAPSHOT_OUT",
        "OPENTTDRS_WORLD_SEMANTIC_OUT",
    ):
        env.pop(key, None)
    env.update(
        {
            "OPENTTDRS_TREE_PRE_SAVE_OUT": str(pre_save),
            "OPENTTDRS_TREE_POST_SAVE_OUT": str(post_save),
            "OPENTTDRS_TREE_TRACE_OUT": str(trace),
            # El hook de mapa nuevo hace terminar al dedicated una vez
            # completado el arranque; no es el oráculo de árboles final.
            "OPENTTDRS_RANDOM_MAP_RAW_OUT": str(generated_raw),
            "OPENTTDRS_RANDOM_MAP_SOURCE": f"tree-phase:{seed}",
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
    for path, label in ((pre_save, "el .sav preárboles"), (post_save, "el .sav postárboles"), (trace, "la traza OpenTTD")):
        require_file(path, label)
    return pre_save, post_save, trace


def run_reference_post_raw(
    reference: Path,
    post_save: Path,
    commit: str,
    out_dir: Path,
    timeout: int,
) -> Path:
    raw = out_dir / "trees.reference.post.raw.jsonl"
    log = out_dir / "post.reference.log"
    env = os.environ.copy()
    for key in ("OPENTTDRS_RANDOM_MAP_RAW_OUT", "OPENTTDRS_TREE_PRE_SAVE_OUT", "OPENTTDRS_TREE_POST_SAVE_OUT", "OPENTTDRS_TREE_TRACE_OUT"):
        env.pop(key, None)
    env.update(
        {
            "OPENTTDRS_WORLD_RAW_OUT": str(raw),
            # Dedicated + -g carga primero una partida nueva y luego el save.
            "OPENTTDRS_WORLD_RAW_MIN_CALL": "2",
            "OPENTTDRS_WORLD_RAW_SOURCE": str(post_save),
            "OPENTTDRS_WORLD_RAW_SAVE_SHA256": sha256(post_save),
            "OPENTTDRS_OPENTTD_COMMIT": commit,
        }
    )
    matrix.run_checked(
        [str(reference), "-X", "-I", "opengfx", "-D", "-g", str(post_save)],
        env,
        timeout,
        log,
    )
    require_file(raw, "el world-raw postárboles de OpenTTD")
    return raw


def run_candidate_replay(
    candidate: Path,
    pre_save: Path,
    out_dir: Path,
    timeout: int,
) -> tuple[Path, Path]:
    raw = out_dir / "trees.candidate.post.raw.jsonl"
    trace = out_dir / "trees.candidate.trace.jsonl"
    log = out_dir / "candidate.replay.log"
    matrix.run_checked(
        [
            str(candidate),
            "--replay-trees",
            str(pre_save),
            str(raw),
            "--tree-trace",
            str(trace),
        ],
        {**os.environ, "CARGO_NET_OFFLINE": "true"},
        timeout,
        log,
    )
    require_file(raw, "el world-raw postárboles de openttdrs")
    require_file(trace, "la traza de openttdrs")
    return raw, trace


def main() -> int:
    args = parse_args()
    try:
        reference, candidate, commit = validate_args(args)
    except (TreePhaseError, matrix.MatrixError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    holder = tempfile.TemporaryDirectory(prefix="openttdrs-tree-phase-") if args.out_dir is None else None
    out_dir = Path(holder.name) if holder else args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.report.resolve() if args.report else out_dir / "report.json"
    report: dict[str, Any] = {
        "schema_version": 1,
        "contract": "tree-phase-parity",
        "reference": {"binary": str(reference), "commit": commit},
        "candidate": {"binary": str(candidate)},
        "size": args.size,
        "seed": args.seed,
        "climate": args.climate,
    }
    try:
        with tempfile.TemporaryDirectory(prefix="openttdrs-tree-phase-config-") as config_dir:
            config = Path(config_dir) / "openttd.cfg"
            matrix.write_config(config, args.size, climate=CLIMATES[args.climate])
            pre_save, post_save, reference_trace = run_reference_generation(
                reference, config, args.seed, commit, out_dir, args.timeout
            )
            reference_raw = run_reference_post_raw(
                reference, post_save, commit, out_dir, args.timeout
            )
            candidate_raw, candidate_trace = run_candidate_replay(
                candidate, pre_save, out_dir, args.timeout
            )
        reference_metadata, reference_tiles = matrix.read_world_raw(reference_raw)
        candidate_metadata, candidate_tiles = matrix.read_world_raw(candidate_raw)
        if (reference_metadata["width"], reference_metadata["height"]) != (args.size, args.size):
            raise TreePhaseError("OpenTTD postárboles no conserva el tamaño solicitado")
        map_comparison = matrix.compare_tiles(reference_tiles, candidate_tiles, args.size, args.size)
        trace_comparison = compare_tree_traces(
            read_tree_trace(reference_trace), read_tree_trace(candidate_trace)
        )
        report.update(
            {
                "artifacts": {
                    "pre_save": str(pre_save),
                    "post_save": str(post_save),
                    "reference_raw": str(reference_raw),
                    "candidate_raw": str(candidate_raw),
                    "reference_trace": str(reference_trace),
                    "candidate_trace": str(candidate_trace),
                },
                "map": map_comparison,
                "trace": trace_comparison,
                "exact_match": map_comparison["exact_match"] and trace_comparison["exact_match"],
            }
        )
        print(
            f"trees {args.size}x{args.size} climate={args.climate} seed={args.seed}: "
            f"{'OK' if report['exact_match'] else 'DIVERGE'} "
            f"tiles={map_comparison['tile_difference_count']} "
            f"blocks4={map_comparison['changed_block_count']}/{map_comparison['block_grid']['count']} "
            f"placements={trace_comparison['candidate_placement_count']}"
        )
    except (TreePhaseError, matrix.MatrixError, OSError) as error:
        report["error"] = str(error)
        report["exact_match"] = False
        print(f"ERROR: {error}", file=sys.stderr)
    finally:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"Reporte escrito en {report_path}")
        if holder is not None:
            holder.cleanup()
    return 0 if report.get("exact_match") else 1


if __name__ == "__main__":
    raise SystemExit(main())
