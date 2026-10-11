#!/usr/bin/env python3
"""Run the isolated Kale bus cycle with a following camera at all six zooms.

Requires Xvfb, Vulkan and ffmpeg. Saves a video at scale 1, CSV traces and a
fail-closed report. This checks a playable route, not native tick or FPS parity.
"""

import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SCALES = ("0.25", "0.5", "1", "2", "4", "8")
HOME, DESTINATION, DEPOT = (116, 75), (117, 51), (118, 78)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def evaluate(path, scale, code):
    with path.open() as stream:
        rows = list(csv.DictReader(stream))
    if not rows:
        return {"scale": scale, "exit": code, "passed": False, "failure": "empty trace"}
    visible = [r for r in rows if math.isfinite(float(r["sprite_x"]))]
    mismatches = [r for r in visible if
                  abs(float(r["camera_x"]) - float(r["sprite_x"])) > 0.001 or
                  abs(float(r["camera_y"]) - float(r["sprite_y"])) > 0.001]
    destination = depot = returned = None
    for r in rows:
        pos = int(r["x"]), int(r["y"])
        at_stop = r["loading"] == "true" and int(r["road_frame"]) == 11
        last_station = int(r["last_station_x"]), int(r["last_station_y"])
        # A render frame at speed 8 may cover the entire brief service. The
        # last-station marker is set by BeginLoading, not by tile entry.
        if destination is None and ((pos == DESTINATION and at_stop) or last_station == DESTINATION):
            destination = int(r["tick"])
        elif destination is not None and depot is None and pos == DEPOT:
            depot = int(r["tick"])
        elif depot is not None and returned is None and ((pos == HOME and at_stop) or last_station == HOME):
            returned = int(r["tick"])
    observed_scales = sorted({float(r["scale"]) for r in rows[60:]})
    income = int(rows[-1]["cargo_income"]) - int(rows[0]["cargo_income"])
    delivered = int(rows[-1]["delivered_units"]) - int(rows[0]["delivered_units"])
    result = {
        "scale": scale, "exit": code, "frames": len(rows),
        "visible_frames": len(visible), "camera_mismatches": len(mismatches),
        "ticks_advanced": int(rows[-1]["tick"]) - int(rows[0]["tick"]),
        "visited_tiles": len({(r["x"], r["y"]) for r in rows}),
        "directions": sorted({int(r["direction"]) for r in rows}),
        "observed_scales": observed_scales, "destination_tick": destination,
        "depot_tick": depot, "returned_home_tick": returned,
        "cargo_income": income, "delivered_units": delivered,
        "trace_sha256": sha256(path),
    }
    result["passed"] = (code == 0 and len(visible) > 50 and not mismatches
                        and observed_scales == [float(scale)] and returned is not None
                        and income > 0 and delivered > 0
                        and all(r["kind"] == "Bus" and r["vehicle_id"] == "1888" for r in rows))
    return result


