#!/usr/bin/env python3
"""Compara dos capturas raster focalizadas de OpenTTD y openttdrs.

El comparador no decide por sí solo que dos renderers sean equivalentes: las
animaciones y las familias aún no instrumentadas pueden diferir. Produce un
``diff.png`` y un ``report.json`` reproducibles, con métricas antes y después
de buscar un pequeño corrimiento de la candidata. Ese corrimiento es evidencia
de un problema de cámara/viewport, no se oculta en el reporte.

Uso:
  python3 scripts/compare_world_screenshots.py referencia.png candidata.png \\
      --diff diff.png --report report.json --center 189,126 --resolution 1280x720
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from window_visual_regression import GateError, PngImage, read_png, write_png


ROOT = Path(__file__).resolve().parents[1]
MAX_ALIGNMENT_RADIUS = 64


def relative(path: Path) -> str:
    """Devuelve una ruta estable dentro del repo o una absoluta fuera de él."""
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path.resolve())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def image_metrics(reference: PngImage, candidate: PngImage, dx: int = 0, dy: int = 0) -> tuple[dict[str, Any], PngImage]:
    """Compara con la candidata trasladada ``dx,dy`` sobre la referencia.

    ``dx > 0`` desplaza la candidata hacia la derecha; ``dy > 0``, hacia
    abajo. Las zonas que queden fuera de la candidata cuentan como diferencia
    máxima y se ven magenta en el diff, para no esconder pérdida de cobertura.
    """
    width, height = reference.width, reference.height
    diff = bytearray(width * height * 4)
    changed = outside = total_delta = max_delta = 0
    overlap = 0
    for y in range(height):
        source_y = y - dy
        for x in range(width):
            target = (y * width + x) * 4
            source_x = x - dx
            if not (0 <= source_x < candidate.width and 0 <= source_y < candidate.height):
                changed += 1
                outside += 1
                total_delta += 255 * 4
                max_delta = 255
                diff[target : target + 4] = bytes((255, 0, 255, 255))
                continue

            overlap += 1
            source = (source_y * candidate.width + source_x) * 4
            deltas = [
                abs(reference.rgba[target + channel] - candidate.rgba[source + channel])
                for channel in range(4)
            ]
            pixel_delta = max(deltas)
            total_delta += sum(deltas)
            max_delta = max(max_delta, pixel_delta)
            if pixel_delta:
                changed += 1
                # Amplificar cada canal para que diferencias pequeñas sean
                # localizables sin perder cuál fue el canal afectado.
                diff[target : target + 4] = bytes(
                    min(255, deltas[channel] * 4) for channel in range(3)
                ) + b"\xff"
            else:
                diff[target : target + 4] = b"\0\0\0\xff"

    total_pixels = width * height
    return (
        {
            "reference_size": [width, height],
            "candidate_size": [candidate.width, candidate.height],
            "candidate_translation": [dx, dy],
            "total_pixels": total_pixels,
            "overlap_pixels": overlap,
            "outside_candidate_pixels": outside,
            "changed_pixels": changed,
            "changed_ratio": changed / total_pixels,
            "changed_ratio_in_overlap": (changed - outside) / overlap if overlap else None,
            "max_channel_delta": max_delta,
            "mean_channel_delta": total_delta / (total_pixels * 4),
        },
        PngImage(width, height, bytes(diff)),
    )


def sampled_error(reference: PngImage, candidate: PngImage, dx: int, dy: int, stride: int) -> int:
    """Error RGB económico para registrar la cámara antes del diff completo."""
    error = 0
    for y in range(0, reference.height, stride):
        source_y = y - dy
        for x in range(0, reference.width, stride):
            source_x = x - dx
            if not (0 <= source_x < candidate.width and 0 <= source_y < candidate.height):
                error += 255 * 3
                continue
            target = (y * reference.width + x) * 4
            source = (source_y * candidate.width + source_x) * 4
            error += sum(
                abs(reference.rgba[target + channel] - candidate.rgba[source + channel])
                for channel in range(3)
            )
    return error


def best_candidate_translation(reference: PngImage, candidate: PngImage, radius: int, stride: int) -> tuple[int, int]:
    """Busca el desplazamiento entero pequeño con menor error de muestreo."""
    if radius == 0:
        return (0, 0)
    best: tuple[int, int, int] | None = None
    for dy in range(-radius, radius + 1):
        for dx in range(-radius, radius + 1):
            candidate_key = (
                sampled_error(reference, candidate, dx, dy, stride),
                abs(dx) + abs(dy),
                dy * (2 * radius + 1) + dx,
            )
            if best is None or candidate_key < best:
                best = candidate_key
                best_dx, best_dy = dx, dy
    return best_dx, best_dy


def parse_center(raw: str) -> list[int]:
    try:
        x, y = (int(value.strip()) for value in raw.split(",", 1))
    except (TypeError, ValueError) as exc:
        raise GateError("--center debe tener el formato x,y") from exc
    if x < 0 or y < 0:
        raise GateError("--center no admite coordenadas negativas")
    return [x, y]


def parse_resolution(raw: str) -> list[int]:
    try:
        width, height = (int(value) for value in raw.lower().split("x", 1))
    except (TypeError, ValueError) as exc:
        raise GateError("--resolution debe tener el formato anchoxalto") from exc
    if width <= 0 or height <= 0:
        raise GateError("--resolution debe ser positiva")
    return [width, height]


def artifact(path: Path) -> dict[str, str]:
    return {"path": relative(path), "sha256": sha256(path)}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="PNG obtenido de OpenTTD")
    parser.add_argument("candidate", type=Path, help="PNG obtenido de openttdrs")
    parser.add_argument("--diff", type=Path, required=True, help="PNG de diferencias a generar")
    parser.add_argument("--report", type=Path, required=True, help="JSON de métricas a generar")
    parser.add_argument("--save", type=Path, help="partida usada para ambas capturas")
    parser.add_argument("--center", required=True, help="tesela central x,y")
    parser.add_argument("--resolution", required=True, help="resolución solicitada, anchoxalto")
    parser.add_argument("--openttdrs-scale", type=float, default=1.0, help="escala ortográfica candidata")
    parser.add_argument("--reference-graphics", default="OpenGFX 8.0 · 8bpp")
    parser.add_argument("--candidate-graphics", required=True)
    parser.add_argument(
        "--capture-profile",
        default="clean-static",
        choices=("clean-static", "dynamic"),
        help="capas incluidas: clean-static omite UI, rótulos, animación y vehículos",
    )
    parser.add_argument("--alignment-radius", type=int, default=8, help="máximo corrimiento a investigar (px)")
    parser.add_argument("--alignment-stride", type=int, default=8, help="muestreo para buscar corrimiento (px)")
    args = parser.parse_args(argv)

    try:
        if not (0 <= args.alignment_radius <= MAX_ALIGNMENT_RADIUS):
            raise GateError(f"--alignment-radius debe estar entre 0 y {MAX_ALIGNMENT_RADIUS}")
        if args.alignment_stride <= 0:
            raise GateError("--alignment-stride debe ser positivo")
        if not args.openttdrs_scale > 0:
            raise GateError("--openttdrs-scale debe ser positivo")
        center = parse_center(args.center)
        requested_resolution = parse_resolution(args.resolution)
        reference, candidate = read_png(args.reference), read_png(args.candidate)
        if (reference.width, reference.height) != (candidate.width, candidate.height):
            raise GateError(
                "las capturas no tienen la misma geometría: "
                f"OpenTTD={reference.width}x{reference.height}, "
                f"openttdrs={candidate.width}x{candidate.height}"
            )
        if [reference.width, reference.height] != requested_resolution:
            raise GateError(
                "la resolución real no coincide con la solicitada: "
                f"se pidió {requested_resolution[0]}x{requested_resolution[1]}, "
                f"se obtuvo {reference.width}x{reference.height}"
            )

        raw_metrics, _ = image_metrics(reference, candidate)
        dx, dy = best_candidate_translation(
            reference, candidate, args.alignment_radius, args.alignment_stride
        )
        aligned_metrics, diff = image_metrics(reference, candidate, dx, dy)
        args.diff.parent.mkdir(parents=True, exist_ok=True)
        write_png(args.diff, diff)

        report: dict[str, Any] = {
            "schema_version": 1,
            "kind": "focused-world-screenshot",
            "status": "identical" if aligned_metrics["changed_pixels"] == 0 else "different",
            "capture": {
                "center": center,
                "requested_resolution": requested_resolution,
                "openttd_zoom": "normal",
                "openttdrs_orthographic_scale": args.openttdrs_scale,
                "reference_graphics": args.reference_graphics,
                "candidate_graphics": args.candidate_graphics,
                "profile": args.capture_profile,
            },
            "artifacts": {
                "reference": artifact(args.reference),
                "candidate": artifact(args.candidate),
                "diff": artifact(args.diff),
            },
            "metrics": {"raw": raw_metrics, "aligned": aligned_metrics},
            "alignment": {
                "search_radius_px": args.alignment_radius,
                "sample_stride_px": args.alignment_stride,
                "candidate_translation": [dx, dy],
                "meaning": "traslación aplicada a la candidata: +x derecha, +y abajo",
            },
        }
        if args.save is not None:
            if not args.save.is_file():
                raise GateError(f"no existe la partida: {args.save}")
            report["save"] = artifact(args.save)
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (GateError, OSError, ValueError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2

    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
