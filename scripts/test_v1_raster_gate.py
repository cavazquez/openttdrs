#!/usr/bin/env python3
"""Bordes reproducibles del presupuesto combinado V1-RAS (#589)."""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
CAPTURE = ROOT / "scripts" / "capture_v1_raster.sh"
sys.path.insert(0, str(ROOT / "scripts"))

import v1_raster_gate
from window_visual_regression import PngImage, write_png


WIDTH = 800
HEIGHT = 600
PIXELS = WIDTH * HEIGHT
SCALES = ((0.25, "In4x", "in4x"), (0.5, "In2x", "in2x"), (2.0, "Out2x", "out2x"), (4.0, "Out4x", "out4x"), (8.0, "Out8x", "out8x"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact(path: Path) -> dict[str, str]:
    return {"path": str(path.resolve()), "sha256": sha256(path)}


def write_fixture(root: Path) -> Path:
    fixture = root / "synthetic-kale-title-game.sav"
    fixture.write_bytes(b"synthetic V1-RAS fixture for gate boundaries\n")
    return fixture


def base_image(width: int = WIDTH, height: int = HEIGHT) -> PngImage:
    return PngImage(width, height, bytes((100, 100, 100, 100)) * (width * height))


def changed_image(
    changed_pixels: int,
    channel_delta: int,
    *,
    extra_first_channel_delta: int = 0,
    width: int = WIDTH,
    height: int = HEIGHT,
) -> PngImage:
    data = bytearray(base_image(width, height).rgba)
    for index in range(changed_pixels):
        offset = index * 4
        for channel in range(4):
            data[offset + channel] += channel_delta
    if extra_first_channel_delta:
        data[0] += extra_first_channel_delta
    return PngImage(width, height, bytes(data))


def write_diagnostics(
    root: Path,
    fixture: Path,
    reference: PngImage,
    candidate: PngImage,
) -> list[Path]:
    reports: list[Path] = []
    for scale, zoom, label in SCALES:
        directory = root / "diagnostics" / label
        directory.mkdir(parents=True)
        reference_path = directory / "reference.png"
        candidate_path = directory / "candidate.png"
        diff_path = directory / "diff.png"
        write_png(reference_path, reference)
        write_png(candidate_path, candidate)
        write_png(diff_path, reference)
        report = {
            "schema_version": 1,
            "kind": "focused-world-screenshot",
            "status": "different",
            "capture": {
                "center": [132, 2],
                "requested_resolution": [WIDTH, HEIGHT],
                "openttd_zoom": zoom,
                "openttdrs_orthographic_scale": scale,
                "profile": "clean-static",
            },
            "alignment": {"search_radius_px": 0, "candidate_translation": [0, 0]},
            "save": artifact(fixture),
            "artifacts": {
                "reference": artifact(reference_path),
                "candidate": artifact(candidate_path),
                "diff": artifact(diff_path),
            },
            "metrics": {"raw": {"changed_pixels": 0}},
        }
        report_path = directory / "report.json"
        report_path.write_text(json.dumps(report), encoding="utf-8")
        reports.append(report_path)
    return reports


def build_artifacts(
    root: Path,
    fixture: Path,
    candidate: PngImage,
    *,
    unstable: bool = False,
) -> list[Path]:
    normal = root / "normal"
    normal.mkdir(parents=True)
    reference = base_image()
    for index in range(1, 4):
        write_png(normal / f"reference-{index}.png", reference)
        image = candidate
        if unstable and index == 2:
            image = changed_image(1, 1, width=candidate.width, height=candidate.height)
        write_png(normal / f"candidate-{index}.png", image)
    return write_diagnostics(root, fixture, reference, candidate)


def run_gate(root: Path, fixture: Path, reports: list[Path]) -> SimpleNamespace:
    reference_asset = root / "reference.obg"
    candidate_asset = root / "candidate.grf"
    mode = root / ".graphics_mode"
    reference_asset.write_bytes(b"reference asset")
    candidate_asset.write_bytes(b"candidate asset")
    mode.write_text("8bpp\n", encoding="utf-8")
    args = [
        "--artifact-dir",
        str(root),
        "--save",
        str(fixture),
        "--candidate-sha",
        "a" * 40,
        "--candidate-mode-file",
        str(mode),
        "--reference-asset",
        str(reference_asset),
        "--candidate-asset",
        str(candidate_asset),
    ]
    for report in reports:
        args.extend(("--diagnostic-report", str(report)))
    original_fixture_sha = v1_raster_gate.FIXTURE_SHA256
    stdout = io.StringIO()
    stderr = io.StringIO()
    try:
        # La constante de producción sigue fijando la fixture canónica. El test
        # unitario usa una fixture mínima temporal para poder correr en CI limpio.
        v1_raster_gate.FIXTURE_SHA256 = sha256(fixture)
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            return_code = v1_raster_gate.main(args)
    finally:
        v1_raster_gate.FIXTURE_SHA256 = original_fixture_sha
    return SimpleNamespace(
        returncode=return_code,
        stdout=stdout.getvalue(),
        stderr=stderr.getvalue(),
    )


class V1RasterGateTest(unittest.TestCase):
    def test_exact_budget_boundaries_pass(self) -> None:
        # 480 * 4 * 50 = 96_000; 96_000 / (800 * 600 * 4) == 0.05 exacto.
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture = write_fixture(root)
            candidate = changed_image(480, 50)
            reports = build_artifacts(root, fixture, candidate)
            completed = run_gate(root, fixture, reports)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            report = json.loads((root / "v1-raster-report.json").read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "passed")
            checks = report["normal"]["checks"]
            self.assertEqual(checks["changed_pixels"]["actual"], 480)
            self.assertEqual(checks["pixels_over_channel_delta_64"]["actual"], 0)
            self.assertEqual(checks["mean_absolute_rgba_channel_delta"]["actual"], 0.05)
            self.assertEqual(checks["outside_candidate_pixels"]["actual"], 0)

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture = write_fixture(root)
            candidate = changed_image(24, 65)
            reports = build_artifacts(root, fixture, candidate)
            completed = run_gate(root, fixture, reports)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            report = json.loads((root / "v1-raster-report.json").read_text(encoding="utf-8"))
            self.assertEqual(report["normal"]["checks"]["pixels_over_channel_delta_64"]["actual"], 24)

    def test_each_budget_rejects_the_next_value(self) -> None:
        cases = (
            ("pixels distintos", changed_image(481, 1)),
            ("delta grande", changed_image(25, 65)),
            ("media RGBA", changed_image(480, 50, extra_first_channel_delta=1)),
        )
        for label, candidate in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                fixture = write_fixture(root)
                reports = build_artifacts(root, fixture, candidate)
                completed = run_gate(root, fixture, reports)
                self.assertEqual(completed.returncode, 1, f"{label}: {completed.stderr}")
                report = json.loads((root / "v1-raster-report.json").read_text(encoding="utf-8"))
                self.assertEqual(report["status"], "failed")

    def test_missing_non_deterministic_or_wrong_geometry_never_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture = write_fixture(root)
            reports = build_artifacts(root, fixture, base_image())
            (root / "normal" / "reference-3.png").unlink()
            completed = run_gate(root, fixture, reports)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("falta captura reference #3", completed.stderr)

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture = write_fixture(root)
            reports = build_artifacts(root, fixture, base_image(), unstable=True)
            completed = run_gate(root, fixture, reports)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("no son hash-idénticas", completed.stderr)

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fixture = write_fixture(root)
            reports = build_artifacts(root, fixture, base_image(799, HEIGHT))
            completed = run_gate(root, fixture, reports)
            self.assertEqual(completed.returncode, 2)
            self.assertIn("geometría", completed.stderr)

    def test_capture_entrypoint_requires_all_fresh_evidence(self) -> None:
        source = CAPTURE.read_text(encoding="utf-8")
        self.assertIn('for index in 1 2 3; do', source)
        self.assertIn('capture_normal "$index"', source)
        for scale, _zoom, label in SCALES:
            self.assertIn(f"capture_diagnostic {scale:g} {label}", source)
        self.assertIn('OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_RADIUS=0', source)
        self.assertIn('OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_STRIDE=1', source)
        self.assertIn('SETTLE_FRAMES="360"', source)
        self.assertIn('OPENTTDRS_WORLD_SCREENSHOT_SETTLE_FRAMES="$SETTLE_FRAMES"', source)
        self.assertIn('git -C "$ROOT" diff --quiet', source)
        self.assertIn('--diagnostic-report "$OUT_DIR/diagnostics/out8x/report.json"', source)
        self.assertNotIn("SKIP", source)


if __name__ == "__main__":
    unittest.main()
