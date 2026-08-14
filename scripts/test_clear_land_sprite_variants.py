#!/usr/bin/env python3
"""Regresión del recorte de suelo rocoso en OpenGFX 8bpp y OpenGFX2 32bpp."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import crop_clear_land_sprites as generator


SPRITES = ((4023, "terrain_rocky_1_00.png"), (4024, "terrain_rocky_1_01.png"))


def nfo_rows(mode: str, prefix: str) -> str:
    lines: list[str] = []
    for index, (sprite_id, _name) in enumerate(SPRITES):
        lines.append(
            f"{sprite_id} {prefix}00.png 8bpp {index * 2} 0 2 1 {-index - 7} {-index - 8} normal\n"
        )
        if mode == "32bpp":
            lines.append(
                f"    | {prefix}00.32.png 32bpp {index * 3} 0 3 2 {-index - 17} {-index - 18} normal\n"
            )
            lines.append(f"    | {prefix}00.32.png 32bpp 0 3 12 8 -99 -99 zi4\n")
    return "".join(lines)


class ClearLandSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        opengfx = repo / "assets" / "opengfx"
        opengfx.mkdir(parents=True)
        (opengfx / ".graphics_mode").write_text(mode, encoding="utf-8")
        sprites = (
            opengfx / "opengfx-8.0" / "sprites"
            if mode == "8bpp"
            else opengfx / "opengfx2-32ez" / "sprites"
        )
        sprites.mkdir(parents=True)
        prefix = "ogfx1_base" if mode == "8bpp" else "ogfx21_base_32ez"
        (sprites / f"{prefix}.nfo").write_text(nfo_rows(mode, prefix), encoding="utf-8")

        image8 = Image.new("RGBA", (len(SPRITES) * 2, 1), (0, 0, 0, 0))
        for index in range(len(SPRITES)):
            image8.paste((index + 8, 20, 30, 255), (index * 2, 0, index * 2 + 2, 1))
        image8.save(sprites / f"{prefix}00.png")
        if mode == "32bpp":
            image32 = Image.new("RGBA", (len(SPRITES) * 3, 11), (0, 0, 0, 0))
            for index in range(len(SPRITES)):
                image32.paste(
                    (index + 32, 120, 130, 255),
                    (index * 3, 0, index * 3 + 3, 2),
                )
            image32.save(sprites / f"{prefix}00.32.png")
        return repo

    def test_rocky_crops_use_the_active_normal_variant(self) -> None:
        original = generator.SPRITES
        self.addCleanup(setattr, generator, "SPRITES", original)
        generator.SPRITES = SPRITES
        for mode, expected_size, expected_pixel in (
            ("8bpp", (2, 1), (8, 20, 30, 255)),
            ("32bpp", (3, 2), (32, 120, 130, 255)),
        ):
            repo = self.make_repo(mode)
            count, failures = generator.crop_clear_land_sprites(repo, force=True)
            self.assertEqual(count, len(SPRITES), mode)
            self.assertEqual(failures, [], mode)
            for index, (_sprite_id, name) in enumerate(SPRITES):
                with Image.open(repo / "assets" / "opengfx" / "tiles" / name) as crop:
                    self.assertEqual(crop.size, expected_size, f"{mode} {name}")
                    self.assertEqual(
                        crop.convert("RGBA").getpixel((0, 0)),
                        (index + expected_pixel[0], expected_pixel[1], expected_pixel[2], 255),
                        f"{mode} {name}",
                    )


if __name__ == "__main__":
    unittest.main()
