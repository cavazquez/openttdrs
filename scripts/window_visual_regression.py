#!/usr/bin/env python3
"""Gate reproducible de regresión visual para familias de ventanas (#297, #299, #300, #301, #302).

Cada perfil mantiene cuatro archivos versionados: referencia OpenTTD,
candidato openttdrs, diff RGBA y sidecar JSON. El lector PNG es deliberadamente
stdlib-only para que el gate no dependa de Pillow/ImageMagick en CI.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ProcessPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import math
import os
import re
import struct
import sys
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "docs" / "parity" / "screenshots" / "window-regression.json"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
ARTIFACT_NAMES = ("reference.png", "candidate.png", "diff.png", "sidecar.json")
VALID_CATEGORIES = {"geometry", "iconographic", "chromatic"}
DIAGNOSTIC_ROLE = "openttd_similarity_diagnostic"
CERTIFICATION_ROLE = "client_regression"
VALID_COMPARISON_ROLES = {DIAGNOSTIC_ROLE, CERTIFICATION_ROLE}
GIT_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
CERTIFICATION_TOLERANCE = {
    "pixel_delta_threshold": 8,
    "max_pixels_over_threshold_ratio": 0.005,
    "max_mean_abs_channel_delta": 1 / 255,
}


class GateError(RuntimeError):
    """A malformed visual-regression fixture or manifest."""


@dataclass(frozen=True)
class PngImage:
    width: int
    height: int
    rgba: bytes


def _paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else b if pb <= pc else c


def read_png(path: Path) -> PngImage:
    data = path.read_bytes()
    if not data.startswith(PNG_SIGNATURE):
        raise GateError(f"{path}: no es PNG")

    pos = len(PNG_SIGNATURE)
    width = height = bit_depth = color_type = None
    chunks: list[bytes] = []
    palette: bytes | None = None
    transparency: bytes | None = None
    while pos < len(data):
        if pos + 12 > len(data):
            raise GateError(f"{path}: chunk PNG truncado")
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        start, end = pos + 8, pos + 8 + length
        if end + 4 > len(data):
            raise GateError(f"{path}: payload PNG truncado")
        payload = data[start:end]
        if kind == b"IHDR":
            if length != 13:
                raise GateError(f"{path}: IHDR inválido")
            width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            if compression or filtering or interlace:
                raise GateError(f"{path}: PNG comprimido/entrelazado no soportado")
        elif kind == b"IDAT":
            chunks.append(payload)
        elif kind == b"PLTE":
            palette = payload
        elif kind == b"tRNS":
            transparency = payload
        elif kind == b"IEND":
            break
        pos = end + 4

    if width is None or height is None or bit_depth != 8 or color_type not in (2, 3, 6):
        raise GateError(f"{path}: se requiere PNG RGB/RGBA/indexado de 8 bits no entrelazado")
    if width <= 0 or height <= 0:
        raise GateError(f"{path}: dimensiones PNG inválidas")
    if color_type == 3:
        if palette is None or not palette or len(palette) % 3:
            raise GateError(f"{path}: PNG indexado sin paleta RGB válida")
        if len(palette) // 3 > 256:
            raise GateError(f"{path}: paleta PNG con más de 256 entradas")
        if transparency is not None and len(transparency) > len(palette) // 3:
            raise GateError(f"{path}: transparencia PNG excede la paleta")
    channels = 1 if color_type == 3 else 3 if color_type == 2 else 4
    row_len = width * channels
    try:
        raw = zlib.decompress(b"".join(chunks))
    except zlib.error as exc:
        raise GateError(f"{path}: IDAT inválido: {exc}") from exc
    if len(raw) != height * (row_len + 1):
        raise GateError(f"{path}: longitud de píxeles inválida")

    previous = bytearray(row_len)
    out = bytearray(width * height * 4)
    source = 0
    target = 0
    for _ in range(height):
        filter_type = raw[source]
        source += 1
        row = bytearray(raw[source : source + row_len])
        source += row_len
        for i, value in enumerate(row):
            left = row[i - channels] if i >= channels else 0
            above = previous[i]
            upper_left = previous[i - channels] if i >= channels else 0
            if filter_type == 0:
                decoded = value
            elif filter_type == 1:
                decoded = (value + left) & 0xFF
            elif filter_type == 2:
                decoded = (value + above) & 0xFF
            elif filter_type == 3:
                decoded = (value + ((left + above) // 2)) & 0xFF
            elif filter_type == 4:
                decoded = (value + _paeth(left, above, upper_left)) & 0xFF
            else:
                raise GateError(f"{path}: filtro PNG desconocido {filter_type}")
            row[i] = decoded
        previous = row
        for x in range(width):
            pixel = row[x * channels : (x + 1) * channels]
            if color_type == 3:
                palette_index = pixel[0]
                palette_start = palette_index * 3
                if palette_start >= len(palette):
                    raise GateError(f"{path}: índice de paleta fuera de rango")
                out[target : target + 3] = palette[palette_start : palette_start + 3]
                out[target + 3] = (
                    transparency[palette_index]
                    if transparency is not None and palette_index < len(transparency)
                    else 255
                )
            else:
                out[target : target + 3] = pixel[:3]
                out[target + 3] = pixel[3] if channels == 4 else 255
            target += 4
    return PngImage(width, height, bytes(out))


def write_png(path: Path, image: PngImage) -> None:
    raw = bytearray()
    stride = image.width * 4
    for row in range(image.height):
        raw.append(0)  # filter None: deterministic and portable.
        start = row * stride
        raw.extend(image.rgba[start : start + stride])
    ihdr = struct.pack(">IIBBBBB", image.width, image.height, 8, 6, 0, 0, 0)

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(PNG_SIGNATURE + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare(reference: PngImage, candidate: PngImage) -> tuple[dict[str, Any], PngImage | None, str | None]:
    if (reference.width, reference.height) != (candidate.width, candidate.height):
        return (
            {
                "reference_size": [reference.width, reference.height],
                "candidate_size": [candidate.width, candidate.height],
                "changed_pixels": None,
                "changed_ratio": None,
                "max_channel_delta": None,
                "mean_channel_delta": None,
            },
            None,
            "geometry",
        )

    changed = 0
    total_delta = 0
    max_delta = 0
    alpha_changed = False
    diff = bytearray(len(reference.rgba))
    for i in range(0, len(reference.rgba), 4):
        deltas = [abs(reference.rgba[i + channel] - candidate.rgba[i + channel]) for channel in range(4)]
        delta = max(deltas)
        total_delta += sum(deltas)
        max_delta = max(max_delta, delta)
        if delta:
            changed += 1
            alpha_changed = alpha_changed or deltas[3] != 0
            diff[i : i + 4] = bytes((255, min(255, deltas[0] * 4), min(255, deltas[1] * 4), 255))
        else:
            diff[i : i + 4] = bytes((0, 0, 0, 255))
    pixels = reference.width * reference.height
    metrics = {
        "reference_size": [reference.width, reference.height],
        "candidate_size": [candidate.width, candidate.height],
        "changed_pixels": changed,
        "changed_ratio": changed / pixels,
        "max_channel_delta": max_delta,
        "mean_channel_delta": total_delta / (pixels * 4),
    }
    category = "iconographic" if alpha_changed else "chromatic"
    return metrics, PngImage(reference.width, reference.height, bytes(diff)), category


def within_tolerance(metrics: dict[str, Any], tolerance: dict[str, Any]) -> bool:
    if metrics["changed_pixels"] is None:
        return False
    checks = {
        "max_changed_pixels": metrics["changed_pixels"],
        "max_changed_ratio": metrics["changed_ratio"],
        "max_channel_delta": metrics["max_channel_delta"],
        "max_mean_channel_delta": metrics["mean_channel_delta"],
    }
    for key, actual in checks.items():
        limit = tolerance.get(key)
        if not isinstance(limit, (int, float)) or limit < 0:
            raise GateError(f"tolerancia inválida: {key}")
        if actual > limit:
            return False
    return True


def profile_key(profile: dict[str, Any]) -> str:
    return f"{profile['width']}x{profile['height']}-{profile['ui_scale']}x"


def roi_for_profile(profile: dict[str, Any]) -> dict[str, int]:
    """Return a bounded, explicit ROI for a client-regression profile."""
    roi = profile.get("roi")
    if not isinstance(roi, dict):
        raise GateError("perfil de certificación sin roi fijo")
    values = {key: roi.get(key) for key in ("x", "y", "width", "height")}
    if any(not isinstance(value, int) for value in values.values()):
        raise GateError("roi debe declarar x/y/width/height enteros")
    if values["x"] < 0 or values["y"] < 0 or values["width"] <= 0 or values["height"] <= 0:
        raise GateError("roi fuera de rango")
    if values["x"] + values["width"] > profile["width"] or values["y"] + values["height"] > profile["height"]:
        raise GateError("roi excede las dimensiones del perfil")
    return {key: int(value) for key, value in values.items()}


def certification_metrics(reference: PngImage, candidate: PngImage, roi: dict[str, int]) -> dict[str, Any]:
    """Measure the fixed V1 client budget inside a single declared ROI.

    The historical OpenTTD comparison keeps its broad metrics for diagnostic
    continuity.  This function intentionally has a different metric shape so
    a diagnostic sidecar can never be mistaken for a client certification.
    """
    if (reference.width, reference.height) != (candidate.width, candidate.height):
        raise GateError("no se puede medir ROI con geometrías distintas")
    if roi["x"] + roi["width"] > reference.width or roi["y"] + roi["height"] > reference.height:
        raise GateError("roi excede la imagen capturada")

    pixels_over_threshold = 0
    total_delta = 0
    pixel_count = roi["width"] * roi["height"]
    for y in range(roi["y"], roi["y"] + roi["height"]):
        row_start = y * reference.width * 4
        for x in range(roi["x"], roi["x"] + roi["width"]):
            offset = row_start + x * 4
            deltas = [abs(reference.rgba[offset + channel] - candidate.rgba[offset + channel]) for channel in range(4)]
            total_delta += sum(deltas)
            if max(deltas) > CERTIFICATION_TOLERANCE["pixel_delta_threshold"]:
                pixels_over_threshold += 1
    return {
        "roi": roi,
        "roi_pixels": pixel_count,
        "pixel_delta_threshold": CERTIFICATION_TOLERANCE["pixel_delta_threshold"],
        "pixels_over_rgba_delta_threshold": pixels_over_threshold,
        "pixels_over_rgba_delta_threshold_ratio": pixels_over_threshold / pixel_count,
        "mean_abs_channel_delta": total_delta / (pixel_count * 4 * 255),
    }


def validate_certification_tolerance(tolerance: dict[str, Any]) -> None:
    if set(tolerance) != set(CERTIFICATION_TOLERANCE):
        raise GateError("certificación exige exactamente el presupuesto V1 de tolerancia")
    for key, expected in CERTIFICATION_TOLERANCE.items():
        actual = tolerance.get(key)
        if not isinstance(actual, (int, float)) or not math.isclose(actual, expected, rel_tol=0, abs_tol=1e-15):
            raise GateError(f"certificación exige {key}={expected}")


def within_certification_budget(metrics: dict[str, Any], tolerance: dict[str, Any]) -> bool:
    validate_certification_tolerance(tolerance)
    return (
        metrics["pixels_over_rgba_delta_threshold_ratio"] <= tolerance["max_pixels_over_threshold_ratio"]
        and metrics["mean_abs_channel_delta"] <= tolerance["max_mean_abs_channel_delta"]
    )


def parse_timestamp(value: Any, field: str) -> datetime:
    if not isinstance(value, str):
        raise GateError(f"{field} debe ser RFC3339 con zona horaria")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise GateError(f"{field} debe ser RFC3339 con zona horaria") from exc
    if parsed.tzinfo is None:
        raise GateError(f"{field} debe incluir zona horaria")
    return parsed.astimezone(timezone.utc)


def valid_git_sha(value: Any) -> bool:
    return isinstance(value, str) and bool(GIT_SHA_RE.fullmatch(value))


def capture_id(provenance: dict[str, Any], candidate_png_sha256: str, profile: dict[str, Any]) -> str:
    """Bind a fresh runtime-capture declaration to one exact candidate PNG."""
    fields = (
        provenance.get("candidate_sha"),
        provenance.get("captured_at"),
        provenance.get("capture_command"),
        provenance.get("source"),
        candidate_png_sha256,
        profile_key(profile),
    )
    return hashlib.sha256("\x1f".join(str(value) for value in fields).encode("utf-8")).hexdigest()


def candidate_provenance_for_profile(
    provenance: dict[str, Any], candidate_path: Path, profile: dict[str, Any]
) -> dict[str, Any]:
    result = dict(provenance)
    result["candidate_png_sha256"] = sha256(candidate_path)
    result["profile"] = profile_key(profile)
    result["capture_id"] = capture_id(result, result["candidate_png_sha256"], profile)
    return result


def relative(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateError(f"no se pudo leer manifiesto {path}: {exc}") from exc
    if data.get("schema_version") != 2 or not isinstance(data.get("windows"), list):
        raise GateError(f"{path}: schema_version=2/windows requeridos")
    if data.get("comparison_role") not in VALID_COMPARISON_ROLES:
        raise GateError(f"{path}: comparison_role debe clasificar diagnóstico o certificación")
    return data


def profile_directory(entry: dict[str, Any], profile: dict[str, Any]) -> Path:
    root = entry.get("artifact_root")
    if not isinstance(root, str):
        raise GateError("window sin artifact_root")
    width, height, scale = profile.get("width"), profile.get("height"), profile.get("ui_scale")
    if not isinstance(width, int) or not isinstance(height, int) or not isinstance(scale, int):
        raise GateError("perfil sin width/height/ui_scale enteros")
    return ROOT / root / f"{width}x{height}-{scale}x"


def expected_profiles(entry: dict[str, Any]) -> list[dict[str, Any]]:
    profiles = entry.get("profiles")
    if not isinstance(profiles, list) or not profiles:
        raise GateError("window sin profiles")
    found = {(p.get("width"), p.get("height"), p.get("ui_scale")) for p in profiles if isinstance(p, dict)}
    needed = {(1280, 720, 1), (1280, 720, 2), (1920, 1080, 1), (1920, 1080, 2)}
    if found != needed:
        raise GateError(f"perfiles incompletos: esperado {sorted(needed)}, recibido {sorted(found)}")
    return profiles


def artifact_metadata(paths: dict[str, Path]) -> dict[str, dict[str, str]]:
    return {
        name: {"path": relative(path), "sha256": sha256(path)}
        for name, path in paths.items()
        if name != "sidecar"
    }


def build_sidecar(
    entry: dict[str, Any],
    profile: dict[str, Any],
    paths: dict[str, Path],
    metrics: dict[str, Any],
    category: str,
    role: str,
    regression: dict[str, Any] | None = None,
    candidate_provenance: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build either a preserved diagnostic sidecar or a certification one."""
    tolerance = entry["tolerance"]
    if role == DIAGNOSTIC_ROLE:
        # Keep schema v1 byte-for-byte compatible with the historical archive.
        sidecar = {
            "schema_version": 1,
            "window": entry["id"],
            "family": entry["family"],
            "profile": profile,
            "openttd_commit": entry["openttd_commit"],
            "fixture": entry["fixture"],
            "geometry": {
                "expected_screen": [profile["width"], profile["height"]],
                "reference_screen": metrics["reference_size"],
                "candidate_screen": metrics["candidate_size"],
            },
            "artifacts": artifact_metadata(paths),
            "metrics": metrics,
            "assessment": {"category": category, "within_tolerance": within_tolerance(metrics, tolerance)},
            "tolerance": tolerance,
            "accepted_differences": entry.get("accepted_differences", []),
        }
    else:
        if regression is None or candidate_provenance is None:
            raise GateError("certificación sin métricas o procedencia de candidato")
        sidecar = {
            "schema_version": 2,
            "comparison_role": CERTIFICATION_ROLE,
            "window": entry["id"],
            "family": entry["family"],
            "profile": profile,
            "fixture": entry["fixture"],
            "geometry": {
                "expected_screen": [profile["width"], profile["height"]],
                "reference_screen": metrics["reference_size"],
                "candidate_screen": metrics["candidate_size"],
            },
            "artifacts": artifact_metadata(paths),
            "metrics": metrics,
            "regression_metrics": regression,
            "assessment": {"category": category, "within_tolerance": within_certification_budget(regression, tolerance)},
            "tolerance": tolerance,
            "required_controls": entry["required_controls"],
            "baseline_provenance": entry["baseline_provenance"],
            "candidate_provenance": candidate_provenance,
        }
    if "capture_route" in entry:
        sidecar["capture_route"] = entry["capture_route"]
    return sidecar


