#!/usr/bin/env python3
"""Run native ChimneySmokeTick at a cycle boundary for all eight sprite phases.

Progress starts at 7, as set by the preceding native sprite advance.
Tile lifetime and viewport notifications are adapted; tick and IncrementSprite
bodies are unchanged. This does not cover RNG initialization, effect import,
native palette decoding or the complete lifecycle of industrial effects.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

from oracle_bridge_pillar_column import PIN, function_body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openttd", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sources = {}
    for name in ["src/effectvehicle.cpp", "src/table/sprites.h"]:
        raw = subprocess.check_output(["git", "show", PIN + ":" + name], cwd=args.openttd)
        (args.out / ("native-" + name.replace("/", "__"))).write_bytes(raw)
        sources[name] = raw
    functions = {}
    for label, signature in [
        ("increment", "static bool IncrementSprite("),
        ("chimney-tick", "static bool ChimneySmokeTick("),
    ]:
        body = function_body(sources["src/effectvehicle.cpp"].decode(), signature)
        (args.out / ("native-" + label + ".inc")).write_text(body + "\n")
        functions[label] = hashlib.sha256(body.encode()).hexdigest()
    constants = []
    for name, expected in [("SPR_CHIMNEY_SMOKE_0", 3701), ("SPR_CHIMNEY_SMOKE_7", 3708)]:
        match = re.search(r"\b" + name + r"\s*=\s*(\d+)\s*;", sources["src/table/sprites.h"].decode())
        if match is None or int(match[1]) != expected:
            raise ValueError("native sprite constant changed: " + name)
        constants.append("constexpr SpriteID " + name + "=" + match[1] + ";")
    (args.out / "native-constants.inc").write_text("\n".join(constants) + "\n")
    cpp = args.out / "oracle.cpp"
    shutil.copyfile(Path(__file__).with_suffix(".cpp"), cpp)
    binary = args.out / "oracle"
    command = ["c++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror", str(cpp), "-o", str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    (args.out / "native-chimney-cadence.csv").write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / "crates/openttdrs-client/tests/fixtures/native-chimney-cadence.csv"
        if output != fixture.read_text():
            raise ValueError("native output differs from committed fixture")
    (args.out / "provenance.json").write_text(json.dumps({
        "native_pin": PIN,
        "sources": {name: hashlib.sha256(raw).hexdigest() for name, raw in sources.items()},
        "unmodified_function_sha256": functions,
        "command": command,
        "rows": len(output.splitlines()) - 1,
        "scope": __doc__,
    }, indent=2) + "\n")
    print("Native ChimneySmokeTick: 520 rows for phases 0..7, ticks 0..64")


if __name__ == "__main__":
    main()
