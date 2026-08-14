#!/usr/bin/env python3
"""Regresión de metadata de paradas viales en 8bpp y 32bpp.

Los IDs usados por ``DrawTileSeq`` no son los IDs locales Action5 de los PNG
extra. Este test verifica ambas variantes de gráficos para que una
regeneración no vuelva a mezclar 2009..2018 con SPR_ROADSTOP_BASE 5978..5985.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_road_stop_gfx_data as generator


BUILD_IDS = tuple(range(2692, 2724))
ACTION5_IDS = (2009, 2010, 2013, 2014, 2015, 2016, 2017, 2018)


def station_land_fixture() -> str:
    """Subset literal de los TILE_SEQ relevantes de station_land.h."""

    rows = {
        67: ((0, 15, 0, 13, 1, 10), (13, 0, 0, 3, 16, 10), (2, 0, 0, 11, 1, 10)),
        68: ((15, 3, 0, 1, 13, 10), (0, 0, 0, 16, 3, 10), (0, 3, 0, 1, 11, 10)),
        69: ((3, 0, 0, 13, 1, 10), (0, 0, 0, 3, 16, 10), (3, 15, 0, 11, 1, 10)),
        70: ((0, 0, 0, 1, 13, 10), (0, 13, 0, 16, 3, 10), (15, 2, 0, 1, 11, 10)),
        71: ((2, 0, 0, 11, 1, 10), (13, 0, 0, 3, 16, 10), (0, 13, 0, 13, 3, 10)),
        72: ((0, 3, 0, 1, 11, 10), (0, 0, 0, 16, 3, 10), (13, 3, 0, 3, 13, 10)),
        73: ((3, 15, 0, 11, 1, 10), (0, 0, 0, 3, 16, 10), (3, 0, 0, 13, 3, 10)),
        74: ((15, 2, 0, 1, 11, 10), (0, 13, 0, 16, 3, 10), (0, 0, 0, 3, 13, 10)),
        168: ((0, 0, 0, 16, 3, 16), (0, 13, 0, 16, 3, 16)),
        169: ((13, 0, 0, 3, 16, 16), (0, 0, 0, 3, 16, 16)),
        170: ((0, 0, 0, 16, 3, 16), (0, 13, 0, 16, 3, 16)),
        171: ((13, 0, 0, 3, 16, 16), (0, 0, 0, 3, 16, 16)),
    }
    chunks: list[str] = []
    for data_id, lines in rows.items():
        chunks.append(f"static const DrawTileSeqStruct _station_display_datas_{data_id}[] = {{")
        for dx, dy, dz, sx, sy, sz in lines:
            chunks.append(f"TILE_SEQ_LINE({dx}, {dy}, {dz}, {sx}, {sy}, {sz}, SPR_ANY)")
        chunks.append("};")
    return "\n".join(chunks)


def nfo_rows(ids: tuple[int, ...], mode: str, *, start: int = 0) -> str:
    """Una fila normal por sprite; nunca se debe seleccionar un zi4."""

    bpp = "8bpp" if mode == "8bpp" else "32bpp"
    width, height = ((3, 2) if mode == "8bpp" else (6, 4))
    rows: list[str] = []
    for index, sprite_id in enumerate(ids, start):
        rows.append(
            f"{sprite_id} sheet.png {bpp} {index * width} 0 {width} {height} "
            f"{-index - 5} {-index - 6} normal\n"
        )
        if mode == "32bpp":
            rows.append("    | sheet.png 32bpp 0 8 24 16 -99 -99 zi4\n")
    return "".join(rows)


class RoadStopSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> tuple[Path, Path]:
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
        base = "ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo"
        extra = "ogfxe_extra.nfo" if mode == "8bpp" else "ogfx2e_extra_32ez.nfo"
        (sprites / base).write_text(nfo_rows(BUILD_IDS, mode), encoding="utf-8")
        (sprites / extra).write_text(nfo_rows(ACTION5_IDS, mode, start=50), encoding="utf-8")

        tiles = opengfx / "tiles"
        tiles.mkdir()
        image_size = (3, 2) if mode == "8bpp" else (6, 4)
        for prefix in ("bus_stop", "truck_stop"):
            for direction in generator.DIRS:
                for layer in ("a", "b", "c"):
                    Image.new("RGBA", image_size).save(tiles / f"{prefix}_{direction}_build_{layer}.png")
        for names in generator.DRIVE_THROUGH_NAMES.values():
            for name in names:
                Image.new("RGBA", image_size).save(tiles / name)

        upstream = repo / "station_land.h"
        upstream.write_text(station_land_fixture(), encoding="utf-8")
        return repo, upstream

    def test_logical_ids_and_bounds_are_graphics_mode_independent(self) -> None:
        for mode, expected_size in (("8bpp", "w: 3.0, h: 2.0"), ("32bpp", "w: 6.0, h: 4.0")):
            repo, upstream = self.make_repo(mode)
            blocks = generator.parse_tile_seq_blocks(upstream)
            nfo = generator.parse_sprite_offs(repo, mode)
            tiles = repo / "assets" / "opengfx" / "tiles"

            bus = "\n".join(
                generator.write_layers(
                    blocks,
                    generator.BUS_DATAS,
                    "bus_stop",
                    True,
                    tiles,
                    nfo,
                    mode,
                )
            )
            truck_dt = "\n".join(
                generator.write_drive_through_layers(
                    blocks,
                    generator.DRIVE_THROUGH_TRUCK_DATAS,
                    "truck",
                    ACTION5_IDS[4:],
                    tiles,
                    nfo,
                    mode,
                )
            )

            self.assertIn("sprite_id: 2696, bounds: (11, 1, 10)", bus, mode)
            self.assertIn("sprite_id: 2704, bounds: (13, 3, 10)", bus, mode)
            self.assertIn(expected_size, bus, mode)
            self.assertIn("sprite_id: 5984, bounds: (16, 3, 16)", truck_dt, mode)
            self.assertIn("sprite_id: 5985, bounds: (16, 3, 16)", truck_dt, mode)
            self.assertNotIn("sprite_id: 2017", truck_dt, mode)


if __name__ == "__main__":
    unittest.main()
