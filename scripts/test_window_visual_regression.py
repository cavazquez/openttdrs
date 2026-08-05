#!/usr/bin/env python3
"""Prueba de mutación del gate visual de ventanas (#297, #299)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GATE = ROOT / "scripts" / "window_visual_regression.py"


def write_manifest(root: Path) -> Path:
    profiles = [
        {"width": 1280, "height": 720, "ui_scale": 1},
        {"width": 1280, "height": 720, "ui_scale": 2},
        {"width": 1920, "height": 1080, "ui_scale": 1},
        {"width": 1920, "height": 1080, "ui_scale": 2},
    ]
    data = {
        "schema_version": 1,
        "windows": [
            {
                "id": "Vehicle",
                "family": "vehicles",
                "fixture": "fixture.sav",
                "openttd_commit": "0" * 40,
                "artifact_root": str(root / "artifacts"),
                "profiles": profiles,
                "tolerance": {
                    "max_changed_pixels": 0,
                    "max_changed_ratio": 0,
                    "max_channel_delta": 0,
                    "max_mean_channel_delta": 0,
                },
                "accepted_differences": [],
            }
        ],
    }
    path = root / "manifest.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def run(manifest: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(GATE), "--manifest", str(manifest), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def main() -> int:
    sys.path.insert(0, str(ROOT / "scripts"))
    import window_visual_regression as visual

    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        manifest = write_manifest(root)
        for width, height, scale in ((1280, 720, 1), (1280, 720, 2), (1920, 1080, 1), (1920, 1080, 2)):
            directory = root / "artifacts" / f"{width}x{height}-{scale}x"
            image = visual.PngImage(width, height, bytes((16, 32, 48, 255)) * (width * height))
            visual.write_png(directory / "reference.png", image)
            visual.write_png(directory / "candidate.png", image)

        updated = run(manifest, "--write-sidecars")
        if updated.returncode != 0:
            print(updated.stdout, updated.stderr, file=sys.stderr)
            print("FAIL: no se pudieron generar sidecars", file=sys.stderr)
            return 1
        passed = run(manifest)
        if passed.returncode != 0:
            print(passed.stdout, passed.stderr, file=sys.stderr)
            print("FAIL: baseline idéntico debería pasar", file=sys.stderr)
            return 1
        passed_one = run(manifest, "--window", "Vehicle")
        if passed_one.returncode != 0:
            print(passed_one.stdout, passed_one.stderr, file=sys.stderr)
            print("FAIL: un gate filtrado debería pasar", file=sys.stderr)
            return 1
        unknown = run(manifest, "--window", "Town")
        if unknown.returncode != 2 or "desconocida" not in unknown.stderr:
            print(unknown.stdout, unknown.stderr, file=sys.stderr)
            print("FAIL: una ventana no declarada debe rechazarse", file=sys.stderr)
            return 1

        data = json.loads(manifest.read_text(encoding="utf-8"))
        data["windows"][0]["family"] = "construction"
        manifest.write_text(json.dumps(data), encoding="utf-8")
        missing_route = run(manifest)
        if missing_route.returncode != 2 or "capture_route" not in missing_route.stderr:
            print(missing_route.stdout, missing_route.stderr, file=sys.stderr)
            print("FAIL: construction sin ruta de captura debe rechazarse", file=sys.stderr)
            return 1
        data["windows"][0]["family"] = "vehicles"
        manifest.write_text(json.dumps(data), encoding="utf-8")

        data["windows"][0]["family"] = "economy"
        manifest.write_text(json.dumps(data), encoding="utf-8")
        missing_route = run(manifest)
        if missing_route.returncode != 2 or "capture_route" not in missing_route.stderr:
            print(missing_route.stdout, missing_route.stderr, file=sys.stderr)
            print("FAIL: economy sin ruta de captura debe rechazarse", file=sys.stderr)
            return 1
        data["windows"][0]["family"] = "vehicles"
        manifest.write_text(json.dumps(data), encoding="utf-8")

        changed = root / "artifacts" / "1280x720-1x" / "candidate.png"
        pixels = bytearray(visual.read_png(changed).rgba)
        pixels[:4] = bytes((255, 0, 0, 255))
        visual.write_png(changed, visual.PngImage(1280, 720, bytes(pixels)))
        data = json.loads(manifest.read_text(encoding="utf-8"))
        data["windows"][0]["tolerance"] = {
            "max_changed_pixels": 2_073_600,
            "max_changed_ratio": 1,
            "max_channel_delta": 255,
            "max_mean_channel_delta": 255,
        }
        manifest.write_text(json.dumps(data), encoding="utf-8")
        unaccepted = run(manifest)
        if unaccepted.returncode != 1 or "sin accepted_differences" not in unaccepted.stdout:
            print(unaccepted.stdout, unaccepted.stderr, file=sys.stderr)
            print("FAIL: un diff baseline debe enlazar un issue", file=sys.stderr)
            return 1

        data["windows"][0]["accepted_differences"] = [{"category": "chromatic", "issue": 297}]
        manifest.write_text(json.dumps(data), encoding="utf-8")
        refreshed = run(manifest, "--write-sidecars")
        if refreshed.returncode != 0:
            print(refreshed.stdout, refreshed.stderr, file=sys.stderr)
            print("FAIL: un baseline aceptado debería poder regenerarse", file=sys.stderr)
            return 1
        accepted = run(manifest)
        if accepted.returncode != 0:
            print(accepted.stdout, accepted.stderr, file=sys.stderr)
            print("FAIL: un baseline aceptado con sidecar actualizado debería pasar", file=sys.stderr)
            return 1

        pixels[4:8] = bytes((0, 255, 0, 255))
        visual.write_png(changed, visual.PngImage(1280, 720, bytes(pixels)))
        failed = run(manifest)
        if failed.returncode != 1:
            print(failed.stdout, failed.stderr, file=sys.stderr)
            print("FAIL: una regresión inyectada debe invalidar el sidecar", file=sys.stderr)
            return 1

        missing = root / "artifacts" / "1920x1080-2x" / "reference.png"
        missing.unlink()
        failed_missing = run(manifest)
        if failed_missing.returncode != 1 or '"absence"' not in failed_missing.stdout:
            print(failed_missing.stdout, failed_missing.stderr, file=sys.stderr)
            print("FAIL: una captura ausente debe fallar", file=sys.stderr)
            return 1

    print("OK: gate visual detecta regresión inyectada y captura ausente")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