def validate_artifacts(sidecar: dict[str, Any], paths: dict[str, Path]) -> list[str]:
    errors: list[str] = []
    artifacts = sidecar.get("artifacts", {})
    for name in ("reference", "candidate", "diff"):
        artifact = artifacts.get(name)
        if not isinstance(artifact, dict) or artifact.get("sha256") != sha256(paths[name]):
            errors.append(f"sidecar.artifacts.{name} no coincide con archivo")
    return errors


def validate_common_sidecar(sidecar: dict[str, Any], expected: dict[str, Any], paths: dict[str, Path]) -> list[str]:
    errors: list[str] = []
    expected_values = {
        "window": expected["id"],
        "family": expected["family"],
        "profile": expected["profile"],
        "fixture": expected["fixture"],
        "tolerance": expected["tolerance"],
    }
    if "capture_route" in expected:
        expected_values["capture_route"] = expected["capture_route"]
    for key, value in expected_values.items():
        if sidecar.get(key) != value:
            errors.append(f"sidecar.{key} no coincide con manifiesto")
    geometry = sidecar.get("geometry", {})
    expected_screen = [expected["profile"]["width"], expected["profile"]["height"]]
    if geometry.get("expected_screen") != expected_screen:
        errors.append("sidecar.geometry.expected_screen no coincide con perfil")
    return errors + validate_artifacts(sidecar, paths)


