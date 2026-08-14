#!/usr/bin/env python3
"""Regresión de las capas de depósito vial en OpenGFX 8bpp y OpenGFX2."""
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_road_depot_gfx_data as generator
from nfo_sprite_meta import parse_sprite_offs


LAYER_BLOCKS = {
    "ne": [(0x584, 0, 15, 0, 16, 1)],
    "se": [(0x580, 0, 0, 0, 1, 16), (0x581, 15, 0, 0, 1, 16)],
    "sw": [(0x582, 0, 0, 0, 16, 1), (0x583, 0, 15, 0, 16, 1)],
    "nw": [(0x585, 15, 0, 0, 1, 16)],
}
SPRITE_IDS = (1412, 1408, 1409, 1410, 1411, 1413)
PNG_BY_ID = {
    1412: "rail_1412.png",
    1408: "road_depot_0.png",
    1409: "road_depot_1.png",
    1410: "road_depot_2.png",
    1411: "road_depot_3.png",
    1413: "rail_1413.png",
}


def nfo_rows(mode: str) -> str:
    lines: list[str] = []
    for index, sprite_id in enumerate(SPRITE_IDS):
        lines.append(
            f"{sprite_id} base.png 8bpp {index * 2} 0 2 1 {-index - 1} {-index - 2} normal\n"
        )
        if mode == "32bpp":
            lines.append(
                f"    | base.32.png 32bpp {index * 3} 0 3 2 {-index - 11} {-index - 12} normal\n"
            )
    return "".join(lines)


class RoadDepotSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        opengfx = repo / "assets/opengfx"
        opengfx.mkdir(parents=True)
        (opengfx / ".graphics_mode").write_text(mode, encoding="utf-8")
        sprites = (
            opengfx / "opengfx-8.0/sprites"
            if mode == "8bpp"
            else opengfx / "opengfx2-32ez/sprites"
        )
        sprites.mkdir(parents=True)
        nfo = "ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo"
        (sprites / nfo).write_text(nfo_rows(mode), encoding="utf-8")

        tiles = opengfx / "tiles"
        tiles.mkdir()
        size = (2, 1) if mode == "8bpp" else (3, 2)
        for index, sprite_id in enumerate(SPRITE_IDS):
            Image.new("RGBA", size, (index, 30, 60, 255)).save(tiles / PNG_BY_ID[sprite_id])
        return repo

    def test_layers_use_the_active_nfo_variant_and_keep_upstream_ids(self) -> None:
        for mode, expected_size, first_offset in (
            ("8bpp", (2, 1), (-1, -2)),
            ("32bpp", (3, 2), (-11, -12)),
        ):
            repo = self.make_repo(mode)
            rows = generator.write_layers(
                LAYER_BLOCKS,
                repo / "assets/opengfx/tiles",
                parse_sprite_offs(repo, mode),
                mode,
            )
            output = "\n".join(rows)
            self.assertIn("sprite_id: 1412", output, mode)
            self.assertIn("sprite_id: 1413", output, mode)
            self.assertIn("sx: 16, sy: 1", output, mode)
            self.assertIn("sx: 1, sy: 16", output, mode)
            self.assertIn(f"w: {expected_size[0]}.0, h: {expected_size[1]}.0", output, mode)
            self.assertIn(
                f"x_offs: {first_offset[0]}.0, y_offs: {first_offset[1]}.0",
                output,
                mode,
            )

    def test_graphics_download_regenerates_and_formats_the_table(self) -> None:
        root = Path(__file__).resolve().parents[1]
        source = (root / "scripts/descargar_graficos.sh").read_text(encoding="utf-8")
        self.assertIn('python3 "$(dirname "$0")/gen_road_depot_gfx_data.py"', source)
        self.assertIn("sprites/road_depot_gfx_data_generated.rs", source)


if __name__ == "__main__":
    unittest.main()
