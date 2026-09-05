#!/usr/bin/env python3
"""Contrato de los 74 `StationGfx` airport contra OpenTTD versionado.

Evita que una simplificación vuelva a convertir un aeropuerto cargado en una
colección de aprons/torres genéricas. Se fijan bases, capas, Action5 y los
cinco StationGfx animados que define ``station_land.h``.
"""

from __future__ import annotations

import re
import tempfile
import unittest
from pathlib import Path

import gen_airport_station_draw_data as generator


ROOT = Path(__file__).resolve().parents[1]
VENDORED_SOURCE = ROOT / "third_party" / "openttd" / "station_land.h"
UPSTREAM_SOURCE = ROOT / "reference" / "openttd-upstream" / "src" / "table" / "station_land.h"


def contract_sources(root: Path = ROOT) -> tuple[Path, ...]:
    """Devuelve la copia versionada y, si existe, el checkout opcional del oracle.

    Actions no descarga ``reference/openttd-upstream``; la copia GPL
    versionada es por tanto la fuente mínima y reproducible del contrato. En
    una estación de desarrollo se valida además el checkout real, sin hacer
    del clone local un requisito implícito del CI.
    """
    vendored = root / "third_party" / "openttd" / "station_land.h"
    upstream = root / "reference" / "openttd-upstream" / "src" / "table" / "station_land.h"
    sources = [vendored]
    if upstream.is_file():
        sources.append(upstream)
    return tuple(sources)


def compact(text: str) -> str:
    """Quita diferencias de formato irrelevantes de las tablas C++."""

    return re.sub(r"\s+", "", text)


class AirportStationSourceContractTest(unittest.TestCase):
    def test_source_selection_has_a_versioned_ci_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            vendored = root / "third_party" / "openttd" / "station_land.h"
            vendored.parent.mkdir(parents=True)
            vendored.write_text("fixture", encoding="utf-8")
            self.assertEqual(contract_sources(root), (vendored,))

    def test_checked_in_generator_contract_matches_openttd(self) -> None:
        bases = {gfx: (sprite, company) for gfx, _label, sprite, company in generator.AIRPORT_STATION_BASES}
        self.assertEqual(set(bases), set(range(74)))
        self.assertEqual(len(bases), 74)
        self.assertEqual(bases[19], (2634, False))  # terminal A sobre apron
        self.assertEqual(bases[20], (3981, False))  # torre sobre grass
        self.assertEqual(bases[35], (2667, True))   # terminal airfield CC
        self.assertEqual(bases[44], (3981, False))  # heliport sobre grass
        self.assertEqual(bases[71], (2634, False))  # mitad de apron Action5
        self.assertEqual(bases[73], (3981, False))  # flag sobre grass

        overlays = {}
        for entry in generator.AIRPORT_STATION_OVERLAYS:
            overlays.setdefault(entry[0], []).append(entry)
        self.assertEqual(overlays[26][0][2:9], (2660, 2, 7, 0, 3, 3, 14))
        self.assertEqual(overlays[27][0][2:9], (2661, 3, 2, 0, 3, 3, 14))
        self.assertEqual(overlays[28][0][2:9], (2662, 0, 8, 0, 14, 3, 14))
        self.assertEqual(overlays[31][0][2], 2680)  # radar frame base
        self.assertEqual(overlays[39][-1][2], 2676) # wind frame base
        self.assertEqual(overlays[44][0][2], 2633)
        self.assertEqual(overlays[47][0][2], 2651)  # torre estática
        self.assertEqual(overlays[53][0][2], 4982)  # Action5 helipad
        self.assertEqual(overlays[66][0][2], 5966)  # Action5 new helipad
        self.assertEqual(overlays[71][0][2], 5968)
        self.assertEqual(overlays[72][0][2], 5967)
        self.assertEqual(overlays[73][-1][2], 2676)

        ground = {}
        for entry in generator.AIRPORT_STATION_GROUND_LAYERS:
            ground.setdefault(entry[0], []).append(entry)
        self.assertEqual(ground[1], [(1, "APT_APRON_FENCE_NW", 2664, 0, 0, 0)])
        self.assertEqual(ground[2], [(2, "APT_APRON_FENCE_SW", 2663, 15, 0, 0)])
        self.assertEqual(
            ground[49],
            [(49, "APT_RUNWAY_END_FENCE_NW", 2664, 0, 0, 0)],
        )
        self.assertEqual(
            ground[50],
            [(50, "APT_RUNWAY_FENCE_NW", 2664, 0, 0, 0)],
        )
        self.assertEqual(ground[56][-1], (56, "APT_APRON_FENCE_NE_SW", 2663, 15, 0, 0))
        self.assertEqual(ground[70][-1], (70, "APT_APRON_FENCE_NE_SE", 2664, 0, 15, 0))
        self.assertEqual(set(generator.DYNAMIC_SPRITE_IDS), set(range(2676, 2692)))

    def test_openttd_station_land_defines_the_same_ground_and_tile_seq(self) -> None:
        self.assertTrue(VENDORED_SOURCE.is_file(), f"falta copia versionada: {VENDORED_SOURCE}")
        for source in contract_sources():
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
                    "TILE_SPRITE_NULL()//APT_RADAR_GRASS_FENCE_SW",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_NULL()//APT_GRASS_FENCE_NE_FLAG",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_NULL()//APT_GRASS_FENCE_NE_FLAG_2",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_FLAT_GRASS_TILE,_station_display_heliport)//APT_HELIPORT",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_AIRPORT_APRON,_station_display_tower)//APT_TOWER",
                    text,
                )
                self.assertIn(
                    "_station_display_fence_nw[]={"
                    "TILE_SEQ_GROUND(0,0,0,SPR_AIRPORT_FENCE_X|(1U<<PALETTE_MODIFIER_COLOUR))"
                    "//fencesnorth};",
                    text,
                )
                self.assertIn(
                    "_station_display_fence_sw[]={"
                    "TILE_SEQ_GROUND(15,0,0,SPR_AIRPORT_FENCE_Y|(1U<<PALETTE_MODIFIER_COLOUR))"
                    "//fenceswest};",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_AIRPORT_APRON,_station_display_fence_nw)//APT_APRON_FENCE_NW",
                    text,
                )
                self.assertIn(
                    "TILE_SPRITE_LINE(SPR_AIRPORT_APRON,_station_display_fence_sw)//APT_APRON_FENCE_SW",
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
