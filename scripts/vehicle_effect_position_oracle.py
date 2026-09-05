#!/usr/bin/env python3
"""Execute OpenTTD's unmodified CB160 positioning function with captured spawns.

Only callback inputs and effect allocation are stubbed. The positioning body
and direction table are extracted from the supplied OpenTTD 15.3 checkout.
Print TSV to stdout; --check compares it with the committed regression fixture.
"""

import argparse
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/openttdrs-client/tests/fixtures/vehicle_effect_positions.tsv"

PREAMBLE = r"""
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
using uint = unsigned;
using Direction = uint8_t;
constexpr int VEH_TRAIN = 0, VEH_ROAD = 1, VEH_SHIP = 2, VEH_AIRCRAFT = 3;
constexpr int VEHICLE_LENGTH = 8, DIRDIFF_90RIGHT = 2;
constexpr int CBID_VEHICLE_SPAWN_VISUAL_EFFECT = 0x160, CALLBACK_FAILED = 0xffff;
enum EffectVehicleType { EV_STEAM_SMOKE, EV_DIESEL_SMOKE, EV_ELECTRIC_SPARK,
                         EV_BREAKDOWN_SMOKE_AIRCRAFT };
enum class VehicleRailFlag { Flipped };
struct Flags { bool flipped; bool Test(VehicleRailFlag) const { return flipped; } };
struct Vehicle {
    int type, engine_type = 0;
    Direction direction;
    struct { int cached_veh_length; } gcache;
    Flags flags;
};
struct RoadVehicle { static const Vehicle *From(const Vehicle *v) { return v; } };
struct Train { static const Vehicle *From(const Vehicle *v) { return v; } };
uint GB(uint value, uint shift, uint bits) { return (value >> shift) & ((1U << bits) - 1); }
bool HasBit(uint value, uint bit) { return (value & (1U << bit)) != 0; }
Direction ReverseDir(Direction d) { return (d + 4) & 7; }
Direction ChangeDir(Direction d, int delta) { return (d + delta) & 7; }
uint Random() { return 0; }
uint16_t result;
int32_t spawn_register;
uint16_t GetVehicleCallback(int, int, uint, int, const Vehicle *, std::array<int32_t, 4> &regs) {
    regs.fill(spawn_register);
    return result;
}
void CreateEffectVehicleRel(const Vehicle *, int x, int y, int z, EffectVehicleType) {
    std::cout << '\t' << x << '\t' << y << '\t' << z << '\n';
}
"""

MAIN = r"""
int main() {
    std::cout << "kind\tdirection\tlength\tflipped\tcenter\trotate\tx\ty\tz\tout_x\tout_y\tout_z\n";
    for (int kind = 0; kind < 4; ++kind)
    for (int dir = 0; dir < 8; ++dir)
    for (int length : {4, 8})
    for (int flipped = 0; flipped <= (kind == VEH_TRAIN ? 1 : 0); ++flipped)
    for (int center = 0; center < 2; ++center)
    for (int rotate = 0; rotate < 2; ++rotate)
    for (auto xyz : {std::array<int, 3>{2, 3, -4}, std::array<int, 3>{127, -128, 10}}) {
        Vehicle v{kind, 0, Direction(dir), {length}, {bool(flipped)}};
        result = 1 | (center << 13) | ((!rotate) << 14);
        spawn_register = int32_t(0xF1U | (uint(uint8_t(xyz[0])) << 8)
                                 | (uint(uint8_t(xyz[1])) << 16) | (uint(uint8_t(xyz[2])) << 24));
        std::cout << kind << '\t' << dir << '\t' << length << '\t' << flipped
                  << '\t' << center << '\t' << rotate << '\t' << xyz[0]
                  << '\t' << xyz[1] << '\t' << xyz[2];
        SpawnAdvancedVisualEffect(&v);
    }
}
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="OpenTTD checkout root")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = (args.source / "src/vehicle.cpp").read_text()
    start = source.index("static const int8_t _vehicle_smoke_pos[8]")
    end = source.index("/**\n * Test if a bridge is above a vehicle.", start)
    with tempfile.TemporaryDirectory(prefix="vehicle-effect-oracle-") as directory:
        source_file = Path(directory) / "oracle.cpp"
        binary = Path(directory) / "oracle"
        source_file.write_text(PREAMBLE + source[start:end] + MAIN)
        subprocess.run(["c++", "-std=c++20", str(source_file), "-o", str(binary)], check=True)
        output = subprocess.check_output([str(binary)], text=True)
    if args.check:
        if output != FIXTURE.read_text():
            raise SystemExit("CB160 native positions differ from the committed fixture")
        print(f"OK: {len(output.splitlines()) - 1} native CB160 positions")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