def run_case(binary, game, out, display, scale, frames, speed, software):
    case = out / scale
    case.mkdir()
    profile = case / "profile"
    for directory in ("config", "data", "cache", "state", "runtime"):
        (profile / directory).mkdir(parents=True)
    (profile / "runtime").chmod(0o700)
    settings = profile / "config/com.github.cavazquez.openttdrs/settings.toml"
    settings.parent.mkdir()
    settings.write_text(f"[client_preferences]\ndefault_sim_speed = {speed}\n"
                        "window_width = 1280\nwindow_height = 720\nminimap_visible = false\n")
    env = {k: v for k, v in os.environ.items() if not k.startswith("OPENTTDRS_")
           and k not in ("OTTDJSON_LOAD", "OTTDMAP_FILE", "WAYLAND_DISPLAY", "VK_ICD_FILENAMES")}
    env.update({
        "DISPLAY": display, "WINIT_UNIX_BACKEND": "x11", "WGPU_BACKEND": "vulkan",
        "OPENTTDRS_ASSET_ROOT": str(ROOT), "OTTDJSON_LOAD": str(game),
        "OPENTTDRS_DISABLE_AUDIO": "1", "OPENTTDRS_FOLLOW_VEHICLE": "1888",
        "OPENTTDRS_FOLLOW_TRACE_OUT": str(case / "follow.csv"),
        "OPENTTDRS_PERF_OUT": str(case / "frames.csv"),
        "OPENTTDRS_PERF_FRAMES": str(frames), "OPENTTDRS_PERF_WARMUP": "60",
        "OPENTTDRS_PERF_SCALE": scale, "OPENTTDRS_SHOT_RES": "1280x720",
        "RUST_LOG": "info", "RUST_BACKTRACE": "1",
    })
    if software:
        env["VK_ICD_FILENAMES"] = "/usr/share/vulkan/icd.d/lvp_icd.json"
    env.update({f"XDG_{key}_HOME": str(profile / directory) for key, directory in
                (("CONFIG", "config"), ("DATA", "data"), ("CACHE", "cache"), ("STATE", "state"))})
    env["XDG_RUNTIME_DIR"] = str(profile / "runtime")
    video = None
    with (case / "client.log").open("wb") as log, (case / "ffmpeg.log").open("wb") as video_log:
        process = subprocess.Popen([str(binary)], cwd=ROOT, env=env, stdout=log, stderr=log)
        try:
            if scale == "1":
                video = subprocess.Popen([
                    "ffmpeg", "-y", "-f", "x11grab", "-draw_mouse", "0", "-framerate", "30", "-video_size", "1280x720",
                    "-i", display, "-c:v", "libx264", "-preset", "veryfast", "-crf", "23",
                    "-pix_fmt", "yuv420p", str(case / "follow.mp4"),
                ], stdout=video_log, stderr=video_log)
            code = process.wait(timeout=240)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            if video:
                video.terminate()
                video.wait(timeout=15)
    result = evaluate(case / "follow.csv", scale, code)
    if video:
        probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                                "-show_entries", "stream=width,height", "-of", "json",
                                str(case / "follow.mp4")], capture_output=True, text=True, check=True)
        result["video_valid"] = bool(json.loads(probe.stdout)["streams"])
        result["video_sha256"] = sha256(case / "follow.mp4")
        result["passed"] = result["passed"] and result["video_valid"]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/openttdrs-client")
    parser.add_argument("--game", type=Path, default=ROOT / "save/Kale_bus_follow.json")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--scales", nargs="+", choices=SCALES, default=list(SCALES))
    parser.add_argument("--frames", type=int, default=500,
                        help="Measured render frames; the full cycle remains mandatory")
    parser.add_argument("--speed", type=float, default=8.0)
    parser.add_argument("--software", action="store_true", help="Use Lavapipe instead of the GPU")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {"passed": False, "bus": 1888, "binary_sha256": sha256(args.binary.resolve()),
              "game_sha256": sha256(args.game.resolve()), "frames_requested": args.frames,
              "speed": args.speed, "zooms": [], "scope": "isolated bus cycle and camera; no FPS/native tick certification"}
    verdict = out / "results.json"
    verdict.write_text(json.dumps(report, indent=2) + "\n")
    read_fd, write_fd = os.pipe()
    with (out / "xvfb.log").open("wb") as log:
        xvfb = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1280x720x24",
                                 "-nolisten", "tcp"], pass_fds=(write_fd,), stdout=log, stderr=log)
    os.close(write_fd)
    try:
        # Xvfb can write the number and newline separately. Closing the pipe
        # after a partial read makes the server fail its remaining write.
        with os.fdopen(read_fd) as ready:
            display_number = ready.readline().strip()
        if not display_number.isdecimal():
            raise RuntimeError("Xvfb did not publish a valid display number")
        display = ":" + display_number
        for scale in args.scales:
            result = run_case(args.binary.resolve(), args.game.resolve(), out, display, scale,
                              args.frames, args.speed, args.software)
            report["zooms"].append(result)
            verdict.write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(result), flush=True)
            if not result["passed"]:
                raise RuntimeError("The bus route/camera check failed; inspect results.json")
        report["passed"] = True
        verdict.write_text(json.dumps(report, indent=2) + "\n")
    finally:
        xvfb.terminate()
        xvfb.wait(timeout=10)


if __name__ == "__main__":
    main()
