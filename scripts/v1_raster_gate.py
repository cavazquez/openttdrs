#!/usr/bin/env python3
"""Certifica el contrato raster V1-RAS de Kale contra OpenTTD.

Este gate no busca una traslación ni permite máscaras o recortes.  Consume las
tres capturas que deja ``capture_v1_raster.sh`` y conserva un JSON autocontenido
con los pins, assets, hashes y métricas de la corrida.  Las otras cinco escalas
se guardan sólo como diagnóstico: su presencia es obligatoria, sus métricas no
participan del resultado verde de Normal.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

from compare_world_screenshots import image_metrics, pixel_tolerance_metrics
from window_visual_regression import GateError, read_png, write_png


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "save" / "Kale_TitleGame.sav"
FIXTURE_SHA256 = "584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69"
OPENTTD_OFFICIAL_COMMIT = "14ec60f248547d4d062a1160f0fc26d742319888"
OPENTTD_ORACLE_COMMIT = "c2661164bcb6cbf5ab97b56ccbee7506a3b26833"
CENTER = [132, 2]
RESOLUTION = [800, 600]
NORMAL_SCALE = 1.0
NORMAL_ZOOM = "Normal"
PROFILE = "clean-static"
SETTLE_FRAMES = 360
SAMPLE_COUNT = 3
DIAGNOSTIC_SCALES = {
    0.25: "In4x",
    0.5: "In2x",
    2.0: "Out2x",
    4.0: "Out4x",
    8.0: "Out8x",
}
BUDGET = {
    "max_changed_pixels": 480,
    "max_pixels_over_channel_delta_64": 24,
    "max_mean_absolute_rgba_channel_delta": 0.05,
    "max_outside_candidate_pixels": 0,
}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_SHA = re.compile(r"[0-9a-f]{40}\Z")


class RasterGateError(GateError):
    """El artefacto no cumple el contrato V1-RAS, aunque no sea un budget miss."""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact(path: Path) -> dict[str, Any]:
    try:
        display_path = str(path.resolve().relative_to(ROOT))
    except ValueError:
        display_path = str(path.resolve())
    return {"path": display_path, "sha256": sha256(path), "size_bytes": path.stat().st_size}


def require_file(path: Path, label: str) -> Path:
    if not path.is_file():
        raise RasterGateError(f"falta {label}: {path}")
    return path.resolve()


def require_sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        raise RasterGateError(f"{label} debe ser SHA-256 hexadecimal")
    return value


def resolve_report_artifact(value: object, report_path: Path, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise RasterGateError(f"{report_path}: artifacts.{label} debe ser un objeto")
    raw_path = value.get("path")
    expected_sha = require_sha256(value.get("sha256"), f"{report_path}: artifacts.{label}.sha256")
    if not isinstance(raw_path, str) or not raw_path:
        raise RasterGateError(f"{report_path}: artifacts.{label}.path debe ser una ruta")
    path = Path(raw_path)
    if not path.is_absolute():
        path = ROOT / path
    require_file(path, f"artefacto diagnóstico {label}")
    actual_sha = sha256(path)
    if actual_sha != expected_sha:
        raise RasterGateError(
            f"{report_path}: SHA de artifacts.{label} no coincide "
            f"({actual_sha} != {expected_sha})"
        )
    return artifact(path)


def load_diagnostic(report_path: Path, expected_save_sha: str) -> tuple[float, dict[str, Any]]:
    require_file(report_path, "reporte diagnóstico")
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RasterGateError(f"reporte diagnóstico inválido {report_path}: {exc}") from exc
    if not isinstance(report, dict) or report.get("kind") != "focused-world-screenshot":
        raise RasterGateError(f"{report_path}: no es un reporte focused-world-screenshot")
    capture = report.get("capture")
    if not isinstance(capture, dict):
        raise RasterGateError(f"{report_path}: falta capture")
    scale = capture.get("openttdrs_orthographic_scale")
    if not isinstance(scale, (int, float)) or float(scale) not in DIAGNOSTIC_SCALES:
        raise RasterGateError(f"{report_path}: escala diagnóstica inválida {scale!r}")
    scale = float(scale)
    if capture.get("center") != CENTER:
        raise RasterGateError(f"{report_path}: centro distinto de {CENTER}")
    if capture.get("requested_resolution") != RESOLUTION:
        raise RasterGateError(f"{report_path}: resolución distinta de {RESOLUTION}")
    if capture.get("profile") != PROFILE:
        raise RasterGateError(f"{report_path}: perfil distinto de {PROFILE}")
    if capture.get("openttd_zoom") != DIAGNOSTIC_SCALES[scale]:
        raise RasterGateError(f"{report_path}: zoom nativo no coincide con escala {scale}")
    alignment = report.get("alignment")
    if not isinstance(alignment, dict) or alignment.get("search_radius_px") != 0:
        raise RasterGateError(f"{report_path}: el diagnóstico no puede realinear la candidata")
    if alignment.get("candidate_translation") != [0, 0]:
        raise RasterGateError(f"{report_path}: el diagnóstico debe registrar dx=dy=0")
    save = report.get("save")
    if not isinstance(save, dict) or save.get("sha256") != expected_save_sha:
        raise RasterGateError(f"{report_path}: la partida diagnóstica no coincide con la fixture V1")
    artifacts = report.get("artifacts")
    if not isinstance(artifacts, dict):
        raise RasterGateError(f"{report_path}: faltan artefactos diagnósticos")
    metrics = report.get("metrics")
    if not isinstance(metrics, dict) or not isinstance(metrics.get("raw"), dict):
        raise RasterGateError(f"{report_path}: faltan métricas raw del diagnóstico")
    return scale, {
        "scale": scale,
        "openttd_zoom": DIAGNOSTIC_SCALES[scale],
        "report": artifact(report_path),
        "status": report.get("status"),
        "metrics_raw": metrics["raw"],
        "artifacts": {
            label: resolve_report_artifact(artifacts.get(label), report_path, label)
            for label in ("reference", "candidate", "diff")
        },
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", type=Path, required=True, help="directorio creado por capture_v1_raster.sh")
    parser.add_argument("--save", type=Path, default=FIXTURE, help="fixture Kale fijada por el contrato")
    parser.add_argument("--candidate-sha", required=True, help="commit completo de openttdrs capturado")
    parser.add_argument(
        "--candidate-mode-file",
        type=Path,
        default=ROOT / "assets" / "opengfx" / ".graphics_mode",
        help="archivo que fija OpenGFX 8bpp de la candidata",
    )
    parser.add_argument(
        "--reference-asset",
        type=Path,
        action="append",
        required=True,
        help="asset OpenGFX usado por OpenTTD; repetir para cada input",
    )
    parser.add_argument(
        "--candidate-asset",
        type=Path,
        action="append",
        required=True,
        help="asset OpenGFX usado por openttdrs; repetir para cada input",
    )
    parser.add_argument(
        "--diagnostic-report",
        type=Path,
        action="append",
        required=True,
        help="report.json de una de las cinco escalas diagnósticas",
    )
    parser.add_argument("--report", type=Path, help="salida JSON; por defecto v1-raster-report.json")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        if GIT_SHA.fullmatch(args.candidate_sha) is None:
            raise RasterGateError("--candidate-sha debe ser un commit Git completo de 40 hexadecimales")
        artifact_dir = args.artifact_dir.resolve()
        if not artifact_dir.is_dir():
            raise RasterGateError(f"no existe --artifact-dir: {artifact_dir}")
        report_path = args.report.resolve() if args.report else artifact_dir / "v1-raster-report.json"
        save = require_file(args.save, "la fixture Kale")
        save_sha = sha256(save)
        if save_sha != FIXTURE_SHA256:
            raise RasterGateError(
                f"fixture Kale distinta: {save_sha}; se esperaba {FIXTURE_SHA256}"
            )
        mode_file = require_file(args.candidate_mode_file, "el modo gráfico candidato")
        if mode_file.read_text(encoding="utf-8").strip() != "8bpp":
            raise RasterGateError("la candidata debe capturarse con OpenGFX 8bpp")
        reference_assets = [artifact(require_file(path, "un asset de referencia")) for path in args.reference_asset]
        candidate_assets = [artifact(require_file(path, "un asset de candidata")) for path in args.candidate_asset]

        samples: dict[str, list[dict[str, Any]]] = {"reference": [], "candidate": []}
        images: dict[str, list[Any]] = {"reference": [], "candidate": []}
        for engine in ("reference", "candidate"):
            for index in range(1, SAMPLE_COUNT + 1):
                path = require_file(
                    artifact_dir / "normal" / f"{engine}-{index}.png",
                    f"captura {engine} #{index}",
                )
                image = read_png(path)
                if [image.width, image.height] != RESOLUTION:
                    raise RasterGateError(
                        f"{path}: geometría {image.width}x{image.height}; "
                        f"se esperaba {RESOLUTION[0]}x{RESOLUTION[1]}"
                    )
                samples[engine].append(artifact(path))
                images[engine].append(image)
            hashes = {entry["sha256"] for entry in samples[engine]}
            if len(hashes) != 1:
                raise RasterGateError(f"las tres capturas de {engine} no son hash-idénticas")

        reference = images["reference"][0]
        candidate = images["candidate"][0]
        metrics, diff = image_metrics(reference, candidate, 0, 0)
        tolerance = pixel_tolerance_metrics(reference, candidate, 0, 0, [64])["64"]
        diff_path = artifact_dir / "normal" / "diff.png"
        write_png(diff_path, diff)
        checks = {
            "changed_pixels": {
                "actual": metrics["changed_pixels"],
                "maximum": BUDGET["max_changed_pixels"],
                "passed": metrics["changed_pixels"] <= BUDGET["max_changed_pixels"],
            },
            "pixels_over_channel_delta_64": {
                "actual": tolerance["changed_pixels"],
                "maximum": BUDGET["max_pixels_over_channel_delta_64"],
                "passed": tolerance["changed_pixels"] <= BUDGET["max_pixels_over_channel_delta_64"],
            },
            "mean_absolute_rgba_channel_delta": {
                "actual": metrics["mean_channel_delta"],
                "maximum": BUDGET["max_mean_absolute_rgba_channel_delta"],
                "passed": metrics["mean_channel_delta"]
                <= BUDGET["max_mean_absolute_rgba_channel_delta"],
            },
            "outside_candidate_pixels": {
                "actual": metrics["outside_candidate_pixels"],
                "maximum": BUDGET["max_outside_candidate_pixels"],
                "passed": metrics["outside_candidate_pixels"]
                <= BUDGET["max_outside_candidate_pixels"],
            },
        }

        diagnostics: dict[float, dict[str, Any]] = {}
        for diagnostic_path in args.diagnostic_report:
            scale, diagnostic = load_diagnostic(diagnostic_path.resolve(), save_sha)
            if scale in diagnostics:
                raise RasterGateError(f"se repite el diagnóstico de escala {scale}")
            diagnostics[scale] = diagnostic
        if set(diagnostics) != set(DIAGNOSTIC_SCALES):
            expected = ", ".join(str(scale) for scale in sorted(DIAGNOSTIC_SCALES))
            found = ", ".join(str(scale) for scale in sorted(diagnostics))
            raise RasterGateError(f"faltan o sobran diagnósticos; esperados {expected}, recibidos {found}")

        passed = all(bool(check["passed"]) for check in checks.values())
        report: dict[str, Any] = {
            "schema_version": 1,
            "kind": "v1-raster-gate",
            "status": "passed" if passed else "failed",
            "contract": {
                "issue": 589,
                "fixture": artifact(save),
                "fixture_expected_sha256": FIXTURE_SHA256,
                "center": CENTER,
                "resolution": RESOLUTION,
                "openttdrs_orthographic_scale": NORMAL_SCALE,
                "openttd_zoom": NORMAL_ZOOM,
                "profile": PROFILE,
                "candidate_settle_frames": SETTLE_FRAMES,
                "sample_count_per_engine": SAMPLE_COUNT,
                "alignment": {
                    "candidate_translation": [0, 0],
                    "search_radius_px": 0,
                    "masks": False,
                    "adaptive_crop": False,
                },
                "budget": BUDGET,
            },
            "pins": {
                "candidate_commit": args.candidate_sha,
                "openttd_official_commit": OPENTTD_OFFICIAL_COMMIT,
                "openttd_oracle_commit": OPENTTD_ORACLE_COMMIT,
            },
            "assets": {
                "reference": reference_assets,
                "candidate": candidate_assets,
                "candidate_graphics_mode": artifact(mode_file),
            },
            "normal": {
                "samples": samples,
                "artifacts": {"diff": artifact(diff_path)},
                "metrics": metrics,
                "pixel_tolerance_64": tolerance,
                "checks": checks,
            },
            "diagnostics": [diagnostics[scale] for scale in sorted(diagnostics)],
        }
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except (GateError, OSError, ValueError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2

    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