def validate_diagnostic_sidecar(sidecar: dict[str, Any], expected: dict[str, Any], paths: dict[str, Path]) -> list[str]:
    errors = validate_common_sidecar(sidecar, expected, paths)
    if sidecar.get("schema_version") != 1:
        errors.append("sidecar.schema_version diagnóstico debe ser 1")
    if sidecar.get("openttd_commit") != expected["openttd_commit"]:
        errors.append("sidecar.openttd_commit no coincide con manifiesto")
    if sidecar.get("accepted_differences") != expected.get("accepted_differences", []):
        errors.append("sidecar.accepted_differences no coincide con manifiesto")
    return errors


def validate_required_controls(required_controls: Any) -> list[str]:
    if not isinstance(required_controls, list) or not required_controls:
        raise GateError("certificación requiere required_controls no vacío")
    if any(not isinstance(control, str) or not control.strip() for control in required_controls):
        raise GateError("required_controls debe contener identificadores no vacíos")
    if len(set(required_controls)) != len(required_controls):
        raise GateError("required_controls contiene identificadores duplicados")
    return required_controls


def validate_baseline_provenance(provenance: Any) -> None:
    if not isinstance(provenance, dict):
        raise GateError("certificación requiere baseline_provenance")
    if not valid_git_sha(provenance.get("baseline_sha")):
        raise GateError("baseline_provenance.baseline_sha debe ser SHA Git de 40 caracteres")
    parse_timestamp(provenance.get("captured_at"), "baseline_provenance.captured_at")
    if not isinstance(provenance.get("capture_command"), str) or not provenance["capture_command"].strip():
        raise GateError("baseline_provenance.capture_command es obligatorio")
    if provenance.get("source") != "runtime":
        raise GateError("baseline_provenance.source debe ser runtime")


