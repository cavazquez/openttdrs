#!/usr/bin/env python3
"""Regresión de metadatos de industria para perfiles 8bpp y 32bpp.

El fallo que motivó este test mezclaba la geometría NFO de OpenGFX2 32bpp con
los PNG activos 8bpp. El fixture usa el mismo generador que producción y un
``M()`` de OpenTTD: valida tamaño, ancla y bounds 3D de ambos perfiles sin
depender de assets descargados ni de una ventana gráfica.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image

import gen_industry_gfx_data as generator


SPRITE_ID = 2096


def nfo_rows(mode: str) -> str:
    lines = [f"{SPRITE_ID} base.png 8bpp 0 0 2 1 -7 -8 normal\n"]
    if mode == "32bpp":
        lines.append(
            "    | base.32.png 32bpp 0 0 3 2 -17 -18 normal\n"
        )
        # El zoom no normal no debe desplazar a la variante normal activa.
        lines.append("    | base.32.png 32bpp 0 3 12 8 -99 -99 zi4\n")
    return "".join(lines)


class IndustrySpriteVariantsTest(unittest.TestCase):
    def make_repo(self, mode: str) -> tuple[Path, Path]:
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

        tiles = root / "tiles"
        tiles.mkdir()
        size = (2, 1) if mode == "8bpp" else (3, 2)
        Image.new("RGBA", size, (20, 30, 40, 255)).save(tiles / f"industry_{SPRITE_ID}.png")

        upstream = repo / "third_party" / "openttd" / "industry_land.h"
        upstream.parent.mkdir(parents=True)
        upstream.write_text(
            "".join(
                "M(0xf54, PAL_NONE, 0x830, PAL_NONE, 0, 0, 16, 16, 20, 0),\n"
                for _ in range(generator.GFX_COUNT * generator.STAGES)
            ),
            encoding="utf-8",
        )
        return repo, upstream

    def test_industry_metadata_follows_active_normal_variant(self) -> None:
        for mode, expected in (
            ("8bpp", "w: 2.0, h: 1.0, xrel: -7.0, yrel: -8.0"),
            ("32bpp", "w: 3.0, h: 2.0, xrel: -17.0, yrel: -18.0"),
        ):
            repo, upstream = self.make_repo(mode)
            content, _stats = generator.build_content(repo, upstream)
            self.assertIn(expected, content, mode)
            self.assertIn(
                "sort_ox: 0, sort_oy: 0, sort_oz: 0, "
                "sort_ex: 16, sort_ey: 16, sort_ez: 20",
                content,
                mode,
            )


if __name__ == "__main__":
    unittest.main()
