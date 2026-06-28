#!/usr/bin/env python3
"""Split command/transport.rs into transport/ submodules."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/openttdrs-core/src/command/transport.rs"
OUT = ROOT / "crates/openttdrs-core/src/command/transport"

IMPORTS = """use crate::map::{
    Map, TileCoord, TileKind, complement_slope, inclined_slope_direction,
    rail_trackbits_valid_on_slope, resolve_tunnel_end, tile_slope_and_z, tunnel_entrance_m5,
    tunnel_path_tiles, tunnel_preview_path,
};
use crate::rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SIGNAL_BUILD_COST,
    SIGNAL_REMOVE_REFUND, rail_signal_present_mask, rail_signal_state_mask, rail_tile_is_signals,
};
use crate::station::is_rail_waypoint_tile;
use crate::pathfinder::{
    diag_dir_offset, station_entrance_faces_rail, station_entrance_faces_road,
    station_site_tile_allows_build, station_site_tile_needs_clear,
};
use crate::{
    CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST, ROAD_BUILD_COST,
    STATION_BUILD_COST, Station, StopKind, WAYPOINT_BUILD_COST,
};

use super::super::terraform::{apply_autoslope_if_needed, check_autoslope_flat};
use super::super::{CommandError, in_bounds};

#[allow(unused_imports)]
use crate::command::transport::internal::*;

"""

RAIL_CONST_PREFIX = ("RAIL_", "MP_RAILWAY")
ROAD_CONSTS = {"ROAD_PLACE_FORCE_AXIS", "ROAD_LINK_TO_NEIGHBOR"}
BRIDGE_CONSTS = {"BRIDGE_AXIS_Y_M5"}


def parse_items(text: str) -> list[str]:
    lines = text.splitlines(keepends=True)
    # skip imports until first item
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("use ") or (s.startswith("///") and i < 30):
            i += 1
            continue
        if (
            re.match(r"^(pub )?const ", s)
            or re.match(r"^pub(?:\([^)]*\))? fn ", s)
            or re.match(r"^pub const fn ", s)
            or re.match(r"^fn ", s)
        ):
            break
        i += 1

    items: list[str] = []
    while i < len(lines):
        line = lines[i]
        if not (
            re.match(r"^(pub )?const ", line.strip())
            or re.match(r"^pub(?:\([^)]*\))? fn ", line)
            or re.match(r"^pub const fn ", line)
            or re.match(r"^fn ", line)
        ):
            i += 1
            continue
        start = i
        if line.strip().startswith("const ") or line.strip().startswith("pub const "):
            if " fn " not in line:
                if line.strip().endswith(";"):
                    i += 1
                    items.append("".join(lines[start:i]))
                    continue
                i += 1
                while i < len(lines) and not lines[i].strip().endswith(";"):
                    i += 1
                i += 1
                items.append("".join(lines[start:i]))
                continue
        depth = 0
        started = False
        while i < len(lines):
            for ch in lines[i]:
                if ch == "{":
                    depth += 1
                    started = True
                elif ch == "}":
                    depth -= 1
            i += 1
            if started and depth == 0:
                break
        items.append("".join(lines[start:i]))
    return items


def item_name(block: str) -> str:
    m = re.search(r"fn (\w+)", block)
    if m:
        return m.group(1)
    m = re.search(r"pub const (\w+)", block)
    if m:
        return m.group(1)
    m = re.search(r"^const (\w+)", block, re.M)
    return m.group(1) if m else "unknown"


def classify(name: str, block: str) -> str:
    if name in BRIDGE_CONSTS or name.startswith(
        ("check_bridge", "check_tunnel", "place_tunnel", "check_tunnel_or_bridge")
    ):
        return "bridge"
    if name.startswith(
        (
            "check_station",
            "place_station",
            "place_stop",
            "station_",
            "clear_station",
            "ottd_station",
            "apply_station",
            "rail_station",
            "check_rail_station",
            "place_rail_station",
            "check_place_rail_waypoint",
            "place_rail_waypoint",
            "rail_waypoint",
        )
    ) or name == "rail_station_footprint":
        return "station"
    if name.startswith(RAIL_CONST_PREFIX) or (
        "rail" in name.lower()
        and "road" not in name.lower()
        and name not in ("station_entrance_faces_rail",)
    ):
        if name.startswith(("road_", "check_place_road", "place_road", "set_road", "merge_road", "connect_road", "propagate_road")):
            return "road"
        return "rail"
    if name in ROAD_CONSTS or "road" in name.lower() or name in (
        "check_place_road_bits",
        "finalize_road_drag_line",
        "infer_road_drag_axis",
        "preview_road_bits_at",
        "road_bits_for_autoroute",
        "road_drag_line_tiles",
        "road_locked_tool_axis",
    ):
        return "road"
    if name in (
        "check_in_bounds",
        "check_single_transport_tile",
        "check_clear_tile",
        "clear_tile",
        "transport_tile_is_buildable",
        "build_error_for_kind",
        "place_single_transport_tile",
        "axis_line",
    ):
        return "shared"
    return "shared"


def main() -> None:
    text = SRC.read_text()
    items = parse_items(text)
    modules: dict[str, list[str]] = {k: [] for k in ("shared", "road", "rail", "bridge", "station")}

    for block in items:
        name = item_name(block)
        mod = classify(name, block)
        # Visibilidad: interno del submódulo vs resto de `command`.
        block = re.sub(
            r"^pub\(super\) fn ",
            "pub(in crate::command) fn ",
            block,
            count=1,
            flags=re.M,
        )
        block = re.sub(r"^fn ", "pub(in crate::command::transport) fn ", block, count=1, flags=re.M)
        block = re.sub(
            r"^const ",
            "pub(in crate::command::transport) const ",
            block,
            count=1,
            flags=re.M,
        )
        modules[mod].append(block)

    OUT.mkdir(exist_ok=True)
    for mod, blocks in modules.items():
        (OUT / f"{mod}.rs").write_text(IMPORTS + "\n".join(blocks) + "\n")

    (OUT / "internal.rs").write_text(
        """//! Re-export interno para llamadas entre submódulos de transporte.
pub(in crate::command::transport) use super::bridge::*;
pub(in crate::command::transport) use super::rail::*;
pub(in crate::command::transport) use super::road::*;
pub(in crate::command::transport) use super::shared::*;
pub(in crate::command::transport) use super::station::*;
"""
    )

    (OUT / "mod.rs").write_text(
        """//! Comandos de transporte: carretera, vía, estaciones, puentes y túneles.

mod bridge;
mod internal;
mod rail;
mod road;
mod shared;
mod station;

pub use road::{
    ROAD_PLACE_FORCE_AXIS, finalize_road_drag_line, infer_road_drag_axis, preview_road_bits_at,
    road_bits_for_autoroute, road_drag_line_tiles, road_locked_tool_axis,
};
pub use rail::{rail_bits_placement_target, rail_trackbits_from_neighbors};
pub use station::rail_station_footprint;
pub(crate) use rail::{
    bridge_collinear_rail_gaps, normalize_rail_trackbits_from_neighbors,
    normalize_synthetic_rail_crossings,
};

pub(in crate::command) use bridge::*;
pub(in crate::command) use rail::*;
pub(in crate::command) use road::*;
pub(in crate::command) use shared::*;
pub(in crate::command) use station::*;
"""
    )

    SRC.unlink()
    for mod, blocks in modules.items():
        print(f"{mod}: {len(blocks)} items")


if __name__ == "__main__":
    main()
