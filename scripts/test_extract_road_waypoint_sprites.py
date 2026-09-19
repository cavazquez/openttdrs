#!/usr/bin/env python3
"""Regresión del fallback opcional de sprites de road waypoint.

Un checkout de referencia puede contener el NFO oficial sin la hoja PNG. En
ese caso el generador debe señalizar fuente opcional ausente (exit 2), sin
sobrescribir la metadata ya versionada ni hacer fallar el pipeline de assets.
"""

from __future__ import annotations

import importlib.util
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "extract_road_waypoint_sprites.py"


def load_module():
    spec = importlib.util.spec_from_file_location("road_waypoint_extractor", MODULE_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    extractor = load_module()
    with tempfile.TemporaryDirectory() as temp:
        temp_root = Path(temp)
        nfo = temp_root / "road_waypoints.nfo"
        nfo.write_text(
            "\n".join(
                f"{index} road_waypoints.png 8bpp 0 0 1 1 0 0"
                for index in range(4)
            ),
            encoding="utf-8",
        )
        out_rs = temp_root / "road_waypoint_gfx_data_generated.rs"
        out_rs.write_text("metadata previa\n", encoding="utf-8")

        original = (
            extractor.ROOT,
            extractor.TILES,
            extractor.OUT_RS,
            extractor.find_nfo,
            extractor.find_sheet,
        )
        try:
            extractor.ROOT = temp_root
            extractor.TILES = temp_root / "tiles"
            extractor.OUT_RS = out_rs
            extractor.find_nfo = lambda: nfo
            extractor.find_sheet = lambda _nfo, _name: None

            result = extractor.main()
        finally:
            (
                extractor.ROOT,
                extractor.TILES,
                extractor.OUT_RS,
                extractor.find_nfo,
                extractor.find_sheet,
            ) = original

        assert result == 2, result
        assert out_rs.read_text(encoding="utf-8") == "metadata previa\n"
        assert not (temp_root / "tiles" / "road_waypoint_y_w.png").exists()

    print("OK: road waypoint sin hoja PNG queda como fuente opcional")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