def validate_candidate_input(provenance: Any, candidate_sha: str) -> dict[str, Any]:
    if not isinstance(provenance, dict):
        raise GateError("--candidate-provenance debe contener un objeto JSON")
    if provenance.get("candidate_sha") != candidate_sha:
        raise GateError("candidate_provenance.candidate_sha no coincide con --candidate-sha")
    if not valid_git_sha(candidate_sha):
        raise GateError("--candidate-sha debe ser SHA Git de 40 caracteres")
    parse_timestamp(provenance.get("captured_at"), "candidate_provenance.captured_at")
    if not isinstance(provenance.get("capture_command"), str) or not provenance["capture_command"].strip():
        raise GateError("candidate_provenance.capture_command es obligatorio")
    if provenance.get("source") != "runtime":
        raise GateError("candidate_provenance.source debe ser runtime")
    if not isinstance(provenance.get("control_assertions"), dict):
        raise GateError("candidate_provenance.control_assertions es obligatorio")
    return provenance


def validate_candidate_provenance(
    provenance: Any,
    candidate_sha: str,
    candidate_path: Path,
    profile: dict[str, Any],
    baseline_provenance: dict[str, Any],
    required_controls: list[str],
) -> list[str]:
    errors: list[str] = []
    if not isinstance(provenance, dict):
        return ["sidecar.candidate_provenance ausente: falta captura fresca del candidato"]
    if provenance.get("candidate_sha") != candidate_sha:
        errors.append("sidecar.candidate_provenance.candidate_sha ausente o distinto de --candidate-sha")
    try:
        captured_at = parse_timestamp(provenance.get("captured_at"), "sidecar.candidate_provenance.captured_at")
        baseline_at = parse_timestamp(baseline_provenance.get("captured_at"), "baseline_provenance.captured_at")
        if captured_at < baseline_at:
            errors.append("sidecar.candidate_provenance no es una captura posterior al baseline")
    except GateError as exc:
        errors.append(str(exc))
    if not isinstance(provenance.get("capture_command"), str) or not provenance["capture_command"].strip():
        errors.append("sidecar.candidate_provenance.capture_command ausente")
    if provenance.get("source") != "runtime":
        errors.append("sidecar.candidate_provenance.source debe ser runtime")
    candidate_hash = sha256(candidate_path)
    if provenance.get("candidate_png_sha256") != candidate_hash:
        errors.append("sidecar.candidate_provenance no está ligada al PNG candidato")
    if provenance.get("profile") != profile_key(profile):
        errors.append("sidecar.candidate_provenance.profile no coincide con captura")
    if provenance.get("capture_id") != capture_id(provenance, candidate_hash, profile):
        errors.append("sidecar.candidate_provenance.capture_id no liga SHA, fecha, perfil y PNG")

    assertions = provenance.get("control_assertions")
    if not isinstance(assertions, dict):
        errors.append("sidecar.candidate_provenance.control_assertions ausente")
    else:
        for control in required_controls:
            assertion = assertions.get(control)
            if not isinstance(assertion, dict) or assertion.get("present") is not True:
                errors.append(f"control ausente: {control}")
            elif assertion.get("actionable") is not True:
                errors.append(f"control inaccesible: {control}")
    return errors


