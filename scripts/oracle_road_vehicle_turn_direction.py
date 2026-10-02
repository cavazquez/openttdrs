#!/usr/bin/env python3
"""Pinned native road direction and turn-delay steps for both road sides.

Direction functions, ordinary drive arrays and the controller's turn block
are unchanged native text. The harness adapts vehicle storage, initial states,
viewport and the outer movement loop. It excludes traffic, bays, reversals,
tile transitions, speed integration and complete journeys.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

from oracle_bridge_pillar_column import PIN, function_body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sources = {}
    fragments = {}
    for name in ['src/roadveh_cmd.cpp', 'src/roadveh.h', 'src/direction_type.h',
                 'src/direction_func.h', 'src/table/roadveh_movement.h',
                 'src/vehicle_base.h']:
        raw = subprocess.check_output(['git', 'show', PIN + ':' + name], cwd=args.openttd)
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw.decode()

    def save(name, body):
        (args.out / ('native-' + name + '.inc')).write_text(body + '\n')
        fragments[name] = hashlib.sha256(body.encode()).hexdigest()

    enums = []
    for name in ['Direction', 'DirDiff', 'DiagDirection']:
        match = re.search(r'enum ' + name + r' : uint8_t \{.*?\};',
                          sources['src/direction_type.h'], re.S)
        if match is None:
            raise ValueError('native enum missing: ' + name)
        enums.append(match[0])
    save('direction-enums', '\n'.join(enums))
    bodies = []
    for signature in ['inline bool IsValidDirection(', 'inline DirDiff DirDifference(',
                      'inline Direction ChangeDir(']:
        bodies.append(function_body(sources['src/direction_func.h'], signature))
    for signature in ['static Direction RoadVehGetNewDirection(',
                      'static Direction RoadVehGetSlidingDirection(']:
        bodies.append(function_body(sources['src/roadveh_cmd.cpp'], signature))
    save('direction-functions', '\n'.join(bodies))
    controller = function_body(sources['src/roadveh_cmd.cpp'], 'bool IndividualRoadVehicleController(')
    save('full-controller-reference', controller)
    start = controller.index('\tDirection old_dir = v->direction;\n\tif (new_dir != old_dir)')
    block = function_body(controller[start:], '\tif (new_dir != old_dir)')
    save('turn-block', '\tDirection old_dir = v->direction;\n' + block)
    arrays = []
    for index in range(32):
        match = re.search(r'static const RoadDriveEntry _roadveh_drive_data_' + str(index)
                          + r'\[\] = \{.*?\};', sources['src/table/roadveh_movement.h'], re.S)
        if match is None:
            raise ValueError('native road array missing: ' + str(index))
        arrays.append(match[0])
    save('drive-arrays', '\n'.join(arrays))
    rows = re.search(r'_road_road_drive_data\[\] = \{(.*?)\};',
                     sources['src/table/roadveh_movement.h'], re.S)[1].splitlines()[1:33]
    if [line.strip().rstrip(',') for line in rows] != ['_roadveh_drive_data_' + str(i) for i in range(32)]:
        raise ValueError('native drive table binding differs')
    save('drive-bindings', '\n'.join(rows))
    constants = []
    for name in ['RDE_NEXT_TILE', 'RDE_TURNED']:
        match = re.search(r'static const uint ' + name + r'\s*=\s*(0x[0-9A-Fa-f]+);',
                          sources['src/roadveh.h'])
        if match is None:
            raise ValueError('native constant missing: ' + name)
        constants.append('constexpr uint ' + name + ' = ' + match[1] + ';')
    save('constants', '\n'.join(constants))
    distance_constants = []
    for name in ['TILE_AXIAL_DISTANCE', 'TILE_CORNER_DISTANCE']:
        match = re.search(r'const uint ' + name + r'\s*=\s*\d+;', sources['src/vehicle_base.h'])
        if match is None:
            raise ValueError('native movement distance missing: ' + name)
        distance_constants.append(match[0])
    save('distance-constants', '\n'.join(distance_constants))
    save('advance-distance', function_body(sources['src/vehicle_base.h'], 'inline uint GetAdvanceDistance('))
    cpp = args.out / 'oracle.cpp'
    shutil.copyfile(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    budget_path = args.out / 'native-road-render-budget.csv'
    output = subprocess.check_output([str(binary.resolve()), str(budget_path.resolve())], text=True)
    (args.out / 'native-road-vehicle-turn-direction.csv').write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity/native-road-vehicle-turn-direction.csv'
        if output != fixture.read_text():
            raise ValueError('native road turn states differ from fixture')
        budget_fixture = fixture.with_name('native-road-render-budget.csv')
        if budget_path.read_text() != budget_fixture.read_text():
            raise ValueError('native road render budgets differ from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, sources={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unmodified_fragments_sha256=fragments, rows=len(output.splitlines()) - 1,
        render_budget_rows=len(budget_path.read_text().splitlines()) - 1,
        render_scope='Adapted continuous interpolation between stored native turn/move states; exact native GetAdvanceDistance costs. This does not execute the native renderer.',
        command=command, scope=__doc__), indent=2) + '\n')
    print('Native road direction/turn-delay states:', len(output.splitlines()) - 1, 'rows, 24 ordinary drive tables')


if __name__ == '__main__':
    main()
