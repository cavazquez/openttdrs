#!/usr/bin/env python3
"""Capture native DrawPillarColumn emission with an adapted sprite sink.

This checks bounding boxes and emission order, not the native raster. Function
bodies come unchanged from the pinned OpenTTD checkout; no network is needed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

PIN = "14ec60f248547d4d062a1160f0fc26d742319888"
SOURCES = ["src/tunnelbridge_cmd.cpp", "src/tile_type.h", "src/viewport_type.h",
           "src/table/sprites.h"]
PREAMBLE = r"""#include <cstdint>
#include <iostream>
struct PalSpriteID { uint32_t sprite, pal; };
struct SubSprite {};
struct Vec3 { int x, y, z; };
struct BBox { Vec3 origin, extent, offset; };
constexpr int TILE_HEIGHT=8, BRIDGE_Z_START=3, BB_HEIGHT_UNDER_BRIDGE=6;
constexpr int TO_BRIDGES=0;
bool IsTransparencySet(int) { return false; }
void AddSortableSpriteToDraw(uint32_t sprite, uint32_t, int x, int y, int z,
                             BBox b, bool, const SubSprite *) {
 std::cout << sprite << ',' << x+b.origin.x << ',' << y+b.origin.y << ','
           << z+b.origin.z << ',' << x+b.origin.x+b.extent.x-1 << ','
           << y+b.origin.y+b.extent.y-1 << ',' << z+b.origin.z+b.extent.z-1 << '\n';
}
"""
MAIN = r"""
int main() {
 std::cout << "sprite_id,xmin,ymin,zmin,xmax,ymax,zmax\n";
 DrawPillarColumn(0,21,PalSpriteID{2567,0},1996,2384,2,16);
 DrawPillarColumn(0,21,PalSpriteID{2566,0},2384,1996,16,2);
}
"""


def function_body(source, signature):
    start = source.index(signature)
    opening = source.index("{", start)
    nesting = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            nesting += 1
        elif source[index] == "}":
            nesting -= 1
            if nesting == 0:
                return source[start:index + 1]
    raise ValueError("Unclosed native function: " + signature)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--openttd", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    native = {}
    for name in SOURCES:
        raw = subprocess.check_output(["git", "show", PIN + ":" + name], cwd=args.openttd)
        (args.out / ("native-" + name.replace("/", "__"))).write_bytes(raw)
        native[name] = raw
    constants = [
        ("src/tile_type.h", "TILE_HEIGHT", 8),
        ("src/tunnelbridge_cmd.cpp", "BRIDGE_Z_START", 3),
        ("src/viewport_type.h", "BB_HEIGHT_UNDER_BRIDGE", 6),
        ("src/table/sprites.h", "SPR_BTTUB_X_PILLAR_MID", 2566),
        ("src/table/sprites.h", "SPR_BTTUB_Y_PILLAR_MID", 2567),
    ]
    for name, constant, expected in constants:
        match = re.search(r"\b" + constant + r"\s*=\s*(\d+)\s*;", native[name].decode())
        if match is None or int(match[1]) != expected:
            raise ValueError("Native constant changed: " + constant)
    drawing = native["src/tunnelbridge_cmd.cpp"].decode()
    functions = [function_body(drawing, signature) for signature in [
        "static inline void DrawPillar(", "static int DrawPillarColumn(",
    ]]
    source = args.out / "oracle.cpp"
    source.write_text(PREAMBLE + "\n".join(functions) + MAIN)
    binary = args.out / "oracle"
    command = ["c++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror",
               str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    with (args.out / "native-pillar-column.csv").open("x") as output:
        subprocess.run([str(binary.resolve())], stdout=output, check=True)
    (args.out / "provenance.json").write_text(json.dumps({
        "native_pin": PIN,
        "native_source_sha256": {name: hashlib.sha256(raw).hexdigest()
                                 for name, raw in native.items()},
        "native_function_sha256": [hashlib.sha256(body.encode()).hexdigest()
                                   for body in functions],
        "compiler_command": command,
        "scope": "Unchanged DrawPillar/DrawPillarColumn bodies; adapted bbox sink, no raster",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
