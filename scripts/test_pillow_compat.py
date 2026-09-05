#!/usr/bin/env python3
"""Regresiones de compatibilidad Pillow para los generadores gráficos."""

from __future__ import annotations

import unittest

from pillow_compat import flattened_data


class PillowCompatTest(unittest.TestCase):
    def test_prefers_modern_flattened_api(self) -> None:
        class ModernImage:
            def get_flattened_data(self) -> tuple[int, ...]:
                return (3, 4)

            def getdata(self) -> tuple[int, ...]:
                raise AssertionError("no debe usar el fallback moderno")

        self.assertEqual(tuple(flattened_data(ModernImage())), (3, 4))

    def test_uses_getdata_when_legacy_pillow_has_no_modern_api(self) -> None:
        class LegacyImage:
            def getdata(self) -> tuple[int, ...]:
                return (7, 8)

        self.assertEqual(tuple(flattened_data(LegacyImage())), (7, 8))


if __name__ == "__main__":
    unittest.main()
