#!/usr/bin/env python3
"""Check the captured pair against loaded native roots and prepared zoom banks.

Native inputs are sprite-1163.indices and sprite-1167.indices dumped from
8bpp-simple after loading Kale, plus its freeze log. This verifies alpha and
bank generation, not viewport placement, native palette or all sprite IDs.
"""
from __future__ import annotations

import argparse
import csv
import gzip
import hashlib
import json
import re
from pathlib import Path

from PIL import Image

from check_frozen_screenshot_state import check_state


ROOT = Path(__file__).resolve().parents[1]


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify(directory: Path, trace_directory: Path) -> list[dict]:
    state = check_state((directory / "native-dump.log").read_text())
    trace = json.loads(gzip.decompress((trace_directory / "zoom-.5-before.json.gz").read_bytes()))
    rects = trace["assets"]["layouts"][0]["textures"]
    normal = Image.open(ROOT / "assets/opengfx/atlas/tiles_atlas_0.png").convert("RGBA")
    rows = []
    log = (directory / "native-dump.log").read_text()
    for name, sprite_id, atlas_index, offset in (
        ("parent", 1163, 281, (-124, -80)),
        ("child", 1167, 1258, (0, 0)),
    ):
        metadata = re.search(
            rf"mask-sprite-dump: id={sprite_id} width=(\d+) height=(\d+) x_offs=(-?\d+) y_offs=(-?\d+) ok=1",
            log,
        )
        assert metadata is not None, "successful native asset dump required"
        assert tuple(map(int, metadata.groups())) == (264, 208, *offset)
        native = (directory / f"native-sprites/sprite-{sprite_id}.indices").read_bytes()
        assert len(native) == 264 * 208
        captured = (trace_directory / f"{name}.rgba").read_bytes()
        assert len(captured) == 66 * 52 * 4
        rectangle = rects[atlas_index]
        source = normal.crop(rectangle)
        assert source.size == (66, 52)
        source_rgba = source.tobytes()
        assert source_rgba[3::4] == captured[3::4]
        expected = bytes(captured[((y // 4) * 66 + x // 4) * 4 + 3] != 0
                         for y in range(208) for x in range(264))
        assert bytes(value != 0 for value in native) == expected
        for factor in (1, 2, 4, 8):
            suffix = "" if factor == 1 else f"_out{factor}"
            path = ROOT / f"assets/opengfx/atlas/tiles_atlas_0{suffix}.png"
            actual = Image.open(path).convert("RGBA").crop(rectangle).tobytes()
            expected_rgba = b"".join(
                source_rgba[((y // factor * factor) * 66 + x // factor * factor) * 4:
                            ((y // factor * factor) * 66 + x // factor * factor) * 4 + 4]
                for y in range(52) for x in range(66)
            )
            assert actual == expected_rgba
            rows.append({"source": name, "sprite_id": sprite_id, "atlas_index": atlas_index,
                         "factor": factor, "tick": state["tick"], "native_root_bytes": len(native),
                         "native_root_sha256": sha(native), "native_offsets": str(offset),
                         "normal_alpha_equals_captured": True, "native_alpha_equals_expanded_normal": True,
                         "bank_rgba_equals_replicated_source_stride": True,
                         "bank_crop_sha256": sha(actual), "bank_crop_opaque": sum(a != 0 for a in actual[3::4]),
                         "scope": "loaded native roots and versioned bank crops; no viewport placement or full-scene parity"})
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--trace-directory", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    rows = verify(args.directory, args.trace_directory)
    with args.out.open("x", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    print(json.dumps({"verified": True, "sources": 2, "bank_crops": len(rows),
                      "tick": rows[0]["tick"], "scope": __doc__}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
