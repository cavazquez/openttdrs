#!/usr/bin/env python3
"""Regresión de metadatos de cercas ferroviarias en 8bpp y 32bpp."""
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_track_fence_meta as generator


SPRITE_IDS = tuple(range(1301, 1309))


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
            lines.append("    | base.32.png 32bpp 0 3 12 8 -99 -99 zi4\n")
    return "".join(lines)


class TrackFenceSpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        root = repo / "assets/opengfx"
        root.mkdir(parents=True)
        (root / ".graphics_mode").write_text(mode, encoding="utf-8")
        sprites = (
            root / "opengfx-8.0/sprites"
            if mode == "8bpp"
            else root / "opengfx2-32ez/sprites"
        )
        sprites.mkdir(parents=True)
        nfo = "ogfx1_base.nfo" if mode == "8bpp" else "ogfx21_base_32ez.nfo"
        (sprites / nfo).write_text(nfo_rows(mode), encoding="utf-8")

        tiles = root / "tiles"
        tiles.mkdir()
        size = (2, 1) if mode == "8bpp" else (3, 2)
        for index in range(len(SPRITE_IDS)):
            Image.new("RGBA", size, (index, 30, 60, 255)).save(
                tiles / f"track_fence_{index}.png"
            )
        return repo

    def test_metadata_uses_the_active_normal_nfo_variant(self) -> None:
        for mode, expected_size, expected_offset in (
            ("8bpp", (2, 1), (-1, -2)),
            ("32bpp", (3, 2), (-11, -12)),
        ):
            metadata = generator.collect(self.make_repo(mode))
            self.assertEqual(len(metadata), len(SPRITE_IDS), mode)
            for index, (width, height, xrel, yrel) in enumerate(metadata):
                self.assertEqual((width, height), expected_size, f"{mode} sprite={index}")
                self.assertEqual(
                    (xrel, yrel),
                    (expected_offset[0] - index, expected_offset[1] - index),
                    f"{mode} sprite={index}",
                )


if __name__ == "__main__":
    unittest.main()
