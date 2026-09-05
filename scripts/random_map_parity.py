#!/usr/bin/env python3
"""Genera una matriz de mapas aleatorios y compara OpenTTD contra openttdrs.

La comparación primaria es el contrato ``world-raw`` por tesela.  Además de
contar bytes divergentes, agrupa el resultado en bloques 4×4 para que un mapa
grande no se reduzca a una captura raster difícil de diagnosticar.

Por defecto usa muchas semillas en el tamaño mínimo y reduce la cantidad al
subir de tamaño: ``64:8,128:4,256:2,512:1``.  Los artefactos pesados quedan en
un directorio temporal y sólo se conservan cuando se pasa ``--keep-artifacts``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
RAW_FIELDS = ("height", "type", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8")
DEFAULT_MATRIX = "64:8,128:4,256:2,512:1"
# `genworld.cpp` runs TILE_UPDATE_FREQUENCY (0x500) tile loops after
# GenerateTrees before handing the new game to the caller. The first regular
# StateGameLoop then animates, runs the 0x501 tile loop and ticks trees; keep
# the generator side at that same observation point as the OpenTTD raw hook.
REFERENCE_STARTUP_TILE_LOOPS = 0x500
Tile = tuple[int, ...]


class MatrixError(RuntimeError):
    """Un caso no pudo generarse o no cumple el contrato world-raw."""


def parse_matrix(value: str) -> list[tuple[int, int]]:
    """Parsea ``WIDTH:COUNT,...`` y valida potencias de dos OpenTTD."""
    result: list[tuple[int, int]] = []
    seen: set[int] = set()
    for item in value.split(","):
        try:
            raw_size, raw_count = item.split(":", 1)
            size = int(raw_size)
            count = int(raw_count)
        except ValueError as error:
            raise MatrixError(f"matriz inválida: {item!r}; usar WIDTH:COUNT") from error
        if size < 64 or size > 4096 or size & (size - 1):
            raise MatrixError(f"tamaño inválido {size}; debe ser potencia de dos entre 64 y 4096")
        if count <= 0:
            raise MatrixError(f"cantidad inválida para {size}: {count}")
        if size in seen:
            raise MatrixError(f"tamaño repetido en la matriz: {size}")
        seen.add(size)
        result.append((size, count))
    if not result:
        raise MatrixError("la matriz no puede estar vacía")
    return sorted(result)


def load_manifest_commit() -> str:
    path = ROOT / "docs/parity/openttd-reference.json"
    try:
        return str(json.loads(path.read_text(encoding="utf-8"))["commit"])
    except (OSError, KeyError, json.JSONDecodeError) as error:
        raise MatrixError(f"no se pudo leer el commit OpenTTD de {path}") from error


def write_config(
    path: Path,
    size: int,
    climate: int = 0,
    *,
    amount_of_rivers: int | None = None,
    min_river_length: int | None = None,
    river_route_random: int | None = None,
    water_borders: int | None = None,
) -> None:
    """Escribe la configuración headless de un mapa cuadrado de OpenTTD.

    Los ajustes de ríos son opcionales para que la matriz canónica conserve el
    perfil de partida nueva, pero el comparador por fases pueda auditar los
    valores expertos que suelen quedar fuera de las pruebas vanilla.
    """
    bits = int(math.log2(size))
    if climate not in range(4):
        raise MatrixError(f"clima inválido {climate}; usar 0..3")
    settings: list[str] = []
    for name, value, minimum, maximum in (
        ("amount_of_rivers", amount_of_rivers, 0, 3),
        ("min_river_length", min_river_length, 2, 255),
        ("river_route_random", river_route_random, 1, 255),
        ("water_borders", water_borders, 0, 16),
    ):
        if value is not None:
            if not minimum <= value <= maximum:
                raise MatrixError(f"{name} inválido {value}; usar {minimum}..{maximum}")
            settings.append(f"{name} = {value}\n")
    path.write_text(
        "[game_creation]\n"
        f"map_x = {bits}\n"
        f"map_y = {bits}\n"
        f"landscape = {climate}\n"
        + "".join(settings)
        + "\n"
        + "[misc]\n"
        + "no_multithreading = true\n",
        encoding="utf-8",
    )


def read_world_raw(path: Path) -> tuple[dict[str, Any], list[Tile]]:
    try:
        lines = path.open(encoding="utf-8")
    except OSError as error:
        raise MatrixError(f"no se pudo abrir {path}: {error}") from error

    with lines:
        try:
            metadata = json.loads(next(lines))
        except (StopIteration, json.JSONDecodeError) as error:
            raise MatrixError(f"{path}: falta metadata JSON válida") from error
        if metadata.get("kind") != "metadata" or metadata.get("contract") != "world-raw":
            raise MatrixError(f"{path}: metadata no declara contract=world-raw")
        width = metadata.get("width")
        height = metadata.get("height")
        if not isinstance(width, int) or not isinstance(height, int) or width <= 0 or height <= 0:
            raise MatrixError(f"{path}: dimensiones inválidas en metadata")
        tiles: list[Tile] = []
        for line_number, line in enumerate(lines, start=2):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as error:
                raise MatrixError(f"{path}:{line_number}: JSON inválido") from error
            if row.get("kind") != "tile_raw":
                raise MatrixError(f"{path}:{line_number}: fila no es tile_raw")
            expected_index = len(tiles)
            expected_x = expected_index % width
            expected_y = expected_index // width
            if (
                row.get("index") != expected_index
                or row.get("x") != expected_x
                or row.get("y") != expected_y
            ):
                raise MatrixError(f"{path}:{line_number}: orden/coordenada no es fila-mayor")
            try:
                tiles.append(tuple(int(row[field]) for field in RAW_FIELDS))
            except (KeyError, TypeError, ValueError) as error:
                raise MatrixError(f"{path}:{line_number}: falta un campo raw") from error

    expected = width * height
    if len(tiles) != expected:
        raise MatrixError(f"{path}: metadata declara {expected} teselas, hay {len(tiles)}")
    if metadata.get("emitted_tile_count") != expected:
        raise MatrixError(f"{path}: emitted_tile_count no coincide con el mapa completo")
    return metadata, tiles


def write_ottdmap(path: Path, metadata: dict[str, Any], tiles: Iterable[Tile]) -> None:
    width = int(metadata["width"])
    height = int(metadata["height"])
    rows = list(tiles)
    if len(rows) != width * height:
        raise MatrixError(f"no se puede escribir {path}: cantidad de teselas incorrecta")
    planes = [bytearray(), bytearray(), bytearray(), bytearray(), bytearray(), bytearray()]
    m8 = bytearray()
    for tile in rows:
        height_byte, tile_type, m1, m2, m3, m4, m5, m6, m7, tile_m8 = tile
        planes[0].append(tile_type & 0xFF)
        planes[1].append(height_byte & 0xFF)
        planes[2].append(m1 & 0xFF)
        planes[3].append(m2 & 0xFF)
        planes[4].append((m2 >> 8) & 0xFF)
        planes[5].append(m3 & 0xFF)
        # MAP1 stores m3hi in its own plane between M3 and M5; append it below.
        m8.extend(struct.pack("<H", tile_m8 & 0xFFFF))

    m3hi = bytearray(tile[5] & 0xFF for tile in rows)
    m5 = bytearray(tile[6] & 0xFF for tile in rows)
    m6 = bytearray(tile[7] & 0xFF for tile in rows)
    m7 = bytearray(tile[8] & 0xFF for tile in rows)
    body = bytearray(b"MAP1")
    body.extend(struct.pack("<IIHH", width, height, 1, 1))
    body.extend(planes[0])
    body.extend(planes[1])
    body.extend(planes[2])
    body.extend(planes[3])
    body.extend(planes[4])
    body.extend(planes[5])
    body.extend(m3hi)
    body.extend(m5)
    body.extend(m6)
    body.extend(m7)
    body.extend(m8)
    path.write_bytes(body)


def compare_tiles(
    reference: list[Tile], candidate: list[Tile], width: int, height: int, block_size: int = 4
) -> dict[str, Any]:
    if len(reference) != len(candidate):
        raise MatrixError("los streams tienen cantidades distintas de teselas")
    field_diffs: Counter[str] = Counter()
    family_diffs: Counter[str] = Counter()
    blocks: dict[tuple[int, int], int] = defaultdict(int)
    samples: list[dict[str, Any]] = []
    tile_difference_count = 0
    byte_difference_count = 0
    for index, (ref, cand) in enumerate(zip(reference, candidate, strict=True)):
        changed_fields = [field for field, a, b in zip(RAW_FIELDS, ref, cand, strict=True) if a != b]
        if not changed_fields:
            continue
        tile_difference_count += 1
        byte_difference_count += len(changed_fields)
        x = index % width
        y = index // width
        blocks[(x // block_size, y // block_size)] += 1
        family = f"0x{(ref[1] >> 4) & 0xF:x}/0x{(cand[1] >> 4) & 0xF:x}"
        family_diffs[family] += 1
        for field in changed_fields:
            field_diffs[field] += 1
        if len(samples) < 25:
            samples.append(
                {
                    "x": x,
                    "y": y,
                    "fields": changed_fields,
                    "reference": dict(zip(RAW_FIELDS, ref, strict=True)),
                    "candidate": dict(zip(RAW_FIELDS, cand, strict=True)),
                }
            )
    block_width = (width + block_size - 1) // block_size
    block_height = (height + block_size - 1) // block_size
    total_blocks = block_width * block_height
    return {
        "tile_difference_count": tile_difference_count,
        "tile_difference_ratio": tile_difference_count / len(reference),
        "byte_difference_count": byte_difference_count,
        "changed_block_count": len(blocks),
        "changed_block_ratio": len(blocks) / total_blocks,
        "block_size": block_size,
        "block_grid": {"width": block_width, "height": block_height, "count": total_blocks},
        "field_difference_counts": dict(sorted(field_diffs.items())),
        "tile_family_difference_counts": dict(sorted(family_diffs.items())),
        "changed_blocks_sample": [
            {"block_x": bx, "block_y": by, "changed_tiles": count}
            for (bx, by), count in sorted(blocks.items())[:25]
        ],
        "tile_difference_samples": samples,
        "exact_match": tile_difference_count == 0,
    }


def summarize_map(tiles: list[Tile]) -> dict[str, Any]:
    """Resume la complejidad del mapa sin serializar una imagen gigante."""
    type_counts: Counter[str] = Counter(f"0x{(tile[1] >> 4) & 0xF:x}" for tile in tiles)
    heights = [tile[0] for tile in tiles]
    return {
        "tile_type_counts": dict(sorted(type_counts.items())),
        "height_min": min(heights),
        "height_max": max(heights),
        "water_tile_ratio": type_counts["0x6"] / len(tiles),
    }


def run_checked(command: list[str], env: dict[str, str], timeout: int, log: Path) -> None:
    try:
        with log.open("w", encoding="utf-8") as stream:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env=env,
                stdout=stream,
                stderr=subprocess.STDOUT,
                timeout=timeout,
                check=False,
            )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise MatrixError(f"falló el proceso {' '.join(command)}: {error}") from error
    if completed.returncode != 0:
        # El dedicated puede informar que no pudo abrir sockets en un sandbox,
        # pero el export sigue siendo válido si dejó el contrato completo.
        return


def ensure_candidate_binary(path: Path | None) -> Path:
    if path is not None:
        if not path.is_file():
            raise MatrixError(f"no existe el binario candidato {path}")
        return path
    candidate = ROOT / "target/debug/world_raw_dumper"
    # An existing executable may predate the fix under test. Let Cargo check
    # source/dependency freshness, and never fall back to it after a failed build.
    command = ["cargo", "build", "--locked", "-q", "-p", "openttdrs-core", "--bin", "world_raw_dumper"]
    subprocess.run(command, cwd=ROOT, env={**os.environ, "CARGO_NET_OFFLINE": "true"}, check=True)
    if not candidate.is_file():
        raise MatrixError(f"cargo no produjo {candidate}")
    return candidate


def candidate_provenance(path: Path, *, managed: bool) -> dict[str, Any]:
    """Identify the tested executable; external binaries need not match this checkout."""
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    provenance: dict[str, Any] = {
        "binary": str(path),
        "binary_sha256": digest,
        "build": "cargo-build-locked" if managed else "external",
    }
    if managed:
        provenance["source_commit"] = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        provenance["source_tracked_changes"] = subprocess.check_output(
            ["git", "status", "--porcelain", "--untracked-files=no"], cwd=ROOT, text=True
        ).splitlines()
    return provenance


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-bin", type=Path, default=ROOT / "reference/openttd-upstream/build/openttd")
    parser.add_argument(
        "--reference-commit",
        default=os.environ.get("OPENTTDRS_REFERENCE_COMMIT"),
        help="commit del checkout usado (por defecto, el pin de docs/parity/openttd-reference.json)",
    )
    parser.add_argument("--candidate-bin", type=Path)
    parser.add_argument("--matrix", default=DEFAULT_MATRIX, help=f"tamaños y cantidad; default: {DEFAULT_MATRIX}")
    parser.add_argument("--base-seed", type=int, default=0x4F54_4452)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--out-dir", type=Path, help="directorio de artefactos (default: temporal)")
    parser.add_argument("--report", type=Path, help="ruta del resumen JSON")
    parser.add_argument(
        "--compare-generator",
        action="store_true",
        help="compara también el generador procedural Rust contra el mapa generado por OpenTTD",
    )
    parser.add_argument("--keep-artifacts", action="store_true", help="conserva raw, .ottdmap y logs")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        matrix = parse_matrix(args.matrix)
        reference_bin = args.reference_bin.resolve()
        if not reference_bin.is_file():
            raise MatrixError(f"no existe el binario OpenTTD {reference_bin}")
        candidate_bin = ensure_candidate_binary(args.candidate_bin.resolve() if args.candidate_bin else None)
        candidate_info = candidate_provenance(candidate_bin, managed=args.candidate_bin is None)
        commit = args.reference_commit or load_manifest_commit()
    except (MatrixError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    temporary = args.out_dir is None
    holder = tempfile.TemporaryDirectory(prefix="openttdrs-random-map-") if temporary else None
    out_dir = Path(holder.name) if holder else args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    report_path = args.report.resolve() if args.report else out_dir / "report.json"
    cases: list[dict[str, Any]] = []
    try:
        with tempfile.TemporaryDirectory(prefix="openttdrs-random-config-") as config_dir:
            config_root = Path(config_dir)
            for size, count in matrix:
                config = config_root / f"openttd-{size}.cfg"
                write_config(config, size)
                for ordinal in range(count):
                    seed = args.base_seed + size * 100 + ordinal
                    stem = f"map-{size:04d}-seed-{seed}"
                    reference_raw = out_dir / f"{stem}.reference.jsonl"
                    reference_save = out_dir / f"{stem}.reference.sav"
                    candidate_map = out_dir / f"{stem}.ottdmap"
                    candidate_raw = out_dir / f"{stem}.candidate.jsonl"
                    generator_raw = out_dir / f"{stem}.generator.jsonl"
                    reference_log = out_dir / f"{stem}.reference.log"
                    candidate_log = out_dir / f"{stem}.candidate.log"
                    env = os.environ.copy()
                    for key in (
                        "OPENTTDRS_WORLD_RAW_OUT",
                        "OPENTTDRS_SNAPSHOT_OUT",
                        "OPENTTDRS_WORLD_SEMANTIC_OUT",
                    ):
                        env.pop(key, None)
                    env.update(
                        {
                            "OPENTTDRS_RANDOM_MAP_RAW_OUT": str(reference_raw),
                            "OPENTTDRS_RANDOM_MAP_SAVE_OUT": str(reference_save),
                            "OPENTTDRS_RANDOM_MAP_SOURCE": f"random:{size}x{size}:seed={seed}",
                            "OPENTTDRS_OPENTTD_COMMIT": commit,
                        }
                    )
                    command = [
                        str(reference_bin),
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
                    ]
                    try:
                        run_checked(command, env, args.timeout, reference_log)
                        ref_meta, ref_tiles = read_world_raw(reference_raw)
                        if (ref_meta["width"], ref_meta["height"]) != (size, size):
                            raise MatrixError(
                                f"{stem}: OpenTTD produjo {ref_meta['width']}x{ref_meta['height']}"
                            )
                        write_ottdmap(candidate_map, ref_meta, ref_tiles)
                        candidate_command = [
                            str(candidate_bin),
                            str(reference_save),
                            str(candidate_raw),
                            "--stage",
                            "sav_map",
                            "--openttd-commit",
                            commit,
                        ]
                        run_checked(candidate_command, {**os.environ, "CARGO_NET_OFFLINE": "true"}, args.timeout, candidate_log)
                        cand_meta, cand_tiles = read_world_raw(candidate_raw)
                        comparison = compare_tiles(ref_tiles, cand_tiles, size, size)
                        case: dict[str, Any] = {
                            "size": size,
                            "seed": seed,
                            "reference_metadata": ref_meta,
                            "candidate_metadata": cand_meta,
                            "reference_map_stats": summarize_map(ref_tiles),
                            "loader": comparison,
                            # Mantener estos campos por compatibilidad con los
                            # reportes anteriores: representan la carga SAV.
                            **comparison,
                        }
                        if args.compare_generator:
                            generator_command = [
                                str(candidate_bin),
                                "--generate",
                                f"{size}x{size}",
                                "--seed",
                                str(seed),
                                str(generator_raw),
                                "--stage",
                                "sav_map",
                                "--openttd-commit",
                                commit,
                            ]
                            run_checked(
                                generator_command,
                                {
                                    **os.environ,
                                    "CARGO_NET_OFFLINE": "true",
                                    "OPENTTDRS_GENERATE_STARTUP_TICKS": str(
                                        REFERENCE_STARTUP_TILE_LOOPS
                                    ),
                                },
                                args.timeout,
                                out_dir / f"{stem}.generator.log",
                            )
                            generator_meta, generator_tiles = read_world_raw(generator_raw)
                            generator_comparison = compare_tiles(
                                ref_tiles, generator_tiles, size, size
                            )
                            case["generator_metadata"] = generator_meta
                            case["generator"] = generator_comparison
                        cases.append(case)
                        generator_status = ""
                        if args.compare_generator:
                            generator_status = (
                                f" gen={'OK' if case['generator']['exact_match'] else 'DIVERGE'}"
                                f" gen_tiles={case['generator']['tile_difference_count']}"
                            )
                        print(
                            f"{stem}: {'OK' if comparison['exact_match'] else 'DIVERGE'} "
                            f"tiles={comparison['tile_difference_count']} "
                            f"blocks4={comparison['changed_block_count']}/{comparison['block_grid']['count']}"
                            f"{generator_status}"
                        )
                    except (MatrixError, OSError) as error:
                        cases.append({"size": size, "seed": seed, "error": str(error)})
                        print(f"{stem}: ERROR: {error}", file=sys.stderr)
                    finally:
                        if not args.keep_artifacts:
                            for path in (
                                reference_raw,
                                reference_save,
                                candidate_map,
                                candidate_raw,
                                generator_raw,
                                reference_log,
                                candidate_log,
                                out_dir / f"{stem}.generator.log",
                            ):
                                path.unlink(missing_ok=True)
    finally:
        summary = {
            "schema_version": 1,
            "contract": "random-map-parity-matrix",
            "reference": {"binary": str(reference_bin), "commit": commit},
            "candidate": {
                **candidate_info,
                "loader": "world_raw_dumper MAP1",
            },
            "matrix": [{"size": size, "maps": count} for size, count in matrix],
            "block_size": 4,
            "cases": cases,
            "case_count": len(cases),
            "exact_match_count": sum(1 for case in cases if case.get("exact_match")),
            "divergence_count": sum(1 for case in cases if "error" not in case and not case.get("exact_match")),
            "error_count": sum(1 for case in cases if "error" in case),
            "generator_exact_match_count": sum(
                1
                for case in cases
                if case.get("generator", {}).get("exact_match")
            ),
            "generator_divergence_count": sum(
                1
                for case in cases
                if "generator" in case and not case["generator"].get("exact_match")
            ),
            "generator_comparison_enabled": args.compare_generator,
        }
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"Reporte escrito en {report_path}")
        if holder:
            holder.cleanup()
    return 1 if summary["error_count"] or summary["divergence_count"] or summary["generator_divergence_count"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
