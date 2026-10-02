#!/usr/bin/env python3
"""Run pinned ShowVisualEffect with limited effect-allocation availability.

ShowVisualEffect, SpawnAdvancedVisualEffect, CreateEffectVehicle/Rel and RNG
bodies are unchanged. Storage, allocation availability, fixed ordinary powered
terrain, non-reversing flags, viewport/draw/sound sinks and callbacks are adapted.
The corpus selects standard steam/diesel/electric models: it checks creations,
RNG consumption/state under unavailable allocation, not the native pool policy,
advanced callback evaluation, grouped ticks, actual sounds or complete raster.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

from oracle_bridge_pillar_column import PIN, function_body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    parent = args.out / 'parent-palette'
    subprocess.run([sys.executable, str(Path(__file__).with_name('oracle_vehicle_effect_palette.py')),
                    '--openttd', str(args.openttd), '--out', str(parent), '--check'], check=True)
    for path in parent.glob('native-*.inc'):
        shutil.copyfile(path, args.out / path.name)
    sources = {}
    for name in ['src/vehicle.cpp', 'src/effectvehicle.cpp', 'src/vehicle_base.h',
                 'src/engine_type.h', 'src/train.h', 'src/core/random_func.hpp', 'src/core/random_func.cpp']:
        raw = subprocess.check_output(['git', 'show', PIN + ':' + name], cwd=args.openttd)
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw.decode()
    functions = {}
    for label, name, signature in [
        ('show', 'src/vehicle.cpp', 'void Vehicle::ShowVisualEffect()'),
        ('dispatch', 'src/vehicle.cpp', 'static void SpawnAdvancedVisualEffect('),
        ('create-relative', 'src/effectvehicle.cpp', 'EffectVehicle *CreateEffectVehicleRel('),
        ('random-next', 'src/core/random_func.cpp', 'uint32_t Randomizer::Next()'),
        ('random-seed', 'src/core/random_func.cpp', 'void Randomizer::SetSeed('),
        ('chance-i', 'src/core/random_func.hpp', 'inline bool Chance16I('),
        ('chance', 'src/core/random_func.hpp', 'inline bool Chance16('),
    ]:
        body = function_body(sources[name], signature)
        (args.out / ('native-' + label + '.inc')).write_text(body + '\n')
        functions[label] = hashlib.sha256(body.encode()).hexdigest()
    enums = {}
    for label, name, pattern in [
        ('visual-model', 'src/vehicle_base.h', r'enum VisualEffectSpawnModel.*?\n};'),
        ('visual-bits', 'src/engine_type.h', r'enum VisualEffect :.*?\n};'),
        ('rail-flags', 'src/train.h', r'enum VehicleRailFlag.*?\n};'),
    ]:
        match = re.search(pattern, sources[name], re.S)
        if match is None:
            raise ValueError('native enum missing: ' + label)
        (args.out / ('native-' + label + '.inc')).write_text(match[0] + '\n')
        enums[label] = hashlib.sha256(match[0].encode()).hexdigest()
    table = re.search(r'static const int8_t _vehicle_smoke_pos\[8\]\s*=\s*\{.*?\};',
                      sources['src/vehicle.cpp'], re.S)
    if table is None:
        raise ValueError('native direction table missing')
    (args.out / 'native-direction-table.inc').write_text(table[0] + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copyfile(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 1945
    (args.out / 'native-vehicle-effect-pool-loop.csv').write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-client/tests/fixtures/native-vehicle-effect-pool-loop.csv'
        if output != fixture.read_text():
            raise ValueError('native pool loop differs from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, sources={name: hashlib.sha256(raw.encode()).hexdigest() for name, raw in sources.items()},
        unmodified_function_sha256=functions, unmodified_enum_sha256=enums,
        direction_table_sha256=hashlib.sha256(table[0].encode()).hexdigest(),
        parent_palette_and_cadence_fixtures_byte_exact=True,
        command=command, rows=1944, scope=__doc__), indent=2) + '\n')
    print('Native effect pool loop: 1944 rows, steam/diesel/electric, free capacity 0/1/48')


if __name__ == '__main__':
    main()
