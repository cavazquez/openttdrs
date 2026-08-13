#!/usr/bin/env python3
"""Contrato de los piers vanilla contra la fuente OpenTTD versionada.

Evita que un cambio de extracción convierta los StationGfx 27/28 en un
concourse centrado: OpenTTD los dibuja como apron más una capa TILE_SEQ.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import gen_airport_station_draw_data as generator


ROOT = Path(__file__).resolve().parents[1]
SOURCES = (
    ROOT / "reference" / "openttd-upstream" / "src" / "table" / "station_land.h",
    ROOT / "third_party" / "openttd" / "station_land.h",
)


def compact(text: str) -> str:
    """Quita diferencias de formato irrelevantes de las tablas C++."""

    return re.sub(r"\s+", "", text)


class AirportStationSourceContractTest(unittest.TestCase):
    def test_checked_in_generator_contract_matches_openttd(self) -> None:
        self.assertEqual(
            generator.AIRPORT_STATION_OVERLAYS,
            (
                (27, "APT_PIER_NW_NE", 2661, 3, 2, 0, 3, 3, 14, "airport_jetway_3.png"),
                (28, "APT_PIER", 2662, 0, 8, 0, 14, 3, 14, "airport_passenger_tunnel.png"),
            ),
        )

    def test_openttd_station_land_defines_the_same_ground_and_tile_seq(self) -> None:
        for source in SOURCES:
            with self.subTest(source=source):
                self.assertTrue(source.is_file(), f"falta fuente OpenTTD: {source}")
                text = compact(source.read_text(encoding="utf-8"))
                self.assertIn(
                    "_station_display_jetway_3[]={"
                    "TILE_SEQ_LINE(3,2,0,3,3,14,"
                    "SPR_AIRPORT_JETWAY_3|(1U<<PALETTE_MODIFIER_COLOUR))"
                    "};",
                    text,
                )
                self.assertIn(
                    "_station_display_passenger_tunnel[]={"
                    "TILE_SEQ_LINE(0,8,0,14,3,14,"
                    "SPR_AIRPORT_PASSENGER_TUNNEL|(1U<<PALETTE_MODIFIER_COLOUR))"
                    "};",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_AIRPORT_APRON,"
                    "_station_display_jetway_3)//APT_PIER_NW_NE",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_AIRPORT_APRON,"
                    "_station_display_passenger_tunnel)//APT_PIER",
                    text,
                )


if __name__ == "__main__":
    unittest.main()
