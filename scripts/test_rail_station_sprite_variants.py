#!/usr/bin/env python3
"""Regresión de recortes rail/mono/maglev de estaciones en 8bpp y 32bpp."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_rail_station_draw_data


BASE_IDS = tuple(range(1069, 1087))
MONO_IDS = tuple(sprite_id + 82 for sprite_id in BASE_IDS)
MAGLEV_IDS = tuple(sprite_id + 164 for sprite_id in BASE_IDS)


def nfo_rows(mode: str) -> str:
    """NFO mínimo con continuaciones 32bpp en otra hoja/rect para cada ID."""

    lines: list[str] = []
    for index, sprite_id in enumerate((*BASE_IDS, *MONO_IDS, *MAGLEV_IDS)):
        x8 = index * 2
        lines.append(
            f"{sprite_id} sprites/base.png 8bpp {x8} 0 2 1 {-index} {-index - 1} normal\n"
        )
        if mode == "32bpp":
            x32 = index * 3
            lines.append(
                f"    | sprites/base.32.png 32bpp {x32} 0 3 2 {-index - 10} {-index - 11} normal\n"
            )
            # Una fila de zoom incorrecto no puede desplazar al `normal`.
            lines.append(
                f"    | sprites/base.32.png 32bpp {x32} 2 12 8 -99 -99 zi4\n"
            )
    return "".join(lines)


class RailStationSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        root = repo / "assets" / "opengfx"
        root.mkdir(parents=True)
        (root / ".graphics_mode").write_text(mode, encoding="utf-8")
        base = (
            root / "opengfx-8.0" / "sprites"
            if mode == "8bpp"
            else root / "opengfx2-32ez" / "sprites"
        )
        base.mkdir(parents=True)
        nfo_name = "ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo"
        (base / nfo_name).write_text(nfo_rows(mode), encoding="utf-8")

        image8 = Image.new("RGBA", (len(BASE_IDS + MONO_IDS + MAGLEV_IDS) * 2, 1))
        for index in range(len(BASE_IDS + MONO_IDS + MAGLEV_IDS)):
            image8.paste((index, 20, 30, 255), (index * 2, 0, index * 2 + 2, 1))
        image8.save(base / "base.png")
        if mode == "32bpp":
            image32 = Image.new("RGBA", (len(BASE_IDS + MONO_IDS + MAGLEV_IDS) * 3, 10))
            for index in range(len(BASE_IDS + MONO_IDS + MAGLEV_IDS)):
                image32.paste(
                    (index, 120, 130, 255), (index * 3, 0, index * 3 + 3, 2)
                )
            image32.save(base / "base.32.png")
        return repo

    def test_typed_station_recrops_match_active_graphics_mode(self) -> None:
        for mode, expected_size, expected_y in (
            ("8bpp", (2, 1), 20),
            ("32bpp", (3, 2), 120),
        ):
            repo = self.make_repo(mode)
            old_repo = gen_rail_station_draw_data.REPO
            old_tiles = gen_rail_station_draw_data.TILES_DIR
            old_out = gen_rail_station_draw_data.OUT_RS
            try:
                gen_rail_station_draw_data.REPO = repo
                gen_rail_station_draw_data.TILES_DIR = repo / "assets" / "opengfx" / "tiles"
                gen_rail_station_draw_data.OUT_RS = repo / "station.rs"
                gen_rail_station_draw_data.TILES_DIR.mkdir(parents=True)
                gen_rail_station_draw_data.extract_typed_station_sprites(mode)
            finally:
                (
                    gen_rail_station_draw_data.REPO,
                    gen_rail_station_draw_data.TILES_DIR,
                    gen_rail_station_draw_data.OUT_RS,
                ) = (old_repo, old_tiles, old_out)

            for index, sprite_id in enumerate((*MONO_IDS, *MAGLEV_IDS), start=len(BASE_IDS)):
                with Image.open(repo / "assets" / "opengfx" / "tiles" / f"rail_{sprite_id}.png") as crop:
                    self.assertEqual(crop.size, expected_size, f"{mode} sprite={sprite_id}")
                    self.assertEqual(
                        crop.convert("RGBA").getpixel((0, 0)),
                        (index, expected_y, expected_y + 10, 255),
                        f"{mode} sprite={sprite_id}",
                    )


if __name__ == "__main__":
    unittest.main()
