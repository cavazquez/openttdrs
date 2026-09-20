#!/usr/bin/env python3
"""Mutation tests for the diagnostic and V1 client visual gates (#584)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GATE = ROOT / "scripts" / "window_visual_regression.py"


def profiles(*, certification: bool = False) -> list[dict[str, object]]:
    values: list[dict[str, object]] = []
    for width, height, scale in ((1280, 720, 1), (1280, 720, 2), (1920, 1080, 1), (1920, 1080, 2)):
        profile: dict[str, object] = {"width": width, "height": height, "ui_scale": scale}
        if certification:
            profile["roi"] = {"x": 0, "y": 0, "width": width, "height": height}
        values.append(profile)
    return values


def current_sha() -> str:
    return subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        text=True,
        capture_output=True,
        check=True,
    ).stdout.strip()


def write_manifest(root: Path) -> Path:
    data = {
        "schema_version": 2,
        "comparison_role": "openttd_similarity_diagnostic",
        "windows": [
            {
                "id": "Vehicle",
                "family": "vehicles",
                "capture_route": "vehicle-view/main",
                "fixture": "fixture.sav",
                "openttd_commit": "0" * 40,
                "artifact_root": str(root / "artifacts"),
                "profiles": profiles(),
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
    path = root / "diagnostic-manifest.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def write_certification_manifest(root: Path, sha: str) -> Path:
    data = {
        "schema_version": 2,
        "comparison_role": "client_regression",
        "windows": [
            {
                "id": "Orders",
                "family": "vehicles",
                "capture_route": "vehicle-view/orders",
                "fixture": "fixture.sav",
                "artifact_root": str(root / "artifacts"),
                "profiles": profiles(certification=True),
                "tolerance": {
                    "pixel_delta_threshold": 8,
                    "max_pixels_over_threshold_ratio": 0.005,
                    "max_mean_abs_channel_delta": 1 / 255,
                },
                "accepted_differences": [],
                "required_controls": ["orders-list", "goto-button"],
                "baseline_provenance": {
                    "baseline_sha": sha,
                    "captured_at": "2026-09-20T00:00:00Z",
                    "capture_command": "cargo run -p openttdrs-client -- --orders-baseline",
                    "source": "runtime",
                },
            }
        ],
    }
    path = root / "certification-manifest.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def write_candidate_provenance(root: Path, sha: str) -> Path:
    data = {
        "candidate_sha": sha,
        "captured_at": "2026-09-20T00:00:01Z",
        "capture_command": "cargo run -p openttdrs-client -- --orders-candidate",
        "source": "runtime",
        "control_assertions": {
            "orders-list": {"present": True, "actionable": True},
            "goto-button": {"present": True, "actionable": True},
        },
    }
    path = root / "candidate-provenance.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def write_images(root: Path, visual: object) -> None:
    for width, height, scale in ((1280, 720, 1), (1280, 720, 2), (1920, 1080, 1), (1920, 1080, 2)):
        directory = root / "artifacts" / f"{width}x{height}-{scale}x"
        image = visual.PngImage(width, height, bytes((16, 32, 48, 255)) * (width * height))
        visual.write_png(directory / "reference.png", image)
        visual.write_png(directory / "candidate.png", image)


def run(manifest: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(GATE), "--manifest", str(manifest), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def require(result: subprocess.CompletedProcess[str], condition: bool, message: str) -> None:
    if condition:
        return
    print(result.stdout, result.stderr, file=sys.stderr)
    print(f"FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> int:
    sys.path.insert(0, str(ROOT / "scripts"))
    import window_visual_regression as visual

    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        diagnostic = write_manifest(root)
        write_images(root, visual)

        updated = run(diagnostic, "--write-sidecars")
        require(updated, updated.returncode == 0, "no se pudieron generar sidecars diagnósticos")
        passed = run(diagnostic)
        require(passed, passed.returncode == 0, "baseline diagnóstico idéntico debería pasar")
        report = json.loads(passed.stdout)
        require(
            passed,
            report["assessments"]["artifact_integrity"]["status"] == "pass"
            and report["assessments"]["client_regression"]["status"] == "not_applicable"
            and report["assessments"]["openttd_similarity"]["status"] == "diagnostic",
            "el reporte diagnóstico debe separar integridad, regresión propia y similitud OpenTTD",
        )
        passed_one = run(diagnostic, "--window", "Vehicle")
        require(passed_one, passed_one.returncode == 0, "un gate filtrado debería pasar")
        unknown = run(diagnostic, "--window", "Town")
        require(unknown, unknown.returncode == 2 and "desconocida" in unknown.stderr, "una ventana no declarada debe rechazarse")

        data = json.loads(diagnostic.read_text(encoding="utf-8"))
        data["windows"][0]["family"] = "construction"
        del data["windows"][0]["capture_route"]
        diagnostic.write_text(json.dumps(data), encoding="utf-8")
        missing_route = run(diagnostic)
        require(missing_route, missing_route.returncode == 2 and "capture_route" in missing_route.stderr, "construction sin ruta de captura debe rechazarse")
        for family in ("vehicles", "world", "economy", "settings", "dialogs"):
            data["windows"][0]["family"] = family
            diagnostic.write_text(json.dumps(data), encoding="utf-8")
            missing_route = run(diagnostic)
            require(missing_route, missing_route.returncode == 2 and "capture_route" in missing_route.stderr, f"{family} sin ruta de captura debe rechazarse")
        data["windows"][0]["family"] = "vehicles"
        data["windows"][0]["capture_route"] = "vehicle-view/main"
        diagnostic.write_text(json.dumps(data), encoding="utf-8")

        changed = root / "artifacts" / "1280x720-1x" / "candidate.png"
        pixels = bytearray(visual.read_png(changed).rgba)
        pixels[:4] = bytes((255, 0, 0, 255))
        visual.write_png(changed, visual.PngImage(1280, 720, bytes(pixels)))
        data = json.loads(diagnostic.read_text(encoding="utf-8"))
        data["windows"][0]["tolerance"] = {
            "max_changed_pixels": 2_073_600,
            "max_changed_ratio": 1,
            "max_channel_delta": 255,
            "max_mean_channel_delta": 255,
        }
        diagnostic.write_text(json.dumps(data), encoding="utf-8")
        unaccepted = run(diagnostic)
        require(unaccepted, unaccepted.returncode == 1 and "sin accepted_differences" in unaccepted.stdout, "un diff diagnóstico debe enlazar un issue")

        data["windows"][0]["accepted_differences"] = [{"category": "chromatic", "issue": 297}]
        diagnostic.write_text(json.dumps(data), encoding="utf-8")
        refreshed = run(diagnostic, "--write-sidecars")
        require(refreshed, refreshed.returncode == 0, "un baseline diagnóstico aceptado debería poder regenerarse")
        accepted = run(diagnostic)
        require(accepted, accepted.returncode == 0, "un baseline diagnóstico aceptado con sidecar actualizado debería pasar")

        pixels[4:8] = bytes((0, 255, 0, 255))
        visual.write_png(changed, visual.PngImage(1280, 720, bytes(pixels)))
        failed = run(diagnostic)
        require(failed, failed.returncode == 1, "una mutación debe invalidar el sidecar diagnóstico")

        missing = root / "artifacts" / "1920x1080-2x" / "reference.png"
        missing.unlink()
        failed_missing = run(diagnostic)
        require(failed_missing, failed_missing.returncode == 1 and '"absence"' in failed_missing.stdout, "una captura ausente debe fallar")

        # Same dimensions, but a distinct role and distinct sidecar schema.
        write_images(root, visual)
        sha = current_sha()
        certification = write_certification_manifest(root, sha)
        provenance = write_candidate_provenance(root, sha)

        rejected_diagnostic = run(diagnostic, "--mode", "certification", "--candidate-sha", sha)
        require(
            rejected_diagnostic,
            rejected_diagnostic.returncode == 2 and "sólo diagnósticos" in rejected_diagnostic.stderr,
            "certificación debe rechazar perfiles sólo diagnósticos",
        )
        missing_sha = run(certification, "--mode", "certification")
        require(missing_sha, missing_sha.returncode == 2 and "candidate-sha" in missing_sha.stderr, "certificación requiere SHA candidata")

        cert_data = json.loads(certification.read_text(encoding="utf-8"))
        cert_data["windows"][0]["tolerance"] = {
            "max_changed_pixels": 2_073_600,
            "max_changed_ratio": 1,
            "max_channel_delta": 255,
            "max_mean_channel_delta": 255,
        }
        certification.write_text(json.dumps(cert_data), encoding="utf-8")
        universal = run(certification, "--mode", "certification", "--candidate-sha", sha)
        require(universal, universal.returncode == 2 and "presupuesto V1" in universal.stderr, "certificación debe rechazar tolerancia universal")
        certification = write_certification_manifest(root, sha)

        cert_write = run(
            certification,
            "--mode",
            "certification",
            "--candidate-sha",
            sha,
            "--candidate-provenance",
            str(provenance),
            "--write-sidecars",
        )
        require(cert_write, cert_write.returncode == 0, "no se pudieron generar sidecars de certificación")
        certified = run(certification, "--mode", "certification", "--candidate-sha", sha)
        require(certified, certified.returncode == 0, "captura propia dentro del presupuesto debe certificar")
        report = json.loads(certified.stdout)
        require(
            certified,
            report["assessments"]["artifact_integrity"]["status"] == "pass"
            and report["assessments"]["client_regression"]["status"] == "pass"
            and report["assessments"]["openttd_similarity"]["status"] == "not_applicable",
            "el reporte de certificación debe separar sus tres afirmaciones",
        )
        mismatched_sha = run(certification, "--mode", "certification", "--candidate-sha", "1" * 40)
        require(mismatched_sha, mismatched_sha.returncode == 1 and "candidate_sha" in mismatched_sha.stdout, "SHA candidata distinta debe fallar")

        sidecar_path = root / "artifacts" / "1280x720-1x" / "sidecar.json"
        sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
        sidecar["artifacts"]["candidate"]["path"] = "/tmp/external-candidate.png"
        sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
        external_path = run(certification, "--mode", "certification", "--candidate-sha", sha)
        require(
            external_path,
            external_path.returncode == 1 and "sidecar.artifacts.candidate" in external_path.stdout,
            "un sidecar no puede apuntar fuera del artefacto versionado",
        )

        cert_write = run(
            certification,
            "--mode",
            "certification",
            "--candidate-sha",
            sha,
            "--candidate-provenance",
            str(provenance),
            "--write-sidecars",
        )
        require(cert_write, cert_write.returncode == 0, "no se pudo restaurar la ruta de artefacto")
        sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
        del sidecar["candidate_provenance"]
        sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
        missing_fresh = run(certification, "--mode", "certification", "--candidate-sha", sha)
        require(missing_fresh, missing_fresh.returncode == 1 and "captura fresca" in missing_fresh.stdout, "falta de captura fresca debe fallar")

        cert_write = run(
            certification,
            "--mode",
            "certification",
            "--candidate-sha",
            sha,
            "--candidate-provenance",
            str(provenance),
            "--write-sidecars",
        )
        require(cert_write, cert_write.returncode == 0, "no se pudo restaurar procedencia fresca")
        sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
        del sidecar["candidate_provenance"]["control_assertions"]["goto-button"]
        sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
        absent_control = run(certification, "--mode", "certification", "--candidate-sha", sha)
        require(absent_control, absent_control.returncode == 1 and "control ausente" in absent_control.stdout, "un control requerido ausente debe fallar aparte")

        cert_write = run(
            certification,
            "--mode",
            "certification",
            "--candidate-sha",
            sha,
            "--candidate-provenance",
            str(provenance),
            "--write-sidecars",
        )
        require(cert_write, cert_write.returncode == 0, "no se pudo restaurar assertion de controles")
        pixels = bytearray(visual.read_png(changed).rgba)
        pixels[: 6_000 * 4] = bytes((255, 0, 0, 255)) * 6_000
        visual.write_png(changed, visual.PngImage(1280, 720, bytes(pixels)))
        mutation = run(
            certification,
            "--mode",
            "certification",
            "--candidate-sha",
            sha,
            "--candidate-provenance",
            str(provenance),
            "--write-sidecars",
        )
        require(
            mutation,
            mutation.returncode == 1 and "presupuesto V1" in mutation.stdout,
            "regenerar sidecar no puede legitimar una imagen fuera del presupuesto",
        )

    print("OK: gate visual separa diagnóstico, integridad y certificación V1 con mutaciones detectadas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
