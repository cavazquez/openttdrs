#!/usr/bin/env python3
"""Ordinary train track transitions from pinned native movement fragments.

The entry table, pixel step, distance cost and XY/heading rebind are unchanged.
The outer loop, vehicle storage, selected tracks and continuous interpolation
are adapted. This does not execute native pathfinding, signals, speed control,
consist movement, tunnels, depots or the native renderer. Fractional positions
continue the current pixel step; a completed boundary step rebinds the entry.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from oracle_bridge_pillar_column import PIN, function_body

HARNESS = r'''
#include <bit>
#include <cassert>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <vector>
using uint = unsigned;
using Direction = uint8_t;
using TileIndex = int;
DISTANCE_CONSTANTS
struct Vehicle {
    int x_pos = 0, y_pos = 0, tile = 0;
    uint8_t direction = 0;
    ADVANCE_DISTANCE
};
struct GetNewVehiclePosResult { int x, y, old_tile, new_tile; };
int TileVirtXY(int x, int y) { return (x >> 4) | ((y >> 4) << 8); }
PIXEL_STEP
INITIAL_TABLE
uint FindFirstBit(uint bits) { return std::countr_zero(bits); }
uint entrance(int dx, int dy) {
    if (dx == -1 && dy == 0) return 0;
    if (dx == 0 && dy == 1) return 1;
    if (dx == 1 && dy == 0) return 2;
    assert(dx == 0 && dy == -1); return 3;
}
int main() {
    const uint budgets[] = {0, 48, 96, 144, 192, 240, 256, 384, 512, 768};
    std::cout << "track_bit,enter_direction,next_track_bit,next_enter_direction,pixel,remainder,budget,tile_x,tile_y,path_index,x,y,direction\n"
              << std::fixed << std::setprecision(8);
    for (uint track = 0; track < 6; ++track)
    for (uint enter = 0; enter < 4; ++enter) {
        const uint8_t *initial = _initial_tile_subcoord[track][enter];
        if (initial[0] == 0 && initial[1] == 0 && initial[2] == 0) continue;
        Vehicle last{32 + initial[0], 32 + initial[1], TileVirtXY(32, 32), initial[2]};
        uint last_pixel = 0;
        for (;;) {
            const auto gp = GetNewVehiclePos(&last);
            if (gp.new_tile != last.tile) break;
            last.x_pos = gp.x; last.y_pos = gp.y;
            assert(++last_pixel < 16);
        }
        const auto crossing = GetNewVehiclePos(&last);
        const uint enterdir = entrance((crossing.x >> 4) - 2, (crossing.y >> 4) - 2);
        for (uint next_track = 0; next_track < 6; ++next_track) {
            const uint8_t *next_initial = _initial_tile_subcoord[next_track][enterdir];
            if (next_initial[0] == 0 && next_initial[1] == 0 && next_initial[2] == 0) continue;
            std::vector<Vehicle> states{last};
            Vehicle v = last;
            for (uint i = 0; i < 5; ++i) {
                auto gp = GetNewVehiclePos(&v);
                if (gp.new_tile != v.tile) {
                    assert(i == 0);
                    const uint chosen_track = 1U << next_track;
                    ENTRY_REBIND
                    v.direction = chosen_dir;
                    v.tile = gp.new_tile;
                }
                v.x_pos = gp.x; v.y_pos = gp.y;
                states.push_back(v);
            }
            for (uint quarter = 0; quarter < 4; ++quarter)
            for (uint budget : budgets) {
                const uint remainder = last.GetAdvanceDistance() * quarter / 4;
                uint remaining = remainder + budget;
                uint index = 0;
                while (remaining >= states[index].GetAdvanceDistance()) {
                    remaining -= states[index].GetAdvanceDistance();
                    assert(++index < states.size());
                }
                Vehicle &current = states[index];
                const auto gp = GetNewVehiclePos(&current);
                const double fraction = double(remaining) / current.GetAdvanceDistance();
                const double x = current.x_pos + (gp.x - current.x_pos) * fraction;
                const double y = current.y_pos + (gp.y - current.y_pos) * fraction;
                std::cout << (1U << track) << ',' << enter * 2 + 1 << ',' << (1U << next_track)
                          << ',' << enterdir * 2 + 1 << ',' << last_pixel << ',' << remainder
                          << ',' << budget << ',' << (current.x_pos >> 4) << ',' << (current.y_pos >> 4)
                          << ',' << (index == 0 ? 0 : 1) << ',' << x << ',' << y
                          << ',' << unsigned(current.direction) << '\n';
            }
        }
    }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sources = {}
    for name in ['src/train_cmd.cpp', 'src/vehicle.cpp', 'src/vehicle_base.h']:
        raw = subprocess.check_output(['git', 'show', PIN + ':' + name], cwd=args.openttd)
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw.decode()
    table = re.search(r'static const uint8_t _initial_tile_subcoord\[6\]\[4\]\[3\] = \{.*?\};',
                      sources['src/train_cmd.cpp'], re.S)
    rebind = re.search(r'const uint8_t \*b = _initial_tile_subcoord\[FindFirstBit\(chosen_track\)\]\[enterdir\];.*?Direction chosen_dir = \(Direction\)b\[2\];',
                       sources['src/train_cmd.cpp'], re.S)
    if table is None or rebind is None:
        raise ValueError('native entry table/rebind missing')
    fragments = dict(entry_table=table[0], entry_rebind=rebind[0],
                     pixel_step=function_body(sources['src/vehicle.cpp'], 'GetNewVehiclePosResult GetNewVehiclePos('),
                     advance_distance=function_body(sources['src/vehicle_base.h'], 'inline uint GetAdvanceDistance('))
    constants = []
    for name in ['TILE_AXIAL_DISTANCE', 'TILE_CORNER_DISTANCE']:
        match = re.search(r'const uint ' + name + r'\s*=\s*\d+;', sources['src/vehicle_base.h'])
        if match is None:
            raise ValueError('native movement constant missing: ' + name)
        constants.append(match[0])
    cpp = args.out / 'oracle.cpp'
    cpp.write_text(HARNESS.replace('ADVANCE_DISTANCE', fragments['advance_distance'])
                   .replace('PIXEL_STEP', fragments['pixel_step'])
                   .replace('DISTANCE_CONSTANTS', '\n'.join(constants))
                   .replace('INITIAL_TABLE', fragments['entry_table'])
                   .replace('ENTRY_REBIND', fragments['entry_rebind']))
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    rows = len(output.splitlines()) - 1
    if rows != 1440:
        raise ValueError(f'expected 1440 samples, got {rows}')
    filename = 'native-train-track-transition.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        if fixture.read_text() != output:
            raise ValueError('native transitions differ from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, source_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unmodified_fragment_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in fragments.items()},
        command=command, rows=rows, scope=__doc__), indent=2) + '\n')
    print('Native train transitions: 1440 samples, 36 ordinary track pairs')


if __name__ == '__main__':
    main()
