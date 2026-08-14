#!/usr/bin/env python3
"""Impide que el oráculo C++ vuelva a degradar ``world-semantic`` a v1."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "patches" / "openttd-15.3-snapshot-export"


def compact(text: str) -> str:
    return re.sub(r"\s+", "", text)


class WorldSemanticExportSourceContractTest(unittest.TestCase):
    def test_cpp_oracle_matches_the_documented_v2_object_contract(self) -> None:
        source = compact((PATCH / "src" / "world_semantic_export.cpp").read_text(encoding="utf-8"))
        header = (PATCH / "src" / "world_semantic_export.h").read_text(encoding="utf-8")
        readme = (PATCH / "README.md").read_text(encoding="utf-8")

        self.assertIn('#include"object_map.h"', source)
        self.assertIn('metadata["schema_version"]=2;', source)
        self.assertIn('{"object_id",object_id},{"object_type",static_cast<uint32_t>(GetObjectType(tile))}', source)
        self.assertIn("`world-semantic` v2", header)
        self.assertIn("`world-semantic` v2", readme)


if __name__ == "__main__":
    unittest.main()
