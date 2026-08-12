#!/usr/bin/env python3
"""Regresiones de la conversión indexada de OpenGFX DOS."""

from __future__ import annotations

import unittest

from PIL import Image

from extract_rail_pbs_palette_sprites import remap_indexed_crash
from gen_company_palette_rust import COMPANY_RAMP_INDICES, build_outputs, load_dos_palette
from opengfx_palette import indexed_dos_to_rgba


class OpenGfxPaletteTest(unittest.TestCase):
    def test_uses_dos_indexes_not_embedded_windows_palette(self) -> None:
        source = Image.new("P", (7, 1))
        source.putdata([0, 1, 9, 10, 215, 218, 245])
        source.putpalette(
            [0, 0, 255, 238, 0, 238, 246, 0, 246, 168, 168, 168]
            + [0] * (256 * 3 - 12)
        )
        actual = list(indexed_dos_to_rgba(source).get_flattened_data())
        self.assertEqual(actual[0], (0, 0, 0, 0))
        self.assertEqual(actual[1], (16, 16, 16, 255))
        self.assertEqual(actual[2], (148, 149, 148, 255))
        self.assertEqual(actual[3], (168, 168, 168, 255))
        self.assertEqual(actual[4], (0, 0, 0, 0))
        self.assertEqual(actual[5], (0, 0, 0, 0))
        self.assertEqual(actual[6], (32, 68, 112, 255))

    def test_company_ramps_keep_the_transparent_dos_slot(self) -> None:
        palette = load_dos_palette()
        self.assertEqual(len(palette), 256)
        self.assertEqual(
            [palette[index] for index in COMPANY_RAMP_INDICES[0]],
            [
                (8, 24, 88),
                (12, 36, 104),
                (20, 52, 124),
                (28, 68, 140),
                (40, 92, 164),
                (56, 120, 188),
                (72, 152, 216),
                (100, 172, 224),
            ],
        )

    def test_company_palette_outputs_are_current(self) -> None:
        for output, expected in build_outputs().items():
            self.assertEqual(output.read_text(encoding="utf-8"), expected, output)

    def test_crash_remap_uses_source_indexes_not_approximate_rgb(self) -> None:
        source = Image.new("P", (5, 1))
        source.putdata([0, 1, 2, 3, 215])
        # La paleta PNG embebida deliberadamente no coincide con DOS: el
        # resultado debe depender sólo de los índices y de la tabla 804.
        source.putpalette([255, 0, 255] * 256)
        table = tuple([0, 9, 2, 1] + list(range(4, 256)))
        palette = tuple((index, index + 1, index + 2) for index in range(256))
        actual = list(remap_indexed_crash(source, table, palette).get_flattened_data())
        self.assertEqual(
            actual,
            [
                (0, 0, 0, 0),
                (9, 10, 11, 255),
                (2, 3, 4, 255),
                (1, 2, 3, 255),
                (0, 0, 0, 0),
            ],
        )


if __name__ == "__main__":
    unittest.main()
