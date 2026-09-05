#!/usr/bin/env python3
"""Regresión de recortes airport en 8bpp/32bpp.

El fixture reduce la tabla a cuatro sprites, pero ejercita el mismo generador
que usa producción: base, capa sortable, capa ground y la selección del NFO
``normal`` del perfil activo. Así no se puede volver a tomar por accidente la
hoja 8bpp al regenerar un atlas 32bpp (ni viceversa).
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_airport_station_draw_data as generator
from pillow_compat import flattened_data


SPRITE_IDS = (2661, 2662, 2663, 2664)


def nfo_rows(mode: str) -> str:
    """NFO mínimo con continuación 32bpp y una variante zi que se ignora."""

    lines: list[str] = []
    for index, sprite_id in enumerate(SPRITE_IDS):
        lines.append(
            f"{sprite_id} base.png 8bpp {index * 2} 0 2 1 {-index - 7} {-index - 8} normal\n"
        )
        if mode == "32bpp":
            lines.append(
                f"    | base.32.png 32bpp {index * 3} 0 3 2 {-index - 17} {-index - 18} normal\n"
            )
            lines.append(
                f"    | base.32.png 32bpp 0 3 12 8 -99 -99 zi4\n"
            )
    return "".join(lines)


class AirportStationSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        root = repo / "assets" / "opengfx"
        root.mkdir(parents=True)
        (root / ".graphics_mode").write_text(mode, encoding="utf-8")
        sprites = (
            root / "opengfx-8.0" / "sprites"
            if mode == "8bpp"
            else root / "opengfx2-32ez" / "sprites"
        )
        sprites.mkdir(parents=True)
        nfo_name = "ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo"
        (sprites / nfo_name).write_text(nfo_rows(mode), encoding="utf-8")

        image8 = Image.new("RGBA", (len(SPRITE_IDS) * 2, 1), (0, 0, 0, 0))
        for index in range(len(SPRITE_IDS)):
            image8.paste((index + 8, 20, 30, 255), (index * 2, 0, index * 2 + 2, 1))
        image8.save(sprites / "base.png")
        if mode == "32bpp":
            image32 = Image.new("RGBA", (len(SPRITE_IDS) * 3, 11), (0, 0, 0, 0))
            for index in range(len(SPRITE_IDS)):
                image32.paste(
                    (index + 32, 120, 130, 255),
                    (index * 3, 0, index * 3 + 3, 2),
                )
            image32.save(sprites / "base.32.png")
        return repo

    def test_airport_crops_use_the_active_normal_variant(self) -> None:
        for mode, expected_size, expected_y, expected_xrel in (
            ("8bpp", (2, 1), 20, -7),
            ("32bpp", (3, 2), 120, -17),
        ):
            repo = self.make_repo(mode)
            saved = {
                "REPO": generator.REPO,
                "TILES_DIR": generator.TILES_DIR,
                "OUT_RS": generator.OUT_RS,
                "AIRPORT_STATION_BASES": generator.AIRPORT_STATION_BASES,
                "AIRPORT_STATION_OVERLAYS": generator.AIRPORT_STATION_OVERLAYS,
                "AIRPORT_STATION_GROUND_LAYERS": generator.AIRPORT_STATION_GROUND_LAYERS,
                "GLOBAL_SPRITES": generator.GLOBAL_SPRITES,
                "ACTION5_SPRITES": generator.ACTION5_SPRITES,
                "DYNAMIC_SPRITE_IDS": generator.DYNAMIC_SPRITE_IDS,
            }
            try:
                generator.REPO = repo
                generator.TILES_DIR = repo / "assets" / "opengfx" / "tiles"
                generator.OUT_RS = repo / "airport.rs"
                generator.AIRPORT_STATION_BASES = ((0, "TEST", 2661, False),)
                generator.AIRPORT_STATION_OVERLAYS = (
                    generator.line(0, "TEST", 2662, 3, 2, 0, 3, 3, 14),
                )
                generator.AIRPORT_STATION_GROUND_LAYERS = (
                    generator.ground(0, "TEST", 2663, 15, 0),
                    generator.ground(0, "TEST", 2664, 0, 15),
                )
                generator.GLOBAL_SPRITES = tuple(
                    (sprite_id, f"fixture_{sprite_id}.png") for sprite_id in SPRITE_IDS
                )
                generator.ACTION5_SPRITES = ()
                generator.DYNAMIC_SPRITE_IDS = ()
                sprites, layers, ground_layers = generator.airport_station_data(
                    mode, write_tiles=True
                )
                rendered = generator.render_output(sprites, layers, ground_layers, mode)
            finally:
                for name, value in saved.items():
                    setattr(generator, name, value)

            self.assertIn("AIRPORT_STATION_SPRITES", rendered)
            self.assertIn("AIRPORT_GFX_0_LAYERS", rendered)
            self.assertIn("AIRPORT_GFX_0_GROUND_LAYERS", rendered)
            self.assertIn("airport_station_base_for_gfx", rendered)
            for index, sprite_id in enumerate(SPRITE_IDS):
                name, rect = sprites[sprite_id]
                with Image.open(repo / "assets" / "opengfx" / "tiles" / name) as crop:
                    self.assertEqual(crop.size, expected_size, f"{mode} sprite={sprite_id}")
                    self.assertEqual(
                        crop.convert("RGBA").getpixel((0, 0)),
                        (
                            index + (8 if mode == "8bpp" else 32),
                            expected_y,
                            expected_y + 10,
                            255,
                        ),
                        f"{mode} sprite={sprite_id}",
                    )
                self.assertEqual(rect.xrel, expected_xrel - index, f"{mode} sprite={sprite_id}")

    def test_32bpp_action5_falls_back_to_official_indexed_sprite(self) -> None:
        repo = self.make_repo("32bpp")
        root = repo / "assets" / "opengfx"
        active_sprites = root / "opengfx2-32ez" / "sprites"
        (active_sprites / "ogfx2e_extra_32ez.nfo").write_text(
            "1 * 8 05 95 FF 01 00 FF 56 00\n"
            "2 active.png 8bpp 0 0 1 1 0 0 normal\n",
            encoding="utf-8",
        )

        fallback_sprites = root / ".signal-src-8bpp" / "sprites"
        fallback_sprites.mkdir(parents=True)
        (fallback_sprites / "ogfxe_extra.nfo").write_text(
            "10 * 8 05 95 FF 01 00 FF 56 00\n"
            "11 fallback.png 8bpp 0 0 2 1 -3 -4 normal\n",
            encoding="utf-8",
        )
        fallback_sheet = Image.new("P", (2, 1))
        fallback_sheet.putpalette([0, 0, 0, 16, 16, 16, 32, 32, 32] + [0] * (256 * 3 - 9))
        fallback_sheet.putdata([1, 2])
        fallback_sheet.save(fallback_sprites / "fallback.png")

        saved = {
            "REPO": generator.REPO,
            "TILES_DIR": generator.TILES_DIR,
            "ACTION5_SPRITES": generator.ACTION5_SPRITES,
        }
        try:
            generator.REPO = repo
            generator.TILES_DIR = root / "tiles"
            generator.ACTION5_SPRITES = ((4982, "airport_helipad.png"),)
            cropper = generator.AirportStationSpriteCropper("32bpp")
            source_dir, rect, source_mode = cropper.source_with_mode(4982)
            self.assertEqual(source_dir, fallback_sprites)
            self.assertEqual(source_mode, "8bpp")
            self.assertEqual(rect, generator.SpriteRect(0, 0, 2, 1, -3, -4, "fallback.png"))

            cropper.crop(4982, "airport_helipad.png")
            with Image.open(fallback_sprites / "fallback.png") as source:
                expected_pixels = list(
                    flattened_data(generator.indexed_dos_to_rgba(source))
                )
            with Image.open(root / "tiles" / "airport_helipad.png") as rendered:
                self.assertEqual(rendered.size, (2, 1))
                self.assertEqual(
                    list(flattened_data(rendered.convert("RGBA"))),
                    expected_pixels,
                )
        finally:
            for name, value in saved.items():
                setattr(generator, name, value)

    def test_32bpp_action5_prefers_native_variant_over_fallback(self) -> None:
        repo = self.make_repo("32bpp")
        root = repo / "assets" / "opengfx"
        active_sprites = root / "opengfx2-32ez" / "sprites"
        (active_sprites / "ogfx2e_extra_32ez.nfo").write_text(
            "1 * 8 05 95 FF 01 00 FF 56 00\n"
            "2 active.png 8bpp 0 0 1 1 0 0 normal\n"
            "| active.32.png 32bpp 0 0 3 2 -5 -6 normal\n",
            encoding="utf-8",
        )
        active_sheet = Image.new("RGBA", (3, 2), (12, 34, 56, 255))
        active_sheet.save(active_sprites / "active.32.png")

        fallback_sprites = root / ".signal-src-8bpp" / "sprites"
        fallback_sprites.mkdir(parents=True)
        (fallback_sprites / "ogfxe_extra.nfo").write_text(
            "10 * 8 05 95 FF 01 00 FF 56 00\n"
            "11 fallback.png 8bpp 0 0 2 1 -3 -4 normal\n",
            encoding="utf-8",
        )
        fallback_sheet = Image.new("P", (2, 1))
        fallback_sheet.putpalette([0, 0, 0, 16, 16, 16, 32, 32, 32] + [0] * (256 * 3 - 9))
        fallback_sheet.putdata([1, 2])
        fallback_sheet.save(fallback_sprites / "fallback.png")

        saved = {
            "REPO": generator.REPO,
            "TILES_DIR": generator.TILES_DIR,
            "ACTION5_SPRITES": generator.ACTION5_SPRITES,
        }
        try:
            generator.REPO = repo
            generator.TILES_DIR = root / "tiles"
            generator.ACTION5_SPRITES = ((4982, "airport_helipad.png"),)
            cropper = generator.AirportStationSpriteCropper("32bpp")
            source_dir, rect, source_mode = cropper.source_with_mode(4982)
            self.assertEqual(source_dir, active_sprites)
            self.assertEqual(source_mode, "32bpp")
            self.assertEqual(rect, generator.SpriteRect(0, 0, 3, 2, -5, -6, "active.32.png"))

            cropper.crop(4982, "airport_helipad.png")
            with Image.open(root / "tiles" / "airport_helipad.png") as rendered:
                self.assertEqual(rendered.size, (3, 2))
                self.assertEqual(rendered.convert("RGBA").getpixel((0, 0)), (12, 34, 56, 255))
        finally:
            for name, value in saved.items():
                setattr(generator, name, value)


if __name__ == "__main__":
    unittest.main()
