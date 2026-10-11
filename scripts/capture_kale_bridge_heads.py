#!/usr/bin/env python3
"""Capture Kale's bridge connections at six zooms with a fixed, flat camera centre."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

from window_visual_regression import read_png

ROOT = Path(__file__).resolve().parents[1]
SCALES = ("0.25", "0.5", "1", "2", "4", "8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--scales", nargs="+", choices=SCALES, default=SCALES)
    parser.add_argument("--software", action="store_true")
    parser.add_argument("--trace", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    sav = ROOT / "save/Kale_TitleGame.sav"
    report = {
        "passed": False, "center": [204, 117], "resolution": [800, 600],
        "settle_frames": 40, "paused": True, "native_zoom": True,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "sav_sha256": hashlib.sha256(sav.read_bytes()).hexdigest(), "cases": [],
    }
    receipt = out / "captures.json"
    receipt.write_text(json.dumps(report, indent=2) + "\n")
    for scale in args.scales:
        case = out / scale
        case.mkdir()
        profile = case / "profile"
        for folder in ("config", "data", "state", "cache", "runtime"):
            (profile / folder).mkdir(parents=True)
        (profile / "runtime").chmod(0o700)
        env = {k: v for k, v in os.environ.items() if not k.startswith("OPENTTDRS_")}
        env.pop("WAYLAND_DISPLAY", None)
        env.update({
            "OPENTTDRS_ASSET_ROOT": str(ROOT),
            "OPENTTDRS_SAV_LOAD": str(sav),
            "OPENTTDRS_MAP_SHOT": str(case / "world.png"),
            "OPENTTDRS_MAP_SHOT_CENTER": "204,117",
            "OPENTTDRS_MAP_SHOT_SCALE": scale,
            "OPENTTDRS_MAP_SHOT_SETTLE_FRAMES": "40",
            "OPENTTDRS_MAP_SHOT_CLEAN": "1",
            "OPENTTDRS_MAP_SHOT_NATIVE_ZOOM": "1",
            "OPENTTDRS_DISABLE_AUDIO": "1",
            "OPENTTDRS_SHOT_RES": "800x600",
            "XDG_CONFIG_HOME": str(profile / "config"),
            "XDG_DATA_HOME": str(profile / "data"),
            "XDG_STATE_HOME": str(profile / "state"),
            "XDG_CACHE_HOME": str(profile / "cache"),
            "WINIT_UNIX_BACKEND": "x11", "WGPU_BACKEND": "vulkan",
        })
        command = [str(binary)]
        if args.trace:
            env["OPENTTDRS_MAP_SPRITE_TRACE_OUT"] = str(case / "sprites.json")
        if args.software:
            env["XDG_RUNTIME_DIR"] = str(profile / "runtime")
            env["VK_ICD_FILENAMES"] = "/usr/share/vulkan/icd.d/lvp_icd.json"
            command = ["xvfb-run", "-a", "-s", "-screen 0 800x600x24", *command]
        with (case / "client.log").open("wb") as log:
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=log,
                                    stderr=subprocess.STDOUT, timeout=600)
        png = case / "world.png"
        passed = result.returncode == 0 and png.is_file()
        if passed:
            img = read_png(png)
            passed = (img.width, img.height) == (800, 600)
        row = {"scale": scale, "exit": result.returncode, "passed": passed,
               "png_sha256": hashlib.sha256(png.read_bytes()).hexdigest() if png.is_file() else None}
        report["cases"].append(row)
        receipt.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(row), flush=True)
        if not passed:
            raise RuntimeError(f"Incomplete capture: {case / 'client.log'}")
    report["passed"] = True
    receipt.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
