#!/usr/bin/env python3
"""Impide que el oráculo raster capture la partida temporal del dedicado."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "patches" / "openttd-15.3-snapshot-export" / "src" / "world_screenshot_export.cpp"
INTEGRATOR = ROOT / "patches" / "openttd-15.3-snapshot-export" / "integrate.sh"
EXPORTER = ROOT / "scripts" / "export_openttd_world_screenshot.sh"
COMPARATOR = ROOT / "scripts" / "compare_focused_world_screenshot.sh"


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

        # La captura conserva el centro incluso cuando resolución o zoom son
        # distintos al viewport dedicado. Hay que trasladar scroll y destino,
        # no sólo la caché virtual que UpdateViewportPosition vuelve a calcular.
        self.assertIn("viewport.scrollpos_x+=delta_x;", source)
        self.assertIn("viewport.dest_scrollpos_x+=delta_x;", source)
        self.assertIn("viewport.scrollpos_y+=delta_y;", source)
        self.assertIn("viewport.dest_scrollpos_y+=delta_y;", source)
        self.assertIn(
            "UpdateViewportPosition(main_window,0);CenterScreenshotViewportOnMainWindow(*main_window,width,height,zoom);UpdateViewportPosition(main_window,0);",
            source,
        )

    def test_scale_contract_reaches_an_explicit_native_zoom(self) -> None:
        source = compact(SOURCE.read_text(encoding="utf-8"))
        integrator = compact(INTEGRATOR.read_text(encoding="utf-8"))
        exporter = EXPORTER.read_text(encoding="utf-8")
        comparator = COMPARATOR.read_text(encoding="utf-8")

        self.assertIn('std::getenv("OPENTTDRS_WORLD_SCREENSHOT_SCALE")', source)
        for scale, zoom in (
            ("0.25", "In4x"),
            ("0.5", "In2x"),
            ("1", "Normal"),
            ("2", "Out2x"),
            ("4", "Out4x"),
            ("8", "Out8x"),
        ):
            self.assertIn(f'value=="{scale}"', source)
            self.assertIn(f'zoom=ZoomLevel::{zoom};', source)
        self.assertIn("MakeScreenshotAtZoom(zoom,screenshot_name,width,height)", source)

        # El parche inyectado conserva la API normal y propaga el zoom al DPI
        # del raster, de modo que Out2x/Out4x seleccionan y escalan sprites
        # igual que el viewport nativo.
        self.assertIn("defintegrate_world_screenshot_zoom(dest:Path)->None:", integrator)
        self.assertIn("boolMakeScreenshotAtZoom(ZoomLevelzoom", integrator)
        self.assertIn('"\\t\\t.zoom=vp.zoom\\n"', integrator)
        self.assertIn('exportOPENTTDRS_WORLD_SCREENSHOT_SCALE="$SCALE"', compact(exporter))
        self.assertIn('OPENTTDRS_WORLD_SCREENSHOT_SCALE="$SCALE"', comparator)

    def test_unpinned_integration_preserves_an_existing_snapshot_or_cleans_it_consistently(self) -> None:
        integrator = INTEGRATOR.read_text(encoding="utf-8")

        # El modo world_raw_only puede entrar en un árbol nuevo o en un fork
        # basado en el pin. En el segundo caso, los hooks de generación/PBS
        # obligan a conservar snapshot_export.cpp; en el primero se retiran
        # juntos fuente, include y hooks post-tick.
        self.assertIn("preserve_snapshot_export = snapshot_source.exists()", integrator)
        self.assertIn("snapshot_dependent_markers", integrator)
        self.assertIn('text = add_cmake_source(cmake, text, "snapshot_export.cpp")', integrator)
        self.assertIn('text = text.replace("    world_raw_export.cpp\\n", "", 1)', integrator)
        self.assertIn("snapshot_export.h ya declara world-raw", integrator)
        self.assertIn("snapshot_hooks = (snapshot_hook + pbs_hook + fta_hook)", integrator)
        self.assertIn('pbs_tick_hook = "\\tOpenttdrsMaybeExportPbsTraceTick();\\n"', integrator)
        self.assertIn('fta_tick_hook = "\\tOpenttdrsMaybeExportAirportFtaTraceTick();\\n"', integrator)
        self.assertIn('for tick_hook in (pbs_tick_hook, fta_tick_hook):', integrator)
        self.assertIn('text = text.replace("    snapshot_export.cpp\\n", "", 1)', integrator)


if __name__ == "__main__":
    unittest.main()
