#!/usr/bin/env python3
"""Smoke gráfico nativo de un paquete extraído para Windows y macOS (#601, #602).

El binario se ejecuta desde un cwd temporal, con un perfil nuevo y la raíz de
assets fijada al paquete extraído. No crea un display virtual ni tiene una ruta
headless: si el runner no puede abrir una sesión gráfica real, el proceso o la
captura fallan y se conserva esa evidencia.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from check_release_graphical_smoke import SmokeCaptureError, read_png, require_visible_frame
from window_visual_regression import GateError


SHA_RE = re.compile(r"[0-9a-f]{40}\Z")
MENU_MARKER = "main_menu_shot: menú localizado sin escenario guiado listo"
DEFAULT_LANGUAGES = ("es", "en")
MACOS_FULLSCREEN_CAPTURE = "OPENTTDRS_CAPTURE_BORDERLESS_FULLSCREEN"


class GraphicalSmokeError(RuntimeError):
    """A package did not demonstrate a usable graphical menu."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def portable_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return str(path.resolve())


def text_output(value: str | bytes | None) -> str:
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value or ""


def runner_metadata() -> dict[str, Any]:
    environment = {}
    for key in (
        "GITHUB_ACTIONS",
        "GITHUB_SHA",
        "RUNNER_OS",
        "RUNNER_ARCH",
        "RUNNER_NAME",
        "ImageOS",
        "ImageVersion",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "SESSIONNAME",
        "WGPU_BACKEND",
    ):
        value = os.environ.get(key)
        if value:
            environment[key] = value
    return {
        "system": platform.platform(),
        "machine": platform.machine(),
        "python": sys.version.split()[0],
        "driver": {
            "mode": "native runner session; sin Xvfb, offscreen ni fallback headless",
            "requested_wgpu_backend": os.environ.get("WGPU_BACKEND"),
        },
        "environment": environment,
    }


