#!/usr/bin/env python3
"""Split command/tests.rs into command/tests/ submodules by domain."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/openttdrs-core/src/command/tests.rs"
OUT = ROOT / "crates/openttdrs-core/src/command/tests"

IMPORTS = """use crate::command::{
    Command, CommandError, apply_command, command_error_message, command_would_fail,
};
use crate::{
    BRIDGE_BUILD_COST_PER_TILE, CLEAR_TILE_COST, GameState, IndustryKind, IndustrySpec, LevelMode,
    RAIL_BUILD_COST, ROAD_BUILD_COST, ROAD_PLACE_FORCE_AXIS, STATION_BUILD_COST,
    STATION_TYPE_RAIL_WAYPOINT, StopKind, TERRAFORM_COST, TileCoord, TileKind, Vehicle,
    VehicleKind, VehicleOrder, WAYPOINT_BUILD_COST, industry_template, infer_road_drag_axis,
    pathfinder, road_bits_for_autoroute, station_type_from_m6, tile_slope_and_z,
};

"""

HELPERS = {
    "set_w_only_slope",
    "train_with_cached_path_to_depot",
    "finish_train_with_cached_path_to_depot",
    "flat_map_for_terraform_tests",
}


def classify_test(name: str) -> str:
    if name in HELPERS:
        return "helpers"
    if any(
        name.startswith(p)
        for p in ("road_", "place_road", "set_road", "infer_road", "generic_inferred")
    ):
        return "road"
    if "road" in name and "rail" not in name and "station" not in name:
        return "road"
    if any(
        name.startswith(p)
        for p in (
            "rail_",
            "place_rail",
            "remove_rail",
            "parallel_",
            "autorail",
            "disconnecting_rail",
            "terraform_",
            "failed_terraform",
            "train_",
            "build_vehicle",
            "sell_vehicle",
        )
    ) or "rail" in name or "signal" in name or "waypoint" in name:
        return "rail"
    if "station" in name or name.startswith(("place_bus", "place_truck")):
        return "station"
    if "bridge" in name or "tunnel" in name:
        return "bridge"
    return "misc"


def parse_tests(text: str) -> list[tuple[str, str]]:
    parts = re.split(r"\n(?=#\[test\])", text)
    tests: list[tuple[str, str]] = []
    for part in parts:
        part = part.strip()
        if not part.startswith("#[test]"):
            continue
        m = re.search(r"fn (\w+)", part)
        if not m:
            continue
        tests.append((m.group(1), part + "\n"))
    return tests


def parse_helpers(text: str) -> list[str]:
    """Non-test helper fns after the last #[test] block."""
    last_test = text.rfind("#[test]")
    if last_test == -1:
        return []
    tail = text[last_test:]
    # find helpers defined after tests in file - scan full file for fn without #[test] before
    helpers: list[str] = []
    lines = text.splitlines(keepends=True)
    i = 0
    while i < len(lines):
        if lines[i].strip().startswith("#[test]"):
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("#[test]"):
                i += 1
            continue
        if re.match(r"^fn ", lines[i]):
            m = re.search(r"fn (\w+)", lines[i])
            if m and m.group(1) in HELPERS:
                start = i
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
                helpers.append("".join(lines[start:i]))
            else:
                i += 1
        else:
            i += 1
    return helpers


def main() -> None:
    text = SRC.read_text()
    tests = parse_tests(text)
    helpers = parse_helpers(text)
    modules: dict[str, list[str]] = {
        k: [] for k in ("helpers", "road", "rail", "station", "bridge", "misc")
    }
    modules["helpers"].extend(helpers)

    for name, block in tests:
        modules[classify_test(name)].append(block)

    OUT.mkdir(exist_ok=True)
    (OUT / "helpers.rs").write_text(IMPORTS + "\n".join(modules["helpers"]) + "\n")
    helper_use = "use super::helpers::{finish_train_with_cached_path_to_depot, flat_map_for_terraform_tests, set_w_only_slope, train_with_cached_path_to_depot};\n\n"
    for mod in ("road", "rail", "station", "bridge", "misc"):
        extra = helper_use if mod in ("rail", "misc") else ""
        (OUT / f"{mod}.rs").write_text(IMPORTS + extra + "\n".join(modules[mod]) + "\n")
        print(f"{mod}: {len(modules[mod])}")

    (OUT / "mod.rs").write_text(
        "mod bridge;\nmod helpers;\nmod misc;\nmod rail;\nmod road;\nmod station;\n"
    )
    print(f"helpers: {len(modules['helpers'])}")
    SRC.unlink()


if __name__ == "__main__":
    main()
