#!/usr/bin/env python3
"""Impide que el oráculo raster capture la partida temporal del dedicado."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "patches" / "openttd-15.3-snapshot-export" / "src" / "world_screenshot_export.cpp"


def compact(text: str) -> str:
    return re.sub(r"\s+", "", text)


class WorldScreenshotExportSourceContractTest(unittest.TestCase):
    def test_capture_waits_for_requested_save_and_preserves_requested_center(self) -> None:
        source = compact(SOURCE.read_text(encoding="utf-8"))

        # OpenTTD dedicated carga primero una partida temporal y recién después
        # el .sav de -g. El raster debe seguir el mismo mínimo de dos llamadas
        # que ya usa el oráculo estructural.
        self.assertIn('std::getenv("OPENTTDRS_WORLD_SCREENSHOT_MIN_CALL")', source)
        self.assertIn("staticintcall_count=0;call_count++;if(call_count<WorldScreenshotMinCall())returntrue;", source)
        self.assertLess(
            source.index("if(call_count<WorldScreenshotMinCall())returntrue;"),
            source.index("ParseResolution(std::getenv"),
        )

        # SC_DEFAULTZOOM mantiene el origen virtual del viewport. Para una
        # resolución distinta hay que trasladar scroll y destino, no sólo la
        # caché virtual que UpdateViewportPosition vuelve a calcular.
        self.assertIn("viewport.scrollpos_x+=delta_x;", source)
        self.assertIn("viewport.dest_scrollpos_x+=delta_x;", source)
        self.assertIn("viewport.scrollpos_y+=delta_y;", source)
        self.assertIn("viewport.dest_scrollpos_y+=delta_y;", source)
        self.assertIn(
            "UpdateViewportPosition(main_window,0);CenterScreenshotViewportOnMainWindow(*main_window,width,height);UpdateViewportPosition(main_window,0);",
            source,
        )


if __name__ == "__main__":
    unittest.main()
