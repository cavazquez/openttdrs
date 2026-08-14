#!/usr/bin/env python3
"""Regresiones del comparador raster focalizado de mundo."""

from __future__ import annotations

import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
import zlib

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "compare_world_screenshots.py"
sys.path.insert(0, str(ROOT / "scripts"))

from compare_world_screenshots import best_candidate_translation, image_metrics, raster_hotspots
from window_visual_regression import PNG_SIGNATURE, PngImage, read_png, write_png


def image(width: int, height: int, shift_x: int = 0, shift_y: int = 0) -> PngImage:
    """Patrón no uniforme, útil para verificar el registro de traslación."""
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            source_x = (x - shift_x) % width
            source_y = (y - shift_y) % height
            pixels.extend(((source_x * 31) % 256, (source_y * 47) % 256, ((source_x + source_y) * 13) % 256, 255))
    return PngImage(width, height, bytes(pixels))


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args], cwd=ROOT, text=True, capture_output=True
    )


def write_indexed_png(path: Path) -> None:
    """PNG 8bpp con PLTE/tRNS, como la captura del blitter OpenTTD."""
    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", 2, 1, 8, 3, 0, 0, 0)
    palette = bytes((1, 2, 3, 40, 50, 60))
    raw = b"\0\0\1"  # filtro none, índices 0 y 1
    path.write_bytes(
        PNG_SIGNATURE
        + chunk(b"IHDR", ihdr)
        + chunk(b"PLTE", palette)
        + chunk(b"tRNS", bytes((255, 127)))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def main() -> int:
    reference = image(40, 32)
    identical_metrics, identical_diff = image_metrics(reference, reference)
    if identical_metrics["changed_pixels"] != 0 or any(identical_diff.rgba[index] for index in range(0, len(identical_diff.rgba), 4)):
        print("FAIL: imágenes idénticas no deben producir diff", file=sys.stderr)
        return 1

    candidate = image(40, 32, shift_x=2, shift_y=-1)
    raw_metrics, _ = image_metrics(reference, candidate)
    dx, dy = best_candidate_translation(reference, candidate, radius=4, stride=1)
    aligned_metrics, _ = image_metrics(reference, candidate, dx, dy)
    if (dx, dy) != (-2, 1) or aligned_metrics["changed_pixels"] >= raw_metrics["changed_pixels"]:
        print(
            f"FAIL: registro inesperado raw={raw_metrics['changed_pixels']} aligned={aligned_metrics['changed_pixels']} offset={(dx, dy)}",
            file=sys.stderr,
        )
        return 1

    altered = bytearray(reference.rgba)
    for y in range(2, 5):
        for x in range(3, 7):
            altered[(y * reference.width + x) * 4] ^= 0xFF
    hotspot_report = raster_hotspots(reference, PngImage(40, 32, bytes(altered)), 0, 0, 8, 3)
    first_hotspot = hotspot_report["reported_cells"][0]
    if (
        hotspot_report["cells_with_difference"] != 1
        or first_hotspot["bounds"] != [0, 0, 8, 8]
        or first_hotspot["changed_pixels"] != 12
        or first_hotspot["total_pixels"] != 64
    ):
        print(f"FAIL: hotspots raster inesperados: {hotspot_report}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        reference_path = root / "reference.png"
        candidate_path = root / "candidate.png"
        diff_path = root / "diff.png"
        report_path = root / "report.json"
        save_path = root / "fixture.sav"
        write_png(reference_path, reference)
        write_png(candidate_path, candidate)
        indexed_path = root / "reference-indexed.png"
        write_indexed_png(indexed_path)
        indexed = read_png(indexed_path)
        if indexed.rgba != bytes((1, 2, 3, 255, 40, 50, 60, 127)):
            print(f"FAIL: PNG indexado se decodificó distinto: {indexed.rgba!r}", file=sys.stderr)
            return 1
        save_path.write_bytes(b"fixture")
        completed = run(
            str(reference_path),
            str(candidate_path),
            "--diff",
            str(diff_path),
            "--report",
            str(report_path),
            "--save",
            str(save_path),
            "--center",
            "189,126",
            "--resolution",
            "40x32",
            "--candidate-graphics",
            "OpenGFX · 8bpp",
            "--alignment-radius",
            "4",
            "--alignment-stride",
            "1",
        )
        if completed.returncode != 0 or not diff_path.is_file() or not report_path.is_file():
            print(completed.stdout, completed.stderr, file=sys.stderr)
            print("FAIL: el comparador no generó sus artefactos", file=sys.stderr)
            return 1
        report = json.loads(report_path.read_text(encoding="utf-8"))
        if report["alignment"]["candidate_translation"] != [-2, 1]:
            print(f"FAIL: reporte sin traslación reproducible: {report}", file=sys.stderr)
            return 1
        if report["capture"]["profile"] != "clean-static":
            print(f"FAIL: perfil de captura inesperado: {report}", file=sys.stderr)
            return 1
        if report["hotspots"]["cell_size_px"] != 64 or not report["hotspots"]["reported_cells"]:
            print(f"FAIL: reporte sin hotspots raster: {report}", file=sys.stderr)
            return 1

        bad_resolution = run(
            str(reference_path),
            str(candidate_path),
            "--diff",
            str(diff_path),
            "--report",
            str(report_path),
            "--center",
            "189,126",
            "--resolution",
            "41x32",
            "--candidate-graphics",
            "OpenGFX · 8bpp",
        )
        if bad_resolution.returncode != 2 or "resolución real" not in bad_resolution.stderr:
            print(bad_resolution.stdout, bad_resolution.stderr, file=sys.stderr)
            print("FAIL: resolución distinta debe rechazarse", file=sys.stderr)
            return 1

    print("OK: el diff raster focal detecta desplazamiento y geometría inválida")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
