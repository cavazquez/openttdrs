#!/usr/bin/env python3
"""Genera la lista de PNG con paleta de compañía para el cliente.

Lee paths de los `.rs` generados (vehículos, paradas, depósitos) y emite
`company_palette_paths_generated.rs`.

Uso: python3 scripts/gen_company_palette_paths.py
"""
from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "crates/openttdrs-client/src/sprites"
OUT = SPRITES / "company_palette_paths_generated.rs"

PATH_RE = re.compile(r'path:\s*"assets/opengfx/tiles/([^"]+)"')


def paths_from_file(path: Path) -> list[str]:
    if not path.is_file():
        return []
    return sorted(set(PATH_RE.findall(path.read_text(encoding="utf-8", errors="replace"))))


def main() -> None:
    sources = [
        SPRITES / "vehicle_gfx_data_generated.rs",
        SPRITES / "road_stop_gfx_data_generated.rs",
        SPRITES / "road_depot_gfx_data_generated.rs",
    ]
    paths: set[str] = set()
    for src in sources:
        paths.update(paths_from_file(src))
    # Depósitos de vía (paths en rail.rs, no en generated depot file).
    rail_rs = (SPRITES / "rail.rs").read_text(encoding="utf-8", errors="replace")
    paths.update(PATH_RE.findall(rail_rs))
    # Excluir suelos neutros (sin PALETTE_MODIFIER_COLOUR en upstream).
    paths.discard("road_depot_ground.png")
    ordered = sorted(paths)
    lines = [
        "// Generado por scripts/gen_company_palette_paths.py — NO EDITAR A MANO.",
        "",
        f"pub static COMPANY_PALETTE_STATIC_PATHS: [&str; {len(ordered)}] = [",
    ]
    for p in ordered:
        lines.append(f'    "{p}",')
    lines += ["];", ""]
    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT.relative_to(REPO)} ({len(ordered)} paths)")


if __name__ == "__main__":
    main()
