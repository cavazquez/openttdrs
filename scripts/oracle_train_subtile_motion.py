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
enum class VehState { Stopped };
constexpr int OT_LOADING = 1;
struct Flags { bool stopped = false; bool Test(VehState) const { return stopped; } };
struct Order { bool loading = false; bool IsType(int type) const { return loading && type == OT_LOADING; } };
struct Vehicle {
    int x_pos = 0, y_pos = 0, tile = 0;
    uint8_t direction = 0;
    uint16_t cur_speed = 0;
    uint8_t progress = 0;
    Flags vehstatus{};
    Order current_order{};
    ADVANCE_DISTANCE
};
struct GetNewVehiclePosResult { int x, y, old_tile, new_tile; };
int TileVirtXY(int x, int y) { return (x >= 0 && x < 16 && y >= 0 && y < 16) ? 0 : 1; }
PIXEL_STEP
INITIAL_TABLE
bool native_hold(Vehicle *v) {
    STOPPED_GUARD
    LOADING_GUARD
    return false;
}
int main(int argc, char **) {
    assert(argc == 1 || argc == 2);
    const bool held = argc == 2;
    std::cout << "track_bit,enter_direction,pixel,remainder,advance_distance,x,y,direction"
              << (held ? ",hold_mode,handler_result" : "") << '\n'
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
            for (uint mode = 1; mode <= (held ? 2U : 1U); ++mode)
            for (uint quarter = 0; quarter < 4; ++quarter) {
                const uint held_remainders[] = {0,1,cost / 2,cost - 1};
                const uint remainder = held ? held_remainders[quarter] : cost * quarter / 4;
                v.progress = remainder;
                v.vehstatus.stopped = mode == 1;
                v.current_order.loading = mode == 2;
                const bool hold_result = held && native_hold(&v);
                assert(!held || hold_result);
                const double fraction = hold_result ? 0.0 : double(remainder) / cost;
                const double x = v.x_pos + (next.x - v.x_pos) * fraction;
                const double y = v.y_pos + (next.y - v.y_pos) * fraction;
                std::cout << (1U << track) << ',' << enter * 2 + 1 << ',' << pixel << ','
                          << remainder << ',' << cost << ',' << x << ',' << y << ','
                          << unsigned(v.direction);
                if (held) std::cout << ',' << mode << ',' << hold_result;
                std::cout << '\n';
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
    parser.add_argument('--held', action='store_true', help='stopped/loading guards; physical pixel unchanged despite retained progress')
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
    handler = function_body(sources['src/train_cmd.cpp'], 'static bool TrainLocoHandler(')
    stopped = next(line for line in handler.splitlines() if 'vehstatus.Test(VehState::Stopped) && v->cur_speed == 0) return true;' in line)
    loading = next(line for line in handler.splitlines() if 'current_order.IsType(OT_LOADING)) return true;' in line)
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
                   .replace('INITIAL_TABLE', table[0])
                   .replace('STOPPED_GUARD', stopped).replace('LOADING_GUARD', loading))
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    invocation = [str(binary.resolve())] + (['--held'] if args.held else [])
    output = subprocess.check_output(invocation, text=True)
    count = 1024 if args.held else 512
    if len(output.splitlines()) != count + 1:
        raise ValueError(f'expected {count} native train pose samples')
    filename = 'native-train-held-motion.csv' if args.held else 'native-train-subtile-motion.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        if fixture.read_text() != output:
            raise ValueError('native train poses differ from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, source_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unmodified_fragment_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in
                                   [('entry_table', table[0]), ('pixel_step', step), ('advance_distance', distance), ('stopped_guard', stopped), ('loading_guard', loading)]},
        command=command, invocation=invocation, rows=count,
        scope=__doc__ if not args.held else 'Isolated unchanged stopped/loading guards retain native physical pixel positions. Adapted state flags, loading maintenance and enumeration; no full handler, speed integration, callbacks or native renderer.'), indent=2) + '\n')
    print('Native train poses:', count, 'samples, 12 valid track entries, four pixel remainders')


if __name__ == '__main__':
    main()
