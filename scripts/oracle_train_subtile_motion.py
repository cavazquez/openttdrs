#!/usr/bin/env python3
"""Pinned native train entry coordinates and pixel movement.

The initial-tile table, GetNewVehiclePos and GetAdvanceDistance are unchanged.
Vehicle storage, tile lookup and continuous interpolation are adapted. This
checks in-tile position/direction and remainder costs, not pathfinding, tile
entry commands, consist spacing, speed integration or the native renderer.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from oracle_bridge_pillar_column import PIN, function_body

HARNESS = r'''
#include <cassert>
#include <cstdint>
#include <iomanip>
#include <iostream>
using uint = unsigned;
DISTANCE_CONSTANTS
using TileIndex = int;
struct Vehicle {
    int x_pos = 0, y_pos = 0, tile = 0;
    uint8_t direction = 0;
    ADVANCE_DISTANCE
};
struct GetNewVehiclePosResult { int x, y, old_tile, new_tile; };
int TileVirtXY(int x, int y) { return (x >= 0 && x < 16 && y >= 0 && y < 16) ? 0 : 1; }
PIXEL_STEP
INITIAL_TABLE
int main() {
    std::cout << "track_bit,enter_direction,pixel,remainder,advance_distance,x,y,direction\n"
              << std::fixed << std::setprecision(8);
    for (uint track = 0; track < 6; ++track)
    for (uint enter = 0; enter < 4; ++enter) {
        const uint8_t *initial = _initial_tile_subcoord[track][enter];
        if (initial[0] == 0 && initial[1] == 0 && initial[2] == 0) continue;
        Vehicle v{initial[0], initial[1], 0, initial[2]};
        for (uint pixel = 0; ; ++pixel) {
            assert(pixel < 16);
            const auto next = GetNewVehiclePos(&v);
            const uint cost = v.GetAdvanceDistance();
            for (uint quarter = 0; quarter < 4; ++quarter) {
                const uint remainder = cost * quarter / 4;
                const double fraction = double(remainder) / cost;
                const double x = v.x_pos + (next.x - v.x_pos) * fraction;
                const double y = v.y_pos + (next.y - v.y_pos) * fraction;
                std::cout << (1U << track) << ',' << enter * 2 + 1 << ',' << pixel << ','
                          << remainder << ',' << cost << ',' << x << ',' << y << ','
                          << unsigned(v.direction) << '\n';
            }
            if (next.new_tile != v.tile) break;
            v.x_pos = next.x; v.y_pos = next.y;
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
    if table is None:
        raise ValueError('native train entry table missing')
    step = function_body(sources['src/vehicle.cpp'], 'GetNewVehiclePosResult GetNewVehiclePos(')
    distance = function_body(sources['src/vehicle_base.h'], 'inline uint GetAdvanceDistance(')
    constants = []
    for name in ['TILE_AXIAL_DISTANCE', 'TILE_CORNER_DISTANCE']:
        match = re.search(r'const uint ' + name + r'\s*=\s*\d+;', sources['src/vehicle_base.h'])
        if match is None:
            raise ValueError('native movement constant missing: ' + name)
        constants.append(match[0])
    cpp = args.out / 'oracle.cpp'
    cpp.write_text(HARNESS.replace('ADVANCE_DISTANCE', distance)
                   .replace('PIXEL_STEP', step)
                   .replace('DISTANCE_CONSTANTS', '\n'.join(constants))
                   .replace('INITIAL_TABLE', table[0]))
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    if len(output.splitlines()) != 513:
        raise ValueError('expected 512 native train pose samples')
    (args.out / 'native-train-subtile-motion.csv').write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity/native-train-subtile-motion.csv'
        if fixture.read_text() != output:
            raise ValueError('native train poses differ from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, source_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unmodified_fragment_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in
                                   [('entry_table', table[0]), ('pixel_step', step), ('advance_distance', distance)]},
        command=command, rows=512, scope=__doc__), indent=2) + '\n')
    print('Native train poses: 512 samples, 12 valid track entries, four pixel remainders')


if __name__ == '__main__':
    main()
