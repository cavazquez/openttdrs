#!/usr/bin/env python3
"""Auditoría SP3.0: PNG requeridos por el cliente vs assets/opengfx/tiles.

Detecta archivos ausentes y placeholders 1×1 (generados por descargar_graficos.sh
cuando falta el sprite en el NFO).

Uso:
  python3 scripts/audit_sp3_assets.py
  python3 scripts/audit_sp3_assets.py --json docs/SP3_AUDIT_REPORT.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
TILES_DIR = REPO_ROOT / "assets" / "opengfx" / "tiles"
CLIENT_SPRITES = REPO_ROOT / "crates" / "openttdrs-client" / "src" / "sprites"
RAIL_RS = CLIENT_SPRITES / "rail.rs"
HOUSE_GFX = REPO_ROOT / "crates" / "openttdrs-client" / "src" / "sprites" / "house_draw_data_generated.rs"
SPRITES_RS = REPO_ROOT / "crates" / "openttdrs-client" / "src" / "sprites.rs"
INDUSTRY_GFX = CLIENT_SPRITES / "industry_gfx_data_generated.rs"
FIXTURES = [
    REPO_ROOT / "crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap",
    REPO_ROOT / "crates/openttdrs-core/tests/fixtures/m3_road_tram_2x2.ottdmap",
    REPO_ROOT / "crates/openttdrs-core/tests/fixtures/v5p12_stxy.ottdmap",
    REPO_ROOT / "crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap",
    REPO_ROOT / "tests/fixtures/stationlist-test.ottdmap",
]

SPR_MAIN = 1275
SPR_ALT = 1352
SIGTYPE_LAST_NOPBS = 3


@dataclass
class AssetEntry:
    path: str
    category: str
    required: bool
    present: bool
    placeholder: bool
    width: int | None
    height: int | None


def signal_sprite_id(sig_type: int, variant: int, image: int, green: bool) -> int:
    cond = 1 if green else 0
    pbs_extra = 64 if sig_type > SIGTYPE_LAST_NOPBS else 0
    base = SPR_MAIN if sig_type == 0 and variant == 0 else SPR_ALT
    return (
        base
        + sig_type * 16
        + variant * 64
        + image * 2
        + cond
        + pbs_extra
    )


def signal_sprite_ids_for_preload() -> set[int]:
    """Réplica acotada de `signal_sprite_ids_for_preload` en `rail.rs`."""
    ids: set[int] = set()
    for rails in range(64):
        m5 = (1 << 6) | rails
        for present in range(1, 16):
            m3 = present << 4
            for states in range(16):
                m3hi = states << 4
                for ty in range(8):
                    for var in range(2):
                        for track in range(6):
                            base = 4 if track in (3, 5) else 0
                            var_bit = 7 if track in (3, 5) else 3
                            m2 = ((ty & 7) << base) | ((var & 1) << var_bit)
                            # Misma lógica que collect_signal_sprite_ids (simplificada vía fórmula)
                            rails_bits = rails & 0x3F
                            present_hi = (m3 >> 4) & 0xF
                            states_hi = (m3hi >> 4) & 0xF
                            if present_hi == 0:
                                continue
                            tb_x, tb_y = 1, 2
                            tb_u, tb_l, tb_le, tb_ri = 4, 8, 16, 32
                            pushes: list[tuple[int, int, int]] = []
                            if (rails_bits & tb_y) == 0:
                                if (rails_bits & tb_x) == 0:
                                    if rails_bits & tb_le:
                                        pushes += [(2, 7, 4), (3, 6, 4)]
                                    if rails_bits & tb_ri:
                                        pushes += [(0, 7, 5), (1, 6, 5)]
                                    if rails_bits & tb_u:
                                        pushes += [(3, 5, 2), (2, 4, 2)]
                                    if rails_bits & tb_l:
                                        pushes += [(1, 5, 3), (0, 4, 3)]
                                else:
                                    pushes += [(3, 0, 0), (2, 1, 0)]
                            else:
                                pushes += [(3, 2, 1), (2, 3, 1)]
                            for sig_bit, image, track_id in pushes:
                                if present_hi & (1 << sig_bit) == 0:
                                    continue
                                green = (states_hi >> sig_bit) & 1
                                st = (m2 >> (4 if track_id in (3, 5) else 0)) & 7
                                vr = (m2 >> (7 if track_id in (3, 5) else 3)) & 1
                                ids.add(signal_sprite_id(st, vr, image, bool(green)))
    return ids


RAIL_SPRITE_SNOW_OFFSET = 26
RAIL_TRACK_SLOPED_OFFSETS = [14, 15, 22, 13, 0, 21, 17, 12, 23, 0, 18, 20, 19, 16]


def rail_sprite_ids_for_preload() -> set[int]:
    m = re.search(
        r"pub const RAIL_SPRITE_IDS:\s*\[u32;\s*\d+\]\s*=\s*\[([^\]]+)\]",
        RAIL_RS.read_text(encoding="utf-8"),
        re.DOTALL,
    )
    if not m:
        raise RuntimeError("no se encontró RAIL_SPRITE_IDS en rail.rs")
    ids = {int(x) for x in re.findall(r"\d+", m.group(1))}
    for th in range(1, 15):
        offset = RAIL_TRACK_SLOPED_OFFSETS[th - 1]
        ids.add(1011 + offset + RAIL_SPRITE_SNOW_OFFSET)
    ids |= signal_sprite_ids_for_preload()
    gaps = {1438, 1439, 1530, 1532, 1540, 1542, 1546, 1548}
    return ids - gaps


def house_sprite_ids() -> set[int]:
    text = HOUSE_GFX.read_text(encoding="utf-8")
    ids: set[int] = set()
    for key in ("s1", "s2"):
        for m in re.finditer(rf"{key}: (\d+)", text):
            v = int(m.group(1))
            if v != 0:
                ids.add(v)
    return ids


def industry_sprite_ids() -> set[int]:
    text = INDUSTRY_GFX.read_text(encoding="utf-8")
    ids: set[int] = set()
    for key in ("sprite_id", "ground_sprite_id"):
        for m in re.finditer(rf"{key}:\s*(\d+)", text):
            v = int(m.group(1))
            if v != 0:
                ids.add(v)
    return ids


def house_sprite_filename(sprite_id: int) -> str:
    # Mismo naming que `house_sprite_filename` en crates/openttdrs-client/src/sprites.rs.
    return f"house_s{sprite_id}.png"


def collect_required_paths() -> list[tuple[str, str]]:
    """(ruta relativa desde repo root, categoría)."""
    out: list[tuple[str, str]] = []

    def add(rel: str, cat: str) -> None:
        out.append((rel, cat))

    add("assets/opengfx/tiles/grass.png", "terrain")
    add("assets/opengfx/tiles/grass_rough.png", "terrain")
    for tileh in range(1, 15):
        add(f"assets/opengfx/tiles/terrain_grass_slope_{tileh:02}.png", "terrain_slope")
        add(f"assets/opengfx/tiles/terrain_rough_slope_{tileh:02}.png", "terrain_slope")
        add(f"assets/opengfx/tiles/foundation_{tileh:02}.png", "foundation")
    add("assets/opengfx/tiles/water.png", "water")
    for i in range(18):  # set completo SPR_SHORE_BASE (gen_shore_full_set.py)
        add(f"assets/opengfx/tiles/shore_full_{i:02d}.png", "water")
    add("assets/opengfx/tiles/object_lighthouse.png", "object")
    add("assets/opengfx/tiles/object_transmitter.png", "object")
    for i in range(19):
        add(f"assets/opengfx/tiles/road_flat_{i:02}.png", "road")
        add(f"assets/opengfx/tiles/tram_flat_{i:02}.png", "tram")
    for rid in sorted(rail_sprite_ids_for_preload()):
        add(f"assets/opengfx/tiles/rail_{rid}.png", "rail")
    for i in range(4):
        add(f"assets/opengfx/tiles/truck_stop_ground_{i}.png", "station")
    for d in ("ne", "se", "sw", "nw"):
        add(f"assets/opengfx/tiles/bus_stop_{d}_ground.png", "station")
        for layer in ("a", "b", "c"):
            add(f"assets/opengfx/tiles/bus_stop_{d}_build_{layer}.png", "station")
            add(f"assets/opengfx/tiles/truck_stop_{d}_build_{layer}.png", "station")
    for p in (
        "assets/opengfx/tiles/rail_1412.png",
        "assets/opengfx/tiles/road_depot_1.png",
        "assets/opengfx/tiles/road_depot_3.png",
        "assets/opengfx/tiles/rail_1413.png",
        "assets/opengfx/tiles/rail_1413.png",
        "assets/opengfx/tiles/rail_depot_ne.png",
        "assets/opengfx/tiles/tunnel_road_rear.png",
        "assets/opengfx/tiles/tunnel_rail_rear.png",
        "assets/opengfx/tiles/bridge_wood_road_x.png",
        "assets/opengfx/tiles/bridge_wood_rail_x.png",
    ):
        add(p, "transport_object")
    for sid in sorted(house_sprite_ids()):
        add(f"assets/opengfx/tiles/{house_sprite_filename(sid)}", "house")
    for i in range(133):  # 19 especies × 7 etapas (gen_tree_draw_data.py)
        add(f"assets/opengfx/tiles/tree_{i:02d}.png", "forest")
    for state in range(9):  # 9 estados × 19 pendientes (gen_field_draw_data.py)
        for off in range(19):
            add(f"assets/opengfx/tiles/field_{state}_{off:02d}.png", "fields")
    for ftype in range(6):  # 6 tipos de cerca × 6 variantes
        for var in range(6):
            add(f"assets/opengfx/tiles/fence_{ftype}_{var}.png", "fields")
    for f in range(15):  # frames del ciclo de paleta (gen_water_anim_frames.py)
        add(f"assets/opengfx/tiles/water_anim_{f:02d}.png", "water")
        for i in range(18):
            add(f"assets/opengfx/tiles/shore_full_{i:02d}_anim_{f:02d}.png", "water")
    for i in range(8):  # humo de chimenea (gen_chimney_smoke.py)
        add(f"assets/opengfx/tiles/chimney_smoke_{i}.png", "industry")
    for iid in sorted(industry_sprite_ids()):
        add(f"assets/opengfx/tiles/industry_{iid}.png", "industry")
    return out


def png_size(path: Path) -> tuple[int, int] | None:
    try:
        from PIL import Image
    except ImportError:
        return None
    with Image.open(path) as img:
        return img.size


def is_placeholder(path: Path) -> bool:
    size = png_size(path)
    if size is None:
        # Sin Pillow: heurística por tamaño de archivo mínimo
        return path.stat().st_size < 120
    w, h = size
    return w <= 1 and h <= 1


def audit() -> dict:
    required = collect_required_paths()
    entries: list[AssetEntry] = []
    by_cat: dict[str, dict[str, int]] = {}

    for rel, cat in required:
        full = REPO_ROOT / rel
        present = full.is_file()
        ph = is_placeholder(full) if present else False
        w = h = None
        if present and png_size(full):
            w, h = png_size(full)
        entries.append(
            AssetEntry(
                path=rel,
                category=cat,
                required=True,
                present=present,
                placeholder=ph,
                width=w,
                height=h,
            )
        )
        bucket = by_cat.setdefault(cat, {"required": 0, "missing": 0, "placeholder": 0})
        bucket["required"] += 1
        if not present:
            bucket["missing"] += 1
        elif ph:
            bucket["placeholder"] += 1

    extra_rails = []
    if TILES_DIR.is_dir():
        for p in sorted(TILES_DIR.glob("rail_*.png")):
            rel = f"assets/opengfx/tiles/{p.name}"
            if rel not in {e.path for e in entries}:
                extra_rails.append(rel)

    fixtures = []
    for f in FIXTURES:
        fixtures.append(
            {
                "path": str(f.relative_to(REPO_ROOT)),
                "present": f.is_file(),
                "size_bytes": f.stat().st_size if f.is_file() else None,
            }
        )

    ref = REPO_ROOT / "reference" / "openttd-upstream"
    upstream = {
        "path": str(ref.relative_to(REPO_ROOT)),
        "present": (ref / ".git").is_dir(),
    }

    missing = [e.path for e in entries if not e.present]
    placeholders = [e.path for e in entries if e.present and e.placeholder]

    return {
        "tiles_dir": str(TILES_DIR.relative_to(REPO_ROOT)),
        "tiles_dir_exists": TILES_DIR.is_dir(),
        "summary": {
            "required_total": len(entries),
            "missing": len(missing),
            "placeholder": len(placeholders),
            "ok": len(entries) - len(missing) - len(placeholders),
        },
        "by_category": by_cat,
        "missing": missing,
        "placeholders": placeholders,
        "extra_rail_pngs_not_in_preload": extra_rails,
        "fixtures": fixtures,
        "upstream_reference": upstream,
        "entries": [asdict(e) for e in entries],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        type=Path,
        default=None,
        help="Escribir informe JSON (p. ej. docs/SP3_AUDIT_REPORT.json)",
    )
    parser.add_argument("--quiet", action="store_true", help="Solo código de salida")
    args = parser.parse_args()

    report = audit()
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, indent=2), encoding="utf-8")

    if not args.quiet:
        s = report["summary"]
        print("SP3.0 — auditoría de assets OpenGFX")
        print(f"  Directorio: {report['tiles_dir']} (existe: {report['tiles_dir_exists']})")
        print(
            f"  Requeridos: {s['required_total']}  OK: {s['ok']}  "
            f"Faltan: {s['missing']}  Placeholder 1×1: {s['placeholder']}"
        )
        if report["upstream_reference"]["present"]:
            print(f"  Referencia upstream: {report['upstream_reference']['path']} ✓")
        else:
            print(
                "  Referencia upstream: no clonada "
                "(ejecutar: bash scripts/fetch-openttd-reference.sh)"
            )
        print("\n  Por categoría:")
        for cat, stats in sorted(report["by_category"].items()):
            print(
                f"    {cat:16} required={stats['required']:4}  "
                f"missing={stats['missing']:3}  placeholder={stats['placeholder']:3}"
            )
        if report["missing"]:
            print("\n  Faltan (primeros 20):")
            for p in report["missing"][:20]:
                print(f"    - {p}")
            if len(report["missing"]) > 20:
                print(f"    ... y {len(report['missing']) - 20} más")
        if report["placeholders"]:
            print("\n  Placeholders 1×1 (primeros 20):")
            for p in report["placeholders"][:20]:
                print(f"    - {p}")
            if len(report["placeholders"]) > 20:
                print(f"    ... y {len(report['placeholders']) - 20} más")
        print("\n  Fixtures .ottdmap:")
        for fx in report["fixtures"]:
            mark = "✓" if fx["present"] else "✗"
            print(f"    {mark} {fx['path']}")
        if args.json:
            print(f"\n  Informe JSON: {args.json}")

    s = report["summary"]
    return 1 if s["missing"] or s["placeholder"] else 0


if __name__ == "__main__":
    sys.exit(main())
