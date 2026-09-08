#!/usr/bin/env python3
"""Run OpenTTD's literal GetCurrentMaxSpeed body for road acceleration models.

The supplied native checkout provides GetCurrentMaxSpeed and the reversing
predicate. Only vehicle storage, bridge lookup and settings are stubbed. This
oracle covers one road vehicle's speed ceiling, not its movement controller.
"""

import argparse
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/road_curve_speed.tsv"

PREAMBLE = r"""
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <iostream>
using Trackdir = uint8_t;
constexpr int AM_REALISTIC = 1, RVSB_TRACKDIR_MASK = 0x0f, RVSB_WORMHOLE = 0xff;
enum class VehState { Hidden };
struct Flags { bool Test(VehState) const { return true; } };
struct Settings { struct { int roadveh_acceleration_model; } vehicle; } _settings_game;
struct BridgeSpec { int speed = 65535; } bridge;
const BridgeSpec *GetBridgeSpec(int) { return &bridge; }
int GetBridgeType(int) { return 0; }
bool IsValidTrackdirForRoadVehicle(Trackdir dir) { return dir <= 15; }
struct RoadVehicle {
    struct { int cached_max_track_speed; } gcache;
    int state, direction, tile = 0;
    Flags vehstatus;
    struct { int speed; int GetMaxSpeed() const { return speed; } } current_order;
    const RoadVehicle *Next() const { return nullptr; }
    int GetCurrentMaxSpeed() const;
};
"""

MAIN = r"""
int main() {
    std::cout << "model\tstate\tdirection\ttrack_max\torder_max_internal\tmax_speed\n";
    for (int model : {0, 1})
    for (int state : {0, 6, 7, 14, 15, 0x26, 0x2e, 0xff})
    for (int direction : {1, 2})
    for (int max_speed : {101, 112})
    for (int order_speed : {30, 65535}) {
        _settings_game.vehicle.roadveh_acceleration_model = model;
        RoadVehicle v{{max_speed}, state, direction, 0, {}, {order_speed}};
        std::cout << model << '\t' << state << '\t' << direction << '\t' << max_speed
                  << '\t' << order_speed * 2 << '\t' << v.GetCurrentMaxSpeed() << '\n';
    }
}
"""


def extract_function(source, signature):
    start = source.index(signature)
    body = source.index("{", start)
    depth = 1
    end = body + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end] + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="OpenTTD checkout root")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    speed = extract_function(
        (args.source / "src/roadveh_cmd.cpp").read_text(),
        "inline int RoadVehicle::GetCurrentMaxSpeed() const",
    )
    reversing = extract_function(
        (args.source / "src/track_func.h").read_text(),
        "inline bool IsReversingRoadTrackdir(Trackdir dir)",
    )
    with tempfile.TemporaryDirectory(prefix="road-curve-speed-oracle-") as directory:
        source_file = Path(directory) / "oracle.cpp"
        binary = Path(directory) / "oracle"
        source_file.write_text(PREAMBLE + reversing + speed + MAIN)
        subprocess.run(["c++", "-std=c++20", str(source_file), "-o", str(binary)], check=True)
        output = subprocess.check_output([str(binary)], text=True)
    if args.check:
        if output != FIXTURE.read_text():
            raise SystemExit("native road speed ceilings differ from the committed fixture")
        print(f"OK: {len(output.splitlines()) - 1} native road speed ceilings")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