def validate_certification_sidecar(
    sidecar: dict[str, Any], expected: dict[str, Any], paths: dict[str, Path], candidate_sha: str
) -> list[str]:
    errors = validate_common_sidecar(sidecar, expected, paths)
    if sidecar.get("schema_version") != 2:
        errors.append("sidecar.schema_version de certificación debe ser 2")
    if sidecar.get("comparison_role") != CERTIFICATION_ROLE:
        errors.append("sidecar.comparison_role no es client_regression")
    if sidecar.get("required_controls") != expected["required_controls"]:
        errors.append("sidecar.required_controls no coincide con manifiesto")
    if sidecar.get("baseline_provenance") != expected["baseline_provenance"]:
        errors.append("sidecar.baseline_provenance no coincide con manifiesto")
    errors.extend(
        validate_candidate_provenance(
            sidecar.get("candidate_provenance"),
            candidate_sha,
            paths["candidate"],
            expected["profile"],
            expected["baseline_provenance"],
            expected["required_controls"],
        )
    )
    return errors


def accepts_difference(entry: dict[str, Any], category: str) -> bool:
    """A non-identical diagnostic baseline must cite an issue in its exact category."""
    return any(item.get("category") == category for item in entry.get("accepted_differences", []))


def append_error(errors: list[dict[str, str]], entry: dict[str, Any], tag: str, category: str, detail: str) -> None:
    errors.append({"window": entry["id"], "profile": tag, "category": category, "detail": detail})