def launch_environment(package_root: Path, profile: Path, language: str, screenshot: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for key in (
        "OPENTTDRS_ASSET_ROOT",
        "OPENTTDRS_DISABLE_AUDIO",
        "OPENTTDRS_LANGUAGE",
        "OPENTTDRS_MAIN_MENU_SHOT",
        "OPENTTDRS_SHOT_RES",
        MACOS_FULLSCREEN_CAPTURE,
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
    ):
        environment.pop(key, None)
    environment.update(
        {
            "OPENTTDRS_ASSET_ROOT": str(package_root),
            "OPENTTDRS_DISABLE_AUDIO": "1",
            "OPENTTDRS_LANGUAGE": language,
            "OPENTTDRS_MAIN_MENU_SHOT": str(screenshot),
            "OPENTTDRS_SHOT_RES": "1280x720",
            "RUST_BACKTRACE": "1",
            "RUST_LOG": "info",
            "HOME": str(profile / "home"),
            "USERPROFILE": str(profile / "home"),
            "APPDATA": str(profile / "config"),
            "LOCALAPPDATA": str(profile / "data"),
            "XDG_CONFIG_HOME": str(profile / "config"),
            "XDG_DATA_HOME": str(profile / "data"),
            "XDG_STATE_HOME": str(profile / "state"),
            "XDG_CACHE_HOME": str(profile / "cache"),
            "XDG_RUNTIME_DIR": str(profile / "runtime"),
        }
    )
    # El escritorio hosted de macOS deja un área útil de 1280x653 cuando se
    # solicita una ventana decorada de 1280x720. La captura debe usar la
    # sesión nativa completa, no recortar ni relajar el contrato del PNG.
    if platform.system() == "Darwin":
        environment[MACOS_FULLSCREEN_CAPTURE] = "1"
    return environment


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--client", required=True, type=Path)
    result.add_argument("--package-root", required=True, type=Path)
    result.add_argument("--archive", required=True, type=Path)
    result.add_argument("--candidate-sha", required=True)
    result.add_argument("--artifact-dir", required=True, type=Path)
    result.add_argument("--timeout-seconds", type=int, default=60)
    result.add_argument("--language", action="append", choices=DEFAULT_LANGUAGES)
    return result


def validate_arguments(args: argparse.Namespace, argument_parser: argparse.ArgumentParser) -> tuple[str, tuple[str, ...]]:
    candidate_sha = args.candidate_sha.lower()
    if not SHA_RE.fullmatch(candidate_sha):
        argument_parser.error("--candidate-sha debe tener 40 hexadecimales")
    if args.timeout_seconds <= 0:
        argument_parser.error("--timeout-seconds debe ser positivo")
    if not args.client.is_file():
        argument_parser.error(f"no existe el cliente empaquetado: {args.client}")
    if not args.archive.is_file():
        argument_parser.error(f"no existe el archivo del paquete: {args.archive}")
    if not args.package_root.is_dir():
        argument_parser.error(f"no existe la raíz del paquete: {args.package_root}")
    if not (args.package_root / "assets").is_dir():
        argument_parser.error("la raíz del paquete no contiene assets/")
    if args.artifact_dir.exists() and any(args.artifact_dir.iterdir()):
        argument_parser.error("--artifact-dir debe ser nuevo o estar vacío")
    args.artifact_dir.mkdir(parents=True, exist_ok=True)
    languages = tuple(args.language or DEFAULT_LANGUAGES)
    if len(set(languages)) != len(languages):
        argument_parser.error("cada --language sólo puede aparecer una vez")
    return candidate_sha, languages


def write_report(path: Path, report: dict[str, Any]) -> None:
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def capture_language(
    client: Path,
    package_root: Path,
    cwd: Path,
    profile: Path,
    artifact_dir: Path,
    language: str,
    timeout_seconds: int,
) -> dict[str, Any]:
    screenshot = artifact_dir / f"menu-{language}.png"
    log = artifact_dir / f"menu-{language}.log"
    started = time.monotonic()
    entry: dict[str, Any] = {
        "language": language,
        "command": [str(client)],
        "log": log.name,
        "screenshot": screenshot.name,
        "status": "failed",
    }
    try:
        completed = subprocess.run(
            [str(client)],
            cwd=cwd,
            env=launch_environment(package_root, profile, language, screenshot),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout_seconds,
            check=False,
        )
        output = text_output(completed.stdout)
        log.write_text(output, encoding="utf-8")
        entry["exit_code"] = completed.returncode
        if completed.returncode != 0:
            raise GraphicalSmokeError(f"el cliente terminó con código {completed.returncode}")
        if MENU_MARKER not in output:
            raise GraphicalSmokeError("el cliente no confirmó que compuso el menú localizado")
        if not screenshot.is_file():
            raise GraphicalSmokeError("el cliente no produjo la captura del menú")
        image = read_png(screenshot)
        stats = require_visible_frame("menú", image, 1280, 720)
        entry["frame"] = {
            "width": image.width,
            "height": image.height,
            "sha256": sha256(screenshot),
            "opaque_pixels": stats.opaque_pixels,
            "varied_pixels": stats.varied_pixels,
            "distinct_colours": stats.distinct_colours,
        }
        entry["status"] = "passed"
    except subprocess.TimeoutExpired as error:
        output = text_output(error.stdout) + text_output(error.stderr)
        log.write_text(output, encoding="utf-8")
        entry["error"] = f"timeout después de {timeout_seconds} s"
    except (GateError, GraphicalSmokeError, OSError, SmokeCaptureError) as error:
        entry["error"] = str(error)
    finally:
        entry["duration_seconds"] = round(time.monotonic() - started, 3)
    return entry


def main(argv: list[str] | None = None) -> int:
    argument_parser = parser()
    args = argument_parser.parse_args(argv)
    candidate_sha, languages = validate_arguments(args, argument_parser)
    client = args.client.resolve()
    package_root = args.package_root.resolve()
    archive = args.archive.resolve()
    report_path = args.artifact_dir / "graphical-smoke.json"
    report: dict[str, Any] = {
        "schema_version": 1,
        "kind": "release-native-graphical-smoke",
        "status": "failed",
        "candidate_sha": candidate_sha,
        "package": {
            "archive": portable_path(archive),
            "sha256": sha256(archive),
            "asset_root": portable_path(package_root),
            "client": portable_path(client),
        },
        "runner": runner_metadata(),
        "languages": [],
    }

    try:
        with tempfile.TemporaryDirectory(prefix="openttdrs-native-graphical-") as temp:
            isolated = Path(temp)
            cwd = isolated / "cwd"
            profile = isolated / "profile"
            cwd.mkdir()
            for name in ("home", "config", "data", "state", "cache", "runtime"):
                (profile / name).mkdir(parents=True)
            for language in languages:
                report["languages"].append(
                    capture_language(
                        client,
                        package_root,
                        cwd,
                        profile,
                        args.artifact_dir,
                        language,
                        args.timeout_seconds,
                    )
                )
    except OSError as error:
        report["error"] = str(error)

    if report["languages"] and all(entry["status"] == "passed" for entry in report["languages"]):
        report["status"] = "passed"
    else:
        report["status"] = "failed"
    write_report(report_path, report)
    print(
        f"Smoke gráfico nativo: status={report['status']} "
        f"languages={len(report['languages'])} evidence={report_path}"
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
