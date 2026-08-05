#!/usr/bin/env python3
"""Gate reproducible de regresión visual para familias de ventanas (#297, #299, #300).

Cada perfil mantiene cuatro archivos versionados: referencia OpenTTD,
candidato openttdrs, diff RGBA y sidecar JSON. El lector PNG es deliberadamente
stdlib-only para que el gate no dependa de Pillow/ImageMagick en CI.
"""

from __future__ import annotations

import argparse
import hashlib
import json
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
        elif kind == b"IEND":
            break
        pos = end + 4

    if width is None or height is None or bit_depth != 8 or color_type not in (2, 6):
        raise GateError(f"{path}: se requiere PNG RGB/RGBA de 8 bits no entrelazado")
    if width <= 0 or height <= 0:
        raise GateError(f"{path}: dimensiones PNG inválidas")
    channels = 3 if color_type == 2 else 4
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
    if data.get("schema_version") != 1 or not isinstance(data.get("windows"), list):
        raise GateError(f"{path}: schema_version/windows inválidos")
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


def build_sidecar(
    entry: dict[str, Any], profile: dict[str, Any], paths: dict[str, Path], metrics: dict[str, Any], category: str
) -> dict[str, Any]:
    tolerance = entry["tolerance"]
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
        "artifacts": {
            name: {"path": relative(path), "sha256": sha256(path)}
            for name, path in paths.items()
            if name != "sidecar"
        },
        "metrics": metrics,
        "assessment": {"category": category, "within_tolerance": within_tolerance(metrics, tolerance)},
        "tolerance": tolerance,
        "accepted_differences": entry.get("accepted_differences", []),
    }
    if "capture_route" in entry:
        sidecar["capture_route"] = entry["capture_route"]
    return sidecar


def validate_sidecar(sidecar: dict[str, Any], expected: dict[str, Any], paths: dict[str, Path]) -> list[str]:
    errors: list[str] = []
    expected_values = {"window": expected["id"], **{key: expected[key] for key in ("family", "profile", "openttd_commit", "fixture", "tolerance")}}
    if "capture_route" in expected:
        expected_values["capture_route"] = expected["capture_route"]
    for key, value in expected_values.items():
        if sidecar.get(key) != value:
            errors.append(f"sidecar.{key} no coincide con manifiesto")
    geometry = sidecar.get("geometry", {})
    expected_screen = [expected["profile"]["width"], expected["profile"]["height"]]
    if geometry.get("expected_screen") != expected_screen:
        errors.append("sidecar.geometry.expected_screen no coincide con perfil")
    artifacts = sidecar.get("artifacts", {})
    for name in ("reference", "candidate", "diff"):
        artifact = artifacts.get(name)
        if not isinstance(artifact, dict) or artifact.get("sha256") != sha256(paths[name]):
            errors.append(f"sidecar.artifacts.{name} no coincide con archivo")
    return errors


def accepts_difference(entry: dict[str, Any], category: str) -> bool:
    """A non-identical baseline must cite an issue in its exact category."""
    return any(item.get("category") == category for item in entry.get("accepted_differences", []))


def assess_entry(entry: dict[str, Any], write_sidecars: bool) -> tuple[list[dict[str, str]], list[str]]:
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
            errors.append({"window": entry["id"], "profile": tag, "category": "absence", "detail": f"faltan {', '.join(missing)}"})
            continue
        try:
            reference, candidate = read_png(paths["reference"]), read_png(paths["candidate"])
            metrics, diff, category = compare(reference, candidate)
            if diff is None:
                errors.append({"window": entry["id"], "profile": tag, "category": "geometry", "detail": f"referencia={metrics['reference_size']} candidato={metrics['candidate_size']}"})
                continue
            if metrics["reference_size"] != [profile["width"], profile["height"]] or metrics["candidate_size"] != [profile["width"], profile["height"]]:
                errors.append({"window": entry["id"], "profile": tag, "category": "geometry", "detail": f"se esperaba {[profile['width'], profile['height']]}; referencia={metrics['reference_size']} candidato={metrics['candidate_size']}"})
                continue
            if metrics["changed_pixels"] and not accepts_difference(entry, category):
                errors.append(
                    {
                        "window": entry["id"],
                        "profile": tag,
                        "category": category,
                        "detail": "diff no idéntico sin accepted_differences enlazada a issue",
                    }
                )
                continue
            if write_sidecars:
                write_png(paths["diff"], diff)
                sidecar = build_sidecar(entry, profile, paths, metrics, category)
                paths["sidecar"].write_text(json.dumps(sidecar, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                notices.append(f"actualizado {relative(paths['sidecar'])}")
                continue
            sidecar = json.loads(paths["sidecar"].read_text(encoding="utf-8"))
            expected = dict(entry)
            expected["profile"] = profile
            for detail in validate_sidecar(sidecar, expected, paths):
                errors.append({"window": entry["id"], "profile": tag, "category": "geometry", "detail": detail})
            if sidecar.get("metrics") != metrics:
                errors.append({"window": entry["id"], "profile": tag, "category": category, "detail": "sidecar.metrics no coincide con imágenes"})
            if not within_tolerance(metrics, entry["tolerance"]):
                errors.append({"window": entry["id"], "profile": tag, "category": category, "detail": f"diff excede tolerancia: {metrics}"})
        except (GateError, OSError, json.JSONDecodeError) as exc:
            errors.append({"window": entry["id"], "profile": tag, "category": "absence", "detail": str(exc)})
    return errors, notices


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
    args = parser.parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
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
        for entry in windows:
            if not isinstance(entry, dict):
                raise GateError("entrada de ventana inválida")
            if requested_ids and entry["id"] not in requested_ids:
                continue
            if not isinstance(entry.get("tolerance"), dict):
                raise GateError(f"{entry.get('id')}: falta tolerance")
            if entry.get("family") not in {"vehicles", "world", "construction", "economy"}:
                raise GateError(f"{entry.get('id')}: familia no cubierta")
            if entry.get("family") in {"construction", "economy"} and not isinstance(entry.get("capture_route"), str):
                raise GateError(f"{entry.get('id')}: falta capture_route de {entry['family']}")
            accepted = entry.get("accepted_differences", [])
            if not isinstance(accepted, list) or any(
                not isinstance(item, dict)
                or item.get("category") not in VALID_CATEGORIES
                or not isinstance(item.get("issue"), int)
                or item["issue"] <= 0
                for item in accepted
            ):
                raise GateError(f"{entry.get('id')}: accepted_differences inválido")
            errors, updated = assess_entry(entry, args.write_sidecars)
            all_errors.extend(errors)
            notices.extend(updated)
    except GateError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2

    report = {
        "schema_version": 1,
        "manifest": relative(args.manifest),
        "status": "pass" if not all_errors else "fail",
        "errors": all_errors,
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    for notice in notices:
        print(f"OK: {notice}", file=sys.stderr)
    return 0 if not all_errors else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