def assess_entry(
    entry: dict[str, Any],
    role: str,
    write_sidecars: bool,
    candidate_sha: str | None,
    candidate_provenance: dict[str, Any] | None,
) -> tuple[list[dict[str, str]], list[str]]:
    errors: list[dict[str, str]] = []
    notices: list[str] = []
    for profile in expected_profiles(entry):
        directory = profile_directory(entry, profile)
        paths = {
            "reference": directory / "reference.png",
            "candidate": directory / "candidate.png",
            "diff": directory / "diff.png",
            "sidecar": directory / "sidecar.json",
        }
        tag = f"{entry['id']} {profile['width']}x{profile['height']}@{profile['ui_scale']}x"
        required = ("reference", "candidate") if write_sidecars else ("reference", "candidate", "diff", "sidecar")
        missing = [name for name in required if not paths[name].is_file()]
        if missing:
            append_error(errors, entry, tag, "absence", f"faltan {', '.join(missing)}")
            continue
        try:
            reference, candidate = read_png(paths["reference"]), read_png(paths["candidate"])
            metrics, diff, category = compare(reference, candidate)
            if diff is None:
                append_error(errors, entry, tag, "geometry", f"referencia={metrics['reference_size']} candidato={metrics['candidate_size']}")
                continue
            if metrics["reference_size"] != [profile["width"], profile["height"]] or metrics["candidate_size"] != [profile["width"], profile["height"]]:
                append_error(
                    errors,
                    entry,
                    tag,
                    "geometry",
                    f"se esperaba {[profile['width'], profile['height']]}; referencia={metrics['reference_size']} candidato={metrics['candidate_size']}",
                )
                continue

            expected = dict(entry)
            expected["profile"] = profile
            if role == DIAGNOSTIC_ROLE:
                if metrics["changed_pixels"] and not accepts_difference(entry, category):
                    append_error(errors, entry, tag, "openttd_similarity", "diff no idéntico sin accepted_differences enlazada a issue")
                    continue
                if not within_tolerance(metrics, entry["tolerance"]):
                    append_error(errors, entry, tag, "openttd_similarity", f"diff excede tolerancia diagnóstica: {metrics}")
                    continue
                if write_sidecars:
                    write_png(paths["diff"], diff)
                    sidecar = build_sidecar(entry, profile, paths, metrics, category, role)
                    paths["sidecar"].write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                    notices.append(f"actualizado {relative(paths['sidecar'])}")
                    continue
                sidecar = json.loads(paths["sidecar"].read_text(encoding="utf-8"))
                for detail in validate_diagnostic_sidecar(sidecar, expected, paths):
                    append_error(errors, entry, tag, "artifact_integrity", detail)
                if sidecar.get("metrics") != metrics:
                    append_error(errors, entry, tag, "artifact_integrity", "sidecar.metrics no coincide con imágenes")
                continue

            if candidate_sha is None:
                raise GateError("certificación sin --candidate-sha")
            roi = roi_for_profile(profile)
            regression = certification_metrics(reference, candidate, roi)
            if not within_certification_budget(regression, entry["tolerance"]):
                append_error(errors, entry, tag, "client_regression", f"diff excede presupuesto V1: {regression}")
                continue
            if write_sidecars:
                if candidate_provenance is None:
                    raise GateError("--write-sidecars de certificación requiere --candidate-provenance")
                profile_provenance = candidate_provenance_for_profile(candidate_provenance, paths["candidate"], profile)
                provenance_errors = validate_candidate_provenance(
                    profile_provenance,
                    candidate_sha,
                    paths["candidate"],
                    profile,
                    entry["baseline_provenance"],
                    entry["required_controls"],
                )
                for detail in provenance_errors:
                    append_error(errors, entry, tag, "candidate_provenance", detail)
                if provenance_errors:
                    continue
                write_png(paths["diff"], diff)
                sidecar = build_sidecar(entry, profile, paths, metrics, category, role, regression, profile_provenance)
                paths["sidecar"].write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                notices.append(f"actualizado {relative(paths['sidecar'])}")
                continue

            sidecar = json.loads(paths["sidecar"].read_text(encoding="utf-8"))
            for detail in validate_certification_sidecar(sidecar, expected, paths, candidate_sha):
                append_error(errors, entry, tag, "candidate_provenance" if "provenance" in detail or "control " in detail else "artifact_integrity", detail)
            if sidecar.get("metrics") != metrics:
                append_error(errors, entry, tag, "artifact_integrity", "sidecar.metrics no coincide con imágenes")
            if sidecar.get("regression_metrics") != regression:
                append_error(errors, entry, tag, "client_regression", "sidecar.regression_metrics no coincide con imágenes y ROI")
        except (GateError, OSError, json.JSONDecodeError) as exc:
            append_error(errors, entry, tag, "absence", str(exc))
    return errors, notices


