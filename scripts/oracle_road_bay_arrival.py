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
    for name, body in fragments.items():
        (args.out / ('native-' + name + '.inc')).write_text(body + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copy2(args.template, cpp)
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(args.out / 'oracle')]
    subprocess.run(command, check=True)
    output = subprocess.check_output([str((args.out / 'oracle').resolve())], text=True)
    result = args.out / 'native-road-bay-arrival.csv'
    result.write_text(output)
    if args.check:
        root = args.base_oracle.resolve().parents[1]
        assert output == (root / 'crates/openttdrs-core/tests/fixtures/parity' / result.name).read_text()
    provenance = dict(native_pin=PIN, rows=len(output.splitlines()) - 1,
                      unchanged_fragments_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in fragments.items()},
                      vehicle_source_sha256=hashlib.sha256(vehicle_source).hexdigest(),
                      scope=__doc__, command=command,
                      movement_source_provenance=json.loads((base / 'provenance.json').read_text()))
    (args.out / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print('Native bay arrivals:', provenance['rows'], 'states in 16 tables')


if __name__ == '__main__':
    main()
