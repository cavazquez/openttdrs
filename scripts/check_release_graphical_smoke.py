#!/usr/bin/env python3
"""Valida las capturas del smoke gráfico de un paquete Linux (#577).

El lector PNG compartido es stdlib-only. Este control no compara píxeles con un
golden: comprueba que el menú y el escenario empaquetados hayan producido
superficies de la resolución solicitada, con contenido visible y distinto.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys

from window_visual_regression import GateError, PngImage, read_png


class SmokeCaptureError(RuntimeError):
    """Una captura no demuestra un arranque gráfico utilizable."""


@dataclass(frozen=True)
class ImageStats:
    opaque_pixels: int
    varied_pixels: int
    distinct_colours: int


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--menu", type=Path, required=True, help="PNG del menú principal")
    result.add_argument("--scenario", type=Path, required=True, help="PNG tras Primera ruta")
    result.add_argument("--width", type=int, default=1280)
    result.add_argument("--height", type=int, default=720)
    return result


def image_stats(image: PngImage) -> ImageStats:
    first_rgb = image.rgba[:3]
    opaque_pixels = 0
    varied_pixels = 0
    colours: set[bytes] = set()
    for offset in range(0, len(image.rgba), 4):
        pixel = image.rgba[offset : offset + 4]
        if pixel[3]:
            opaque_pixels += 1
        if pixel[:3] != first_rgb:
            varied_pixels += 1
        # Alcanzar el umbral es suficiente; no se reserva memoria proporcional
        # a un PNG de pantalla completa sólo para contar toda su paleta.
        if len(colours) < 16:
            colours.add(pixel)
    return ImageStats(opaque_pixels, varied_pixels, len(colours))


def require_visible_frame(label: str, image: PngImage, width: int, height: int) -> ImageStats:
    if (image.width, image.height) != (width, height):
        raise SmokeCaptureError(
            f"{label}: dimensiones {image.width}x{image.height}; se esperaba {width}x{height}"
        )
    stats = image_stats(image)
    pixels = width * height
    minimum = max(1_024, pixels // 1_000)
    if stats.opaque_pixels < minimum:
        raise SmokeCaptureError(
            f"{label}: pantalla vacía ({stats.opaque_pixels} píxeles opacos; mínimo {minimum})"
        )
    if stats.varied_pixels < minimum or stats.distinct_colours < 8:
        raise SmokeCaptureError(
            f"{label}: pantalla plana o sin composición visible "
            f"({stats.varied_pixels} píxeles variados, {stats.distinct_colours} colores)"
        )
    return stats


def changed_pixels(left: PngImage, right: PngImage) -> int:
    return sum(
        left.rgba[offset : offset + 4] != right.rgba[offset : offset + 4]
        for offset in range(0, len(left.rgba), 4)
    )


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    if args.width <= 0 or args.height <= 0:
        parser().error("--width y --height deben ser positivos")

    try:
        menu = read_png(args.menu)
        scenario = read_png(args.scenario)
        menu_stats = require_visible_frame("menú", menu, args.width, args.height)
        scenario_stats = require_visible_frame("escenario", scenario, args.width, args.height)
        changed = changed_pixels(menu, scenario)
        minimum_changed = max(1_024, args.width * args.height // 100)
        if changed < minimum_changed:
            raise SmokeCaptureError(
                "menú y escenario son demasiado parecidos "
                f"({changed} píxeles distintos; mínimo {minimum_changed})"
            )
    except (GateError, OSError, SmokeCaptureError) as error:
        print(f"FAIL: smoke gráfico de paquete: {error}", file=sys.stderr)
        return 1

    print(
        "OK: smoke gráfico de paquete "
        f"({args.width}x{args.height}; menú={menu_stats.varied_pixels} variados, "
        f"escenario={scenario_stats.varied_pixels} variados, distintos={changed})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
