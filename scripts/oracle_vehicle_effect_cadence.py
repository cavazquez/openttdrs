#!/usr/bin/env python3
"""Run pinned native CB160 dispatch and complete F1/F2/F3/FA effect lifetimes.

SpawnAdvancedVisualEffect, init/tick bodies, IncrementSprite, direction table
and selected native effect-procedure rows are unchanged. Callback inputs,
allocation, compact effect enum and viewport updates use a
scaffold. This verifies sprite IDs, lifetime and vertical movement; it does
not verify emission probability/RNG, save import, decoding or scene raster.
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
    for name in ['src/vehicle.cpp', 'src/effectvehicle.cpp', 'src/table/sprites.h']:
        raw = subprocess.check_output(['git', 'show', PIN + ':' + name], cwd=args.openttd)
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw
    functions = {}
    signatures = [('increment', 'static bool IncrementSprite('),
                  ('dispatch', 'static void SpawnAdvancedVisualEffect(')]
    for name in ['SteamSmoke', 'DieselSmoke', 'ElectricSpark', 'Smoke']:
        signatures.extend([(name + '-init', 'static void ' + name + 'Init('),
                           (name + '-tick', 'static bool ' + name + 'Tick(')])
    for label, signature in signatures:
        source = sources['src/vehicle.cpp' if label == 'dispatch' else 'src/effectvehicle.cpp'].decode()
        body = function_body(source, signature)
        (args.out / ('native-' + label + '.inc')).write_text(body + '\n')
        functions[label] = hashlib.sha256(body.encode()).hexdigest()
    table = re.search(r'static const int8_t _vehicle_smoke_pos\[8\]\s*=\s*\{.*?\};',
                      sources['src/vehicle.cpp'].decode(), re.S)
    if table is None:
        raise ValueError('native smoke direction table missing')
    (args.out / 'native-direction-table.inc').write_text(table[0] + '\n')
    procs = re.search(r'static const std::array<EffectProcs, EV_END> _effect_procs\s*=\s*\{\{.*?\}\};',
                      sources['src/effectvehicle.cpp'].decode(), re.S)
    if procs is None:
        raise ValueError('native effect procedure table missing')
    selected_procs = []
    for name in ['EV_STEAM_SMOKE', 'EV_DIESEL_SMOKE', 'EV_ELECTRIC_SPARK',
                 'EV_BREAKDOWN_SMOKE_AIRCRAFT']:
        rows = [line for line in procs[0].splitlines() if line.rstrip().endswith('// ' + name)]
        if len(rows) != 1:
            raise ValueError('native effect binding missing: ' + name)
        selected_procs.append(rows[0])
    selected = '\n'.join(selected_procs) + '\n'
    (args.out / 'native-selected-procs.inc').write_text(selected)
    (args.out / 'native-procedure-table.inc').write_text(procs[0] + '\n')
    constants = []
    for name in ['SPR_STEAM_SMOKE_0', 'SPR_STEAM_SMOKE_4', 'SPR_DIESEL_SMOKE_0',
                 'SPR_DIESEL_SMOKE_5', 'SPR_ELECTRIC_SPARK_0', 'SPR_ELECTRIC_SPARK_5',
                 'SPR_SMOKE_0', 'SPR_SMOKE_4']:
        match = re.search(r'\b' + name + r'\s*=\s*(\d+)\s*;', sources['src/table/sprites.h'].decode())
        if match is None:
            raise ValueError('native constant missing: ' + name)
        constants.append('constexpr SpriteID ' + name + '=' + match[1] + ';')
    (args.out / 'native-constants.inc').write_text('\n'.join(constants) + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copyfile(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 389
    (args.out / 'native-vehicle-effect-cadence.csv').write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-client/tests/fixtures/native-vehicle-effect-cadence.csv'
        if output != fixture.read_text():
            raise ValueError('native cadence differs from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, sources={name: hashlib.sha256(raw).hexdigest() for name, raw in sources.items()},
        unmodified_function_sha256=functions,
        direction_table_sha256=hashlib.sha256(table[0].encode()).hexdigest(),
        procedure_table_sha256=hashlib.sha256(procs[0].encode()).hexdigest(),
        selected_procedure_rows_sha256=hashlib.sha256(selected.encode()).hexdigest(),
        command=command, rows=388, scope=__doc__), indent=2) + '\n')
    print('Native CB160 dispatch and effect lifetimes: 388 rows, F1/F2/F3/FA, ages 0..96')


if __name__ == '__main__':
    main()
