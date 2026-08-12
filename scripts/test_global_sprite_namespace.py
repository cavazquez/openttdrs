#!/usr/bin/env python3
"""Regresión: los IDs globales OpenTTD no comparten namespace con un GRF extra."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from gen_field_draw_data import parse_global_sprite_rects
from nfo_sprite_meta import active_global_sprite_nfo, parse_sprite_offs


BASE_ROW = "4259 sprites/ogfx1_base00.png 8bpp 242 15528 64 31 -31 0 normal\n"
EXTRA_ROW = "4259 sprites/ogfxe_extra00.png 8bpp 274 9976 12 6 -5 -8 normal\n"
BASE_32_ROW = "4259 sprites/ogfx21_base_32ez00.png 32bpp 242 15528 128 62 -62 0 normal\n"
EXTRA_32_ROW = "4259 sprites/ogfx2e_extra_32ez00.png 32bpp 274 9976 24 12 -10 -16 normal\n"


class GlobalSpriteNamespaceTest(unittest.TestCase):
    def make_repo(self, mode: str) -> tuple[Path, Path]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        root = repo / "assets" / "opengfx"
        root.mkdir(parents=True)
        (root / ".graphics_mode").write_text(mode, encoding="utf-8")

        if mode == "8bpp":
            sprites = root / "opengfx-8.0" / "sprites"
            base_name, extra_name = "ogfx1_base.nfo", "ogfxe_extra.nfo"
            base_row, extra_row = BASE_ROW, EXTRA_ROW
        else:
            sprites = root / "opengfx2-32ez" / "sprites"
            base_name, extra_name = "ogfx21_base_32ez.nfo", "ogfx2e_extra_32ez.nfo"
            base_row, extra_row = BASE_32_ROW, EXTRA_32_ROW
        sprites.mkdir(parents=True)
        base = sprites / base_name
        base.write_text(base_row, encoding="utf-8")
        (sprites / extra_name).write_text(extra_row, encoding="utf-8")
        return repo, base

    def assert_uses_base_namespace(self, mode: str, expected_wh: tuple[int, int]) -> None:
        repo, base = self.make_repo(mode)
        self.assertEqual(active_global_sprite_nfo(repo), base)

        rect = parse_global_sprite_rects(base)[4259]
        self.assertEqual(rect[2:4], expected_wh)
        self.assertTrue(rect[4].startswith("ogfx1_base" if mode == "8bpp" else "ogfx21_base"))

        entries = parse_sprite_offs(repo)[4259]
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0][1:3], expected_wh)

    def test_8bpp_global_id_ignores_extra_local_id(self) -> None:
        self.assert_uses_base_namespace("8bpp", (64, 31))

    def test_32bpp_global_id_ignores_extra_local_id(self) -> None:
        self.assert_uses_base_namespace("32bpp", (128, 62))


if __name__ == "__main__":
    unittest.main()
