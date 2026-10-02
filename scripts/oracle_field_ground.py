#!/usr/bin/env python3
"""Execute unchanged native DrawTile_Clear for nine crop stages and 32 slope values.

Ground sprite IDs and the fence call are observed. Other ground branches and
the actual fence drawing are stubbed and never exercised. Bridge drawing is
also stubbed; bridge geometry is outside this oracle. This does not verify
sprite decoding, palettes, effect initialization, GPU ordering or save import.
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
    for name in ["src/clear_cmd.cpp", "src/table/clear_land.h", "src/landscape.cpp",
                 "src/slope_func.h", "src/table/sprites.h"]:
        raw = subprocess.check_output(["git", "show", PIN + ":" + name], cwd=args.openttd)
        (args.out / ("native-" + name.replace("/", "__"))).write_bytes(raw)
        sources[name] = raw.decode()
    bodies = {}
    for label, source, signature in [
        ("draw-clear", "src/clear_cmd.cpp", "static void DrawTile_Clear("),
        ("slope-offset", "src/slope_func.h", "inline uint SlopeToSpriteOffset("),
    ]:
        body = function_body(sources[source], signature)
        (args.out / ("native-" + label + ".inc")).write_text(body + "\n")
        bodies[label] = hashlib.sha256(body.encode()).hexdigest()
    for label, source, pattern in [
        ("farmland", "src/table/clear_land.h", r"static const SpriteID _clear_land_sprites_farmland\[16\]\s*=\s*\{.*?\};"),
        ("slope-table", "src/landscape.cpp", r"extern const uint8_t _slope_to_sprite_offset\[32\]\s*=\s*\{.*?\};"),
    ]:
        match = re.search(pattern, sources[source], re.S)
        if match is None:
            raise ValueError("native table missing: " + label)
        (args.out / ("native-" + label + ".inc")).write_text(match[0] + "\n")
        bodies[label] = hashlib.sha256(match[0].encode()).hexdigest()
    names = ["SPR_FARMLAND_BARE", *["SPR_FARMLAND_STATE_" + str(i) for i in range(1, 8)],
             "SPR_FARMLAND_HAYPACKS", "SPR_FLAT_ROCKY_LAND_1", "SPR_FLAT_ROCKY_LAND_2"]
    constants = []
    for name in names:
        match = re.search(r"\b" + name + r"\s*=\s*(\d+)\s*;", sources["src/table/sprites.h"])
        if match is None:
            raise ValueError("native constant missing: " + name)
        constants.append("constexpr SpriteID " + name + "=" + match[1] + ";")
    (args.out / "native-constants.inc").write_text("\n".join(constants) + "\n")
    cpp = args.out / "oracle.cpp"
    shutil.copyfile(Path(__file__).with_suffix(".cpp"), cpp)
    binary = args.out / "oracle"
    command = ["c++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror", str(cpp), "-o", str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 289
    (args.out / "native-field-ground.csv").write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / "crates/openttdrs-client/tests/fixtures/native-field-ground.csv"
        if output != fixture.read_text():
            raise ValueError("native output differs from fixture")
    (args.out / "provenance.json").write_text(json.dumps({
        "native_pin": PIN,
        "sources": {name: hashlib.sha256(raw.encode()).hexdigest() for name, raw in sources.items()},
        "unmodified_body_sha256": bodies,
        "command": command,
        "rows": len(output.splitlines()) - 1,
        "scope": __doc__,
    }, indent=2) + "\n")
    print("Native DrawTile_Clear: 288 field ground states, one fence call each")


if __name__ == "__main__":
    main()
