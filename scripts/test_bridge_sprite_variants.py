#!/usr/bin/env python3
"""Regresión de recortes de puente con variantes OpenGFX 8bpp/32bpp."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_bridge_sprites
from nfo_sprite_meta import parse_global_sprite_rects


SPRITE_ID = 2510  # cantilever Y usado por el puente rojo de Kale.


def nfo_rows(mode: str) -> str:
    rows = [
        f"{SPRITE_ID} base.png 8bpp 2 0 2 1 -7 -8 normal\n",
    ]
    if mode == "32bpp":
        rows.extend(
            [
                "    | base.32.png 32bpp 3 0 3 2 -17 -18 normal\n",
                # Una variante de zoom no puede reemplazar la variante normal.
                "    | base.32.png 32bpp 0 3 12 8 -99 -99 zi4\n",
            ]
        )
    return "".join(rows)


class BridgeSpriteVariantsTest(unittest.TestCase):
    def make_sprites(self, mode: str) -> tuple[Path, Path]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        sprites = root / "sprites"
        sprites.mkdir()
        nfo = sprites / ("ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo")
        nfo.write_text(nfo_rows(mode), encoding="utf-8")

        image8 = Image.new("RGBA", (6, 1))
        image8.paste((8, 20, 30, 255), (2, 0, 4, 1))
        image8.save(sprites / "base.png")
        if mode == "32bpp":
            image32 = Image.new("RGBA", (6, 11))
            image32.paste((32, 120, 130, 255), (3, 0, 6, 2))
            image32.save(sprites / "base.32.png")
        return sprites, nfo

    def test_bridge_crop_uses_active_normal_variant(self) -> None:
        for mode, expected_size, expected_pixel in (
            ("8bpp", (2, 1), (8, 20, 30, 255)),
            ("32bpp", (3, 2), (32, 120, 130, 255)),
        ):
            sprites, nfo = self.make_sprites(mode)
            rects = parse_global_sprite_rects(nfo, mode)
            rect = rects[SPRITE_ID]
            crop = gen_bridge_sprites.crop_sprite(
                SPRITE_ID, rects, sprites, {}, mode
            )
            self.assertIsNotNone(crop)
            assert crop is not None
            self.assertEqual(crop.size, expected_size, mode)
            self.assertEqual(crop.getpixel((0, 0)), expected_pixel, mode)
            self.assertEqual((rect.xrel, rect.yrel), (-7, -8) if mode == "8bpp" else (-17, -18))


if __name__ == "__main__":
    unittest.main()
