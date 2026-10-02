#!/usr/bin/env python3
"""Pinned native pixel geometry and offsets for three-unit ordinary train chains.

Native entry coordinates, GetNewVehiclePos, entry XY/heading rebinding and
CalcNextVehicleOffset remain unchanged. Storage, route selection and the
outer movement loop are adapted. Each follower is initialized on the recorded
route at its native centre offset and advances one recorded pixel per head
step. This is a geometry oracle, not a complete TrainController, speed,
signals, depot, tunnel, save import, callback or renderer oracle.
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
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <iomanip>
#include <vector>
using uint = unsigned;
using Direction = uint8_t;
using TileIndex = int;
DISTANCE_CONSTANTS
struct Vehicle { int x_pos, y_pos, tile; uint8_t direction; ADVANCE_DISTANCE };
struct Train : Vehicle {
    struct { uint cached_veh_length = 8; } gcache;
    Train *next = nullptr;
    Train *Next() const { return next; }
    NEXT_OFFSET
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
struct Segment { int x, y; uint track, enter, exit = 0; };
struct State { Vehicle v; uint segment, pixel; };
int main(int argc, char **) {
    assert(argc == 1 || argc == 2);
    const bool render = argc == 2;
    const uint lengths[][3] = {{8,8,8},{8,7,5},{7,8,3},{1,1,1},{3,5,7}};
    const int dx[] = {-1,0,1,0}, dy[] = {0,1,0,-1};
    std::cout << "route,first_track,first_enter,next_track,next_enter,head_segment,head_pixel,head_direction,length0,length1,length2,unit,tile_x,tile_y,pixel,x,y,direction,enter,exit"
              << (render ? ",initial_remainder,render_budget,head_fraction" : "") << '\n';
    std::cout << std::fixed << std::setprecision(8);
    uint route = 0;
    for (uint first_track = 0; first_track < 6; ++first_track)
    for (uint first_enter = 0; first_enter < 4; ++first_enter) {
        const auto *initial = _initial_tile_subcoord[first_track][first_enter];
        if (initial[0] == 0 && initial[1] == 0 && initial[2] == 0) continue;
        Vehicle last{64 + initial[0], 64 + initial[1], TileVirtXY(64,64), initial[2]};
        for (;;) {
            const auto gp = GetNewVehiclePos(&last);
            if (gp.new_tile != last.tile) break;
            last.x_pos = gp.x; last.y_pos = gp.y;
        }
        const auto crossing = GetNewVehiclePos(&last);
        const uint next_enter = entrance((crossing.x >> 4) - 4, (crossing.y >> 4) - 4);
        for (uint next_track = 0; next_track < 6; ++next_track) {
            const auto *next_initial = _initial_tile_subcoord[next_track][next_enter];
            if (next_initial[0] == 0 && next_initial[1] == 0 && next_initial[2] == 0) continue;
            std::vector<Segment> segments;
            const uint prefix_track = first_enter & 1;
            for (int back = 3; back > 0; --back)
                segments.push_back({4 - dx[first_enter] * back, 4 - dy[first_enter] * back, prefix_track, first_enter});
            segments.push_back({4,4,first_track,first_enter});
            segments.push_back({crossing.x >> 4,crossing.y >> 4,next_track,next_enter});
            std::vector<State> states;
            for (uint segment = 0; segment < segments.size(); ++segment) {
                auto &s = segments[segment];
                const auto *entry = _initial_tile_subcoord[s.track][s.enter];
                Vehicle v{(s.x << 4) + entry[0],(s.y << 4) + entry[1],TileVirtXY(s.x << 4,s.y << 4),entry[2]};
                for (uint pixel = 0; ; ++pixel) {
                    assert(pixel < 16);
                    states.push_back({v,segment,pixel});
                    auto gp = GetNewVehiclePos(&v);
                    if (gp.new_tile != v.tile) {
                        s.exit = entrance((gp.x >> 4) - s.x,(gp.y >> 4) - s.y);
                        if (segment + 1 < segments.size()) {
                            const auto &next = segments[segment + 1];
                            assert(next.x == (gp.x >> 4) && next.y == (gp.y >> 4) && next.enter == s.exit);
                            const uint chosen_track = 1U << next.track;
                            const uint enterdir = next.enter;
                            ENTRY_REBIND
                            // The next iteration starts at this same native entry.
                            assert(gp.x == (next.x << 4) + _initial_tile_subcoord[next.track][next.enter][0]);
                            assert(gp.y == (next.y << 4) + _initial_tile_subcoord[next.track][next.enter][1]);
                            assert(chosen_dir == _initial_tile_subcoord[next.track][next.enter][2]);
                        }
                        break;
                    }
                    v.x_pos = gp.x; v.y_pos = gp.y;
                }
            }
            for (uint index = 48; index < states.size(); ++index) {
                auto &head = states[index];
                assert(head.segment == 3 || head.segment == 4);
                if (render && head.segment != 3) continue;
                for (const auto &length : lengths) {
                    Train units[3];
                    for (uint unit = 0; unit < 3; ++unit) {
                        units[unit].gcache.cached_veh_length = length[unit];
                        units[unit].next = unit == 2 ? nullptr : &units[unit + 1];
                    }
                    for (uint quarter = 0; quarter < (render ? 4U : 1U); ++quarter)
                    for (uint budget : {0U,96U,192U,384U,512U}) {
                    if (!render && budget != 0) continue;
                    const uint initial_remainder = render ? head.v.GetAdvanceDistance() * quarter / 4 : 0;
                    uint advanced_index = index, credit = initial_remainder + budget;
                    while (credit >= states[advanced_index].v.GetAdvanceDistance()) {
                        credit -= states[advanced_index].v.GetAdvanceDistance();
                        ++advanced_index;
                        assert(advanced_index + 1 < states.size());
                    }
                    const double fraction = double(credit) / states[advanced_index].v.GetAdvanceDistance();
                    uint behind = 0;
                    for (uint unit = 0; unit < 3; ++unit) {
                        if (unit > 0) behind += units[unit - 1].CalcNextVehicleOffset();
                        assert(behind <= index);
                        const auto &state = states[advanced_index - behind];
                        const auto &s = segments[state.segment];
                        units[unit].x_pos = state.v.x_pos;
                        units[unit].y_pos = state.v.y_pos;
                        if (unit > 0) {
                            Train *u = &units[unit - 1], *w = &units[unit];
                            assert(!(LENGTH_MISMATCH));
                        }
                        std::cout << route << ',' << (1U << first_track) << ',' << first_enter * 2 + 1
                                  << ',' << (1U << next_track) << ',' << next_enter * 2 + 1
                                  << ',' << head.segment << ',' << head.pixel << ',' << unsigned(head.v.direction)
                                  << ',' << length[0] << ',' << length[1] << ',' << length[2]
                                  << ',' << unit << ',' << s.x << ',' << s.y << ',' << state.pixel << ',';
                        if (render) {
                            Vehicle candidate = state.v;
                            const auto next = GetNewVehiclePos(&candidate);
                            std::cout << state.v.x_pos + (next.x - state.v.x_pos) * fraction
                                      << ',' << state.v.y_pos + (next.y - state.v.y_pos) * fraction;
                        } else {
                            std::cout << state.v.x_pos << ',' << state.v.y_pos;
                        }
                        std::cout << ',' << unsigned(state.v.direction)
                                  << ',' << s.enter * 2 + 1 << ',' << s.exit * 2 + 1;
                        if (render) std::cout << ',' << initial_remainder << ',' << budget << ',' << fraction;
                        std::cout << '\n';
                    }
                    }
                }
            }
            ++route;
        }
    }
    assert(route == 36);
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--render', action='store_true', help='adapted continuous presentation between native chain pixels, sharing the head step clock')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sources = {}
    for name in ['src/train_cmd.cpp', 'src/train.h', 'src/vehicle.cpp', 'src/vehicle_base.h']:
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
                     next_offset=function_body(sources['src/train.h'], 'int CalcNextVehicleOffset('))
    length_mismatch = 'std::max(abs(u->x_pos - w->x_pos), abs(u->y_pos - w->y_pos)) != u->CalcNextVehicleOffset()'
    if sources['src/train_cmd.cpp'].count(length_mismatch) != 1:
        raise ValueError('native ordinary train length invariant missing')
    fragments['length_mismatch'] = length_mismatch
    fragments['advance_distance'] = function_body(sources['src/vehicle_base.h'], 'inline uint GetAdvanceDistance(')
    constants = []
    for name in ['TILE_AXIAL_DISTANCE', 'TILE_CORNER_DISTANCE']:
        match = re.search(r'const uint ' + name + r'\s*=\s*\d+;', sources['src/vehicle_base.h'])
        if match is None:
            raise ValueError('native movement constant missing: ' + name)
        constants.append(match[0])
    fragments['distance_constants'] = '\n'.join(constants)
    cpp = args.out / 'oracle.cpp'
    cpp.write_text(HARNESS.replace('NEXT_OFFSET', fragments['next_offset'])
                   .replace('PIXEL_STEP', fragments['pixel_step'])
                   .replace('INITIAL_TABLE', fragments['entry_table'])
                   .replace('ENTRY_REBIND', fragments['entry_rebind'])
                   .replace('LENGTH_MISMATCH', fragments['length_mismatch'])
                   .replace('DISTANCE_CONSTANTS', fragments['distance_constants'])
                   .replace('ADVANCE_DISTANCE', fragments['advance_distance']))
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    invocation = [str(binary.resolve())] + (['--render'] if args.render else [])
    output = subprocess.check_output(invocation, text=True)
    rows = len(output.splitlines()) - 1
    expected = 115200 if args.render else 11520
    if rows != expected:
        raise ValueError(f'expected {expected} samples, got {rows}')
    filename = 'native-train-consist-fractional-render.csv' if args.render else 'native-train-consist-track-geometry.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        if fixture.read_text() != output:
            raise ValueError('native consist geometry differs from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, source_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unmodified_fragment_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in fragments.items()},
        command=command, invocation=invocation, rows=rows,
        scope=__doc__ if not args.render else __doc__ + '\nContinuous interpolation is adapted from each native pixel toward its GetNewVehiclePos candidate before tile-entry rebinding, matching the existing presentation contract. Every unit shares the head step count and fraction, using the head GetAdvanceDistance; no claim of native continuous rendering.'), indent=2) + '\n')
    print('Native train chain geometry:', rows, 'unit samples, 36 track pairs, five length configurations')


if __name__ == '__main__':
    main()
