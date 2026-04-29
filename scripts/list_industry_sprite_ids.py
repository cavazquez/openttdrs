#!/usr/bin/env python3
"""Lista IDs OpenGFX únicos para PNG `industry_<id>.png` según industry_gfx_data_generated.rs."""
from __future__ import annotations

import re
import sys
from pathlib import Path


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    path = repo / "crates" / "openttdrs-client" / "src" / "sprites" / "industry_gfx_data_generated.rs"
    if len(sys.argv) >= 2:
        path = Path(sys.argv[1])
    text = path.read_text(encoding="utf-8")
    ids = set(int(x) for x in re.findall(r"ground_sprite_id:\s*(\d+)", text))
    ids |= set(int(x) for x in re.findall(r"sprite_id:\s*(\d+)", text))
    ids.discard(0)
    for i in sorted(ids):
        print(i)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
