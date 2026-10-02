#!/usr/bin/env python3
"""Native bay arrival branch with adapted single-vehicle/service storage.

Native movement arrays, direction helpers, turn block, stop-frame table and
the complete arrival branch remain unchanged. BeginLoading retains only its
native speed assignment; order/payment/RNG/animation/traffic effects are stubs.
The outer loop counts controller calls, not full ticks or complete journeys.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--tick', action='store_true', help='native loading-entry tick: original acceleration, fixed speed ceiling 112')
    parser.add_argument('--held', action='store_true', help='isolated native loading guard after entry; service maintenance omitted')
    parser.add_argument('--base-oracle', type=Path, default=Path(__file__).with_name('oracle_road_vehicle_turn_direction.py'))
    parser.add_argument('--template', type=Path, default=Path(__file__).with_suffix('.cpp'))
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    subprocess.run([sys.executable, str(args.base_oracle), '--openttd', str(args.openttd),
                    '--out', str(args.out / 'movement-fragments'), '--bay'], check=True)
    sys.path.insert(0, str(args.base_oracle.resolve().parent))
    from oracle_bridge_pillar_column import PIN, function_body
    base = args.out / 'movement-fragments'
    for source in base.glob('native-*.inc'):
        shutil.copy2(source, args.out / source.name)
    controller = (base / 'native-full-controller-reference.inc').read_text()
    branch = function_body(controller, 'if (v->IsFrontEngine() && ((IsInsideMM(v->state, RVSB_IN_ROAD_STOP, RVSB_IN_ROAD_STOP_END)')
    table = (base / 'native-src__table__roadveh_movement.h').read_text()
    stop_frames = re.search(r'extern const uint8_t _road_stop_stop_frame\[\] = \{.*?\};', table, re.S)[0]
    vehicle_source = subprocess.check_output(['git', '-C', str(args.openttd), 'show', PIN + ':src/vehicle.cpp'])
    (args.out / 'native-src__vehicle.cpp').write_bytes(vehicle_source)
    loading = function_body(vehicle_source.decode(), 'void Vehicle::BeginLoading(')
    speed = next(line for line in loading.splitlines() if line.strip() == 'this->cur_speed = 0;')
    fragments = {'arrival-branch': branch, 'stop-frames': stop_frames, 'begin-loading-speed': speed}
    extra_sources = {}
    for name in ['src/ground_vehicle.hpp', 'src/vehicle_base.h']:
        raw = subprocess.check_output(['git', '-C', str(args.openttd), 'show', PIN + ':' + name])
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        extra_sources[name] = raw.decode()
    road_source = (base / 'native-src__roadveh_cmd.cpp').read_text()
    fragments['loading-guard'] = next(line for line in road_source.splitlines()
                                     if line.strip() == 'if (v->current_order.IsType(OT_LOADING)) return true;')
    tick_controller = function_body(road_source, 'static bool RoadVehController(')
    fragments.update({
        'do-update-speed': function_body(extra_sources['src/ground_vehicle.hpp'], 'inline uint DoUpdateSpeed('),
        'advance-speed': function_body(extra_sources['src/vehicle_base.h'], 'static inline uint GetAdvanceSpeed('),
        'road-update-speed': function_body(road_source, 'int RoadVehicle::UpdateSpeed('),
        'tick-loop': function_body(tick_controller, 'while (j >= adv_spd)'),
        'tick-progress': next(line for line in tick_controller.splitlines() if 'v->progress == 0' in line),
    })
    for name, body in fragments.items():
        (args.out / ('native-' + name + '.inc')).write_text(body + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copy2(args.template, cpp)
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(args.out / 'oracle')]
    subprocess.run(command, check=True)
    invocation = [str((args.out / 'oracle').resolve())] + (['--tick'] if args.tick or args.held else []) + (['--held'] if args.held else [])
    output = subprocess.check_output(invocation, text=True)
    expected_rows = 3264 if args.held else 1728 if args.tick else 432
    if len(output.splitlines()) != expected_rows + 1:
        raise ValueError(f'expected {expected_rows} native bay states')
    result = args.out / ('native-road-bay-held-remainder.csv' if args.held else 'native-road-bay-loading-remainder.csv' if args.tick else 'native-road-bay-arrival.csv')
    result.write_text(output)
    if args.check:
        root = args.base_oracle.resolve().parents[1]
        assert output == (root / 'crates/openttdrs-core/tests/fixtures/parity' / result.name).read_text()
    provenance = dict(native_pin=PIN, rows=len(output.splitlines()) - 1,
                      unchanged_fragments_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in fragments.items()},
                      vehicle_source_sha256=hashlib.sha256(vehicle_source).hexdigest(),
                      extra_sources_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in extra_sources.items()},
                      scope=('Isolated unchanged OT_LOADING guard after native loading-entry tick; adapted order flags and repeated guard calls. Service maintenance, order processing, tick counters, callbacks/RNG, departure and full world are omitted.' if args.held else __doc__ if not args.tick else 'Adapted one-vehicle loading-entry tick with unchanged native speed, advance, movement loop/progress and arrival fragments. Original acceleration; fixed speed ceiling 112. No service continuation, traffic, callbacks, RNG or full journey.'),
                      command=command, invocation=invocation,
                      movement_source_provenance=json.loads((base / 'provenance.json').read_text()))
    (args.out / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print('Native bay loading holds:' if args.held else 'Native bay loading-entry ticks:' if args.tick else 'Native bay arrivals:', provenance['rows'], 'states in 16 tables')


if __name__ == '__main__':
    main()
