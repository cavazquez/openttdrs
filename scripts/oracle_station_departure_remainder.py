#!/usr/bin/env python3
"""Pinned complete LeaveStation body: fractional movement conservation.

Native vehicle/order enums and LeaveStation are unchanged. Vehicle/station
storage, flags, order predicates, clock and external side effects are adapted;
cargo payment is null. This does not execute loading decisions, real cargo,
timetables, callbacks/RNG, world journeys, subsequent movement or rendering.
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
    for name in ['src/vehicle.cpp', 'src/order_type.h', 'src/vehicle_type.h', 'src/station_type.h']:
        raw = subprocess.check_output(['git', '-C', str(args.openttd), 'show', PIN + ':' + name])
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw.decode()
    leave = function_body(sources['src/vehicle.cpp'], 'void Vehicle::LeaveStation(')
    enums = []
    for name in ['OrderLoadType', 'OrderUnloadType', 'OrderType']:
        match = re.search(r'enum(?: class)? ' + name + r'\s*:\s*uint8_t\s*\{.*?\};', sources['src/order_type.h'], re.S)
        if match is None:
            raise ValueError('native enum missing: ' + name)
        enums.append(match[0])
    match = re.search(r'enum VehicleType : uint8_t \{.*?\};', sources['src/vehicle_type.h'], re.S)
    if match is None:
        raise ValueError('native vehicle enum missing')
    enums.append(match[0])
    enum_text = '\n'.join(enums)
    (args.out / 'native-leave-station.inc').write_text(leave + '\n')
    (args.out / 'native-enums.inc').write_text(enum_text + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copy2(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 1345, 'expected 1344 native departure states'
    filename = 'native-station-departure-remainder.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        assert output == fixture.read_text(), 'native station departure fixture changed'
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, rows=1344, command=command, invocation=[str(binary.resolve())],
        source_sha256={name: hashlib.sha256(body.encode()).hexdigest() for name, body in sources.items()},
        unchanged_fragment_sha256={'leave_station': hashlib.sha256(leave.encode()).hexdigest(),
                                   'enums': hashlib.sha256(enum_text.encode()).hexdigest()},
        output_sha256=hashlib.sha256(output.encode()).hexdigest(), scope=__doc__), indent=2) + '\n')
    print('Native station departures: 1344 states, train/road, 32 hook contexts, 21 fraction pairs')


if __name__ == '__main__':
    main()
