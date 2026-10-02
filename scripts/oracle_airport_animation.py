#!/usr/bin/env python3
"""Run the pristine OpenTTD airport animation template with a tiny world adapter.

Usage: oracle_airport_animation.py --openttd reference/openttd-15.3-oracle --out DIR
The CSV checks default frame/dirty cadence only; it does not load native saves
or execute NewGRF callbacks. The companion C++ source adapts world interfaces.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

PIN = "14ec60f248547d4d062a1160f0fc26d742319888"
NATIVE = ["src/newgrf_animation_base.h", "src/newgrf_animation_type.h",
          "src/table/airporttiles.h"]
STUBS = ["animated_tile_func.h", "core/random_func.hpp", "timer/timer_game_tick.h",
         "viewport_func.h", "newgrf_callbacks.h", "tile_map.h", "table/strings.h"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openttd", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    include = args.out / "oracle-include"
    include.mkdir()
    hashes = {}
    for name in NATIVE:
        raw = subprocess.check_output(["git", "show", PIN + ":" + name], cwd=args.openttd)
        (args.out / ("native-" + name.replace("/", "__"))).write_bytes(raw)
        hashes[name] = hashlib.sha256(raw).hexdigest()
        if name.endswith("newgrf_animation_type.h"):
            (include / "newgrf_animation_type.h").write_bytes(raw)
    for name in STUBS:
        dest = include / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text("// World interface supplied by oracle_airport_animation.cpp.\n")
    source = args.out / "oracle.cpp"
    shutil.copyfile(Path(__file__).with_suffix(".cpp"), source)
    binary = args.out / "oracle"
    command = ["c++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror",
               "-I" + str(include), str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    with (args.out / "native-cadence.csv").open("x") as output:
        subprocess.run([str(binary.resolve())], stdout=output, check=True)
    (args.out / "provenance.json").write_text(json.dumps({
        "native_pin": PIN, "native_source_sha256": hashes,
        "adapter_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "compiler_command": command,
        "scope": "Default animation frame and dirty cadence; adapted world interfaces",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
