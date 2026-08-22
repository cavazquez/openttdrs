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

    def test_final_sort_stream_keeps_parent_identity_and_legacy_capture(self) -> None:
        header = compact((PATCH / "src" / "world_draw_export.h").read_text(encoding="utf-8"))
        source = compact((PATCH / "src" / "world_draw_export.cpp").read_text(encoding="utf-8"))
        integration = (PATCH / "integrate.sh").read_text(encoding="utf-8")

        self.assertIn("boolOpenttdrsWorldDrawFinalSortRequested();", header)
        self.assertIn("voidOpenttdrsWorldDrawRecordFinalParent(", header)
        self.assertIn("voidOpenttdrsWorldDrawRecordFinalChild(", header)
        self.assertIn('getenv("OPENTTDRS_WORLD_SORT_OUT")', source)
        self.assertIn('"contract","world-sort"', source)
        self.assertIn('row["parent_id"]', source)
        self.assertIn('"stage","post_viewport_sprite_sorter"', source)
        self.assertIn("sortable_final_tail", integration)
        self.assertIn("OpenttdrsWorldDrawFinalSortRequested", integration)
        self.assertIn("ViewportSortParentSprites(&_vd.parent_sprites_to_sort);", integration)


if __name__ == "__main__":
    unittest.main()