def assess_entry_worker(
    args: tuple[dict[str, Any], str, bool, str | None, dict[str, Any] | None]
) -> tuple[list[dict[str, str]], list[str]]:
    """Evalúa una ventana en un proceso separado para evitar el GIL del diff RGBA."""
    return assess_entry(*args)


def validate_entry(entry: dict[str, Any], role: str) -> None:
    identifier = entry.get("id")
    if not isinstance(entry.get("tolerance"), dict):
        raise GateError(f"{identifier}: falta tolerance")
    if entry.get("family") not in {"vehicles", "world", "construction", "economy", "settings", "dialogs"}:
        raise GateError(f"{identifier}: familia no cubierta")
    if not isinstance(entry.get("capture_route"), str) or not entry["capture_route"].strip():
        raise GateError(f"{identifier}: falta capture_route de {entry['family']}")
    if not isinstance(entry.get("fixture"), str) or not entry["fixture"].strip():
        raise GateError(f"{identifier}: falta fixture")

    profiles = expected_profiles(entry)
    if role == DIAGNOSTIC_ROLE:
        if not valid_git_sha(entry.get("openttd_commit")):
            raise GateError(f"{identifier}: openttd_commit debe ser SHA Git de 40 caracteres")
        accepted = entry.get("accepted_differences", [])
        if not isinstance(accepted, list) or any(
            not isinstance(item, dict)
            or item.get("category") not in VALID_CATEGORIES
            or not isinstance(item.get("issue"), int)
            or item["issue"] <= 0
            for item in accepted
        ):
            raise GateError(f"{identifier}: accepted_differences inválido")
        return

    validate_certification_tolerance(entry["tolerance"])
    if entry.get("accepted_differences", []) != []:
        raise GateError(f"{identifier}: certificación no acepta accepted_differences")
    required_controls = validate_required_controls(entry.get("required_controls"))
    validate_baseline_provenance(entry.get("baseline_provenance"))
    for profile in profiles:
        roi_for_profile(profile)
    # Keep the name alive for a helpful type/intent check above.
    if not required_controls:
        raise GateError(f"{identifier}: required_controls vacío")


