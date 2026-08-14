#!/usr/bin/env python3
"""Impide que el oráculo world-draw vuelva a perder offsets de suelo."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "patches" / "openttd-15.3-snapshot-export"


def compact(text: str) -> str:
    return re.sub(r"\s+", "", text)


class WorldDrawExportSourceContractTest(unittest.TestCase):
    def test_ground_trace_keeps_add_tile_sprite_offsets(self) -> None:
        header = compact((PATCH / "src" / "world_draw_export.h").read_text(encoding="utf-8"))
        source = compact((PATCH / "src" / "world_draw_export.cpp").read_text(encoding="utf-8"))
        integration = (PATCH / "integrate.sh").read_text(encoding="utf-8")

        self.assertIn(
            "OpenttdrsWorldDrawRecordTileSprite("
            "uint32_timage,uint32_tpalette,int32_tx,int32_ty,int32_tz,"
            "int32_toffset_x,int32_toffset_y);",
            header,
        )
        self.assertIn(
            'row["offset"]={{"x",offset_x},{"y",offset_y},{"z",0}};',
            source,
        )
        self.assertIn(
            "OpenttdrsWorldDrawRecordTileSprite("
            "image, pal, x, y, z, extra_offs_x, extra_offs_y);",
            integration,
        )
        self.assertIn("old_tile_trace_call", integration)


if __name__ == "__main__":
    unittest.main()
