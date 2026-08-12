#!/usr/bin/env python3
"""Regresión: los IDs globales OpenTTD no comparten namespace con un GRF extra."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import active_global_sprite_nfo, parse_global_sprite_rects, parse_sprite_offs


BASE_ROW = "4259 sprites/ogfx1_base00.png 8bpp 242 15528 64 31 -31 0 normal\n"
EXTRA_ROW = "4259 sprites/ogfxe_extra00.png 8bpp 274 9976 12 6 -5 -8 normal\n"
BASE_32_FALLBACK_ROW = "4259 sprites/ogfx21_base_32ez00.png 8bpp 242 15528 64 31 -31 0 normal\n"
BASE_32_NORMAL_ROW = "    | sprites/ogfx21_base_32ez00.32.png 32bpp 514 7824 128 62 -62 0 normal\n"
BASE_32_ZI4_ROW = "    | sprites/ogfx21_base_32ez00.32.png 32bpp 2 8000 512 248 -248 0 zi4\n"
EXTRA_32_ROW = "4259 sprites/ogfx2e_extra_32ez00.png 32bpp 274 9976 24 12 -10 -16 normal\n"

# Entrada pequeña que se puede recortar en una fixture sin construir sheets de
# 16.000 px. La fila RGBA tiene otra hoja/posición que la indexada a propósito.
CROP_8_ROW = "4126 sprites/ogfx1_base00.png 8bpp 2 3 6 4 -3 0 normal\n"
CROP_32_FALLBACK_ROW = "4126 sprites/ogfx21_base_32ez00.png 8bpp 2 3 6 4 -3 0 normal\n"
CROP_32_NORMAL_ROW = "    | sprites/ogfx21_base_32ez00.32.png 32bpp 20 30 12 8 -6 0 normal\n"
CROP_32_ZI4_ROW = "    | sprites/ogfx21_base_32ez00.32.png 32bpp 0 0 48 32 -24 0 zi4\n"


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
            base_row, extra_row = BASE_ROW + CROP_8_ROW, EXTRA_ROW
        else:
            sprites = root / "opengfx2-32ez" / "sprites"
            base_name, extra_name = "ogfx21_base_32ez.nfo", "ogfx2e_extra_32ez.nfo"
            base_row = (
                BASE_32_FALLBACK_ROW
                + BASE_32_NORMAL_ROW
                + BASE_32_ZI4_ROW
                + CROP_32_FALLBACK_ROW
                + CROP_32_NORMAL_ROW
                + CROP_32_ZI4_ROW
            )
            extra_row = EXTRA_32_ROW
        sprites.mkdir(parents=True)
        base = sprites / base_name
        base.write_text(base_row, encoding="utf-8")
        (sprites / extra_name).write_text(extra_row, encoding="utf-8")
        return repo, base

    def assert_uses_base_namespace(self, mode: str, expected_wh: tuple[int, int]) -> None:
        repo, base = self.make_repo(mode)
        self.assertEqual(active_global_sprite_nfo(repo), base)

        rect = parse_global_sprite_rects(base, mode)[4259]
        self.assertEqual(rect[2:4], expected_wh)
        self.assertTrue(
            rect.sheet.startswith("ogfx1_base" if mode == "8bpp" else "ogfx21_base")
        )
        if mode == "32bpp":
            self.assertTrue(rect.sheet.endswith(".32.png"))

        entries = parse_sprite_offs(repo)[4259]
        expected_bpps = ["8bpp"] if mode == "8bpp" else ["8bpp", "32bpp"]
        self.assertEqual([entry[0] for entry in entries], expected_bpps)
        self.assertEqual(entries[-1][1:3], expected_wh)

    def test_8bpp_global_id_ignores_extra_local_id(self) -> None:
        self.assert_uses_base_namespace("8bpp", (64, 31))

    def test_32bpp_global_id_ignores_extra_local_id(self) -> None:
        self.assert_uses_base_namespace("32bpp", (128, 62))

    def test_32bpp_uses_matching_normal_variant_not_8bpp_or_zi4(self) -> None:
        _repo, base = self.make_repo("32bpp")
        rect = parse_global_sprite_rects(base, "32bpp")[4259]
        self.assertEqual(
            rect,
            (514, 7824, 128, 62, -62, 0, "ogfx21_base_32ez00.32.png"),
        )

    def test_field_cropper_uses_the_active_8bpp_or_32bpp_sheet(self) -> None:
        """La alternativa 32bpp conserva su rect y no cae en la hoja 8bpp."""
        import gen_field_draw_data

        for mode, expected_size, expected_color in (
            ("8bpp", (6, 4), (25, 50, 75, 255)),
            ("32bpp", (12, 8), (125, 150, 175, 255)),
        ):
            repo, _base = self.make_repo(mode)
            sprites = next((repo / "assets" / "opengfx").glob("*/sprites"))
            sheet8 = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
            sheet8.paste((25, 50, 75, 255), (2, 3, 8, 7))
            if mode == "8bpp":
                sheet8.save(sprites / "ogfx1_base00.png")
            else:
                sheet8.save(sprites / "ogfx21_base_32ez00.png")
                sheet32 = Image.new("RGBA", (40, 48), (0, 0, 0, 0))
                sheet32.paste((125, 150, 175, 255), (20, 30, 32, 38))
                sheet32.save(sprites / "ogfx21_base_32ez00.32.png")

            old_repo, old_tiles = gen_field_draw_data.REPO, gen_field_draw_data.TILES_DIR
            try:
                gen_field_draw_data.REPO = repo
                gen_field_draw_data.TILES_DIR = repo / "assets" / "opengfx" / "tiles"
                gen_field_draw_data.TILES_DIR.mkdir(parents=True, exist_ok=True)
                cropper = gen_field_draw_data.Cropper(mode)
                cropper.crop(4126, "field.png")
            finally:
                gen_field_draw_data.REPO, gen_field_draw_data.TILES_DIR = old_repo, old_tiles

            with Image.open(repo / "assets" / "opengfx" / "tiles" / "field.png") as crop:
                self.assertEqual(crop.size, expected_size, mode)
                self.assertEqual(crop.convert("RGBA").getpixel((0, 0)), expected_color, mode)


if __name__ == "__main__":
    unittest.main()