def load_candidate_provenance(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateError(f"no se pudo leer candidate provenance {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise GateError("candidate provenance debe ser un objeto JSON")
    return data


def assessment_report(role: str, errors: list[dict[str, str]]) -> dict[str, dict[str, Any]]:
    integrity_categories = {"absence", "geometry", "artifact_integrity"}
    integrity_errors = [error for error in errors if error["category"] in integrity_categories]
    client_errors = [
        error
        for error in errors
        if error["category"] in {"client_regression", "candidate_provenance"}
    ]
    similarity_errors = [error for error in errors if error["category"] == "openttd_similarity"]

    integrity = {"status": "fail" if integrity_errors else "pass", "errors": integrity_errors}
    if role == DIAGNOSTIC_ROLE:
        client = {"status": "not_applicable", "errors": []}
        similarity = {
            "status": "fail" if similarity_errors else "diagnostic",
            "errors": similarity_errors,
        }
    else:
        client = {
            "status": "fail" if client_errors else ("not_evaluated" if integrity_errors else "pass"),
            "errors": client_errors,
        }
        similarity = {"status": "not_applicable", "errors": []}
    return {
        "artifact_integrity": integrity,
        "client_regression": client,
        "openttd_similarity": similarity,
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument(
        "--window",
        action="append",
        default=[],
        metavar="ID",
        help="evalúa únicamente esta ventana (repetible); por defecto evalúa todas",
    )
    parser.add_argument("--write-sidecars", action="store_true", help="regenera diff + sidecar desde referencia y candidato")
    parser.add_argument(
        "--mode",
        choices=("diagnostic", "certification"),
        default="diagnostic",
        help="diagnostic conserva la comparación OpenTTD; certification activa sólo el presupuesto V1 propio",
    )
    parser.add_argument(
        "--candidate-sha",
        help="SHA Git completo del binario/captura candidata; obligatorio para certificación",
    )
    parser.add_argument(
        "--candidate-provenance",
        type=Path,
        help="JSON de captura runtime fresca; obligatorio al regenerar sidecars de certificación",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=0,
        help="procesos para comparar ventanas (por defecto: hasta 4; 1 desactiva el paralelismo)",
    )
    args = parser.parse_args(argv)
    try:
        if args.jobs < 0:
            raise GateError("--jobs no puede ser negativo")
        manifest = load_manifest(args.manifest)
        role = manifest["comparison_role"]
        if args.mode == "certification" and role != CERTIFICATION_ROLE:
            raise GateError("certificación rechaza perfiles sólo diagnósticos de similitud OpenTTD")
        if args.mode == "diagnostic" and role != DIAGNOSTIC_ROLE:
            raise GateError("un manifiesto client_regression exige --mode certification")
        candidate_sha: str | None = None
        candidate_provenance: dict[str, Any] | None = None
        if args.mode == "certification":
            if not valid_git_sha(args.candidate_sha):
                raise GateError("certificación requiere --candidate-sha Git completo de 40 caracteres")
            candidate_sha = args.candidate_sha
            if args.write_sidecars:
                if args.candidate_provenance is None:
                    raise GateError("--write-sidecars de certificación requiere --candidate-provenance")
                candidate_provenance = validate_candidate_input(
                    load_candidate_provenance(args.candidate_provenance), candidate_sha
                )
            elif args.candidate_provenance is not None:
                raise GateError("--candidate-provenance sólo se usa al regenerar sidecars de certificación")
        elif args.candidate_sha is not None or args.candidate_provenance is not None:
            raise GateError("--candidate-sha/provenance sólo se permiten con --mode certification")

        windows = manifest["windows"]
        ids = [entry.get("id") for entry in windows if isinstance(entry, dict)]
        if len(ids) != len(set(ids)) or any(not isinstance(value, str) for value in ids):
            raise GateError("ids de ventana inválidos o duplicados")
        requested_ids = set(args.window)
        unknown = requested_ids.difference(ids)
        if unknown:
            raise GateError(f"--window desconocida: {', '.join(sorted(unknown))}")
        all_errors: list[dict[str, str]] = []
        notices: list[str] = []
        selected: list[dict[str, Any]] = []
        for entry in windows:
            if not isinstance(entry, dict):
                raise GateError("entrada de ventana inválida")
            if requested_ids and entry["id"] not in requested_ids:
                continue
            validate_entry(entry, role)
            selected.append(entry)

        jobs = args.jobs or min(4, os.cpu_count() or 1)
        tasks = [(entry, role, args.write_sidecars, candidate_sha, candidate_provenance) for entry in selected]
        if jobs == 1 or len(tasks) <= 1:
            results = (assess_entry_worker(task) for task in tasks)
        else:
            try:
                with ProcessPoolExecutor(max_workers=jobs) as executor:
                    results = list(executor.map(assess_entry_worker, tasks))
            except (OSError, PermissionError):
                # Algunos runners restringidos no permiten fork/spawn; conservar
                # el gate funcional allí, aunque sin aceleración.
                results = (assess_entry_worker(task) for task in tasks)
        for errors, updated in results:
            all_errors.extend(errors)
            notices.extend(updated)
    except GateError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2

    report = {
        "schema_version": 2,
        "manifest": relative(args.manifest),
        "comparison_role": role,
        "mode": args.mode,
        "status": "pass" if not all_errors else "fail",
        "assessments": assessment_report(role, all_errors),
        "errors": all_errors,
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    for notice in notices:
        print(f"OK: {notice}", file=sys.stderr)
    return 0 if not all_errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
