#!/usr/bin/env python3
"""Regenerate native emission for Kale's four trees at tile 134,96.

DrawTile_Trees, TreeListEnt and the tree table are unmodified. The companion
C++ adapts tile getters (m3=7, m5=192, m2=48) and sprite/ground sinks. This
checks emission order, not full map decoding, native pixels or other trees.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

from oracle_bridge_pillar_column import PIN, function_body

SOURCES = ["src/tree_cmd.cpp", "src/table/tree_land.h", "src/table/sprites.h",
           "src/tree_map.h"]


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
    table = native["src/table/tree_land.h"].decode()
    header = native["src/table/sprites.h"].decode()
    constants = []
    for name in sorted(set(re.findall(r"\b(?:PALETTE_TO_\w+|PAL_NONE)\b", table))):
        match = re.search(r"\b" + name + r"\s*=\s*(\d+)\s*;", header)
        if match is None:
            raise ValueError("Missing native palette: " + name)
        constants.append("constexpr PaletteID " + name + "=" + match[1] + ";")
    drawing = native["src/tree_cmd.cpp"].decode()
    draw = function_body(drawing, "static void DrawTile_Trees(")
    tree_list = function_body(drawing, "struct TreeListEnt : PalSpriteID") + ";"
    (args.out / "native-palettes.inc").write_text("\n".join(constants) + "\n")
    (args.out / "native-tree-table.h").write_bytes(native["src/table/tree_land.h"])
    (args.out / "native-tree-list.inc").write_text(tree_list)
    (args.out / "native-draw-tree.inc").write_text(draw)
    source = args.out / "oracle.cpp"
    shutil.copyfile(Path(__file__).with_suffix(".cpp"), source)
    binary = args.out / "oracle"
    command = ["c++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror",
               str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    with (args.out / "native-tree-order.csv").open("x") as output:
        subprocess.run([str(binary.resolve())], stdout=output, check=True)
    (args.out / "provenance.json").write_text(json.dumps({
        "native_pin": PIN,
        "native_source_sha256": {name: hashlib.sha256(raw).hexdigest()
                                 for name, raw in native.items()},
        "draw_function_sha256": hashlib.sha256(draw.encode()).hexdigest(),
        "tree_list_sha256": hashlib.sha256(tree_list.encode()).hexdigest(),
        "compiler_command": command,
        "scope": __doc__,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
