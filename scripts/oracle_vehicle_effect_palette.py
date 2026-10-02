#!/usr/bin/env python3
"""Run pinned native effect creation, lifetimes and draw-palette selection.

CreateEffectVehicle, VehicleSpriteSeq::Set, DoDrawVehicle, transparency lookup,
init/tick bodies and selected procedure rows are unchanged. Storage, available
allocation, bounds, viewport notifications and the draw sink are adapted.
This checks draw sprite/palette/status for F1/F2/F3/FA effect types, not emission,
RNG, callback execution, atlas decoding, blitting or complete scene raster.
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
    cadence = args.out / 'parent-cadence'
    subprocess.run([sys.executable, str(Path(__file__).with_name('oracle_vehicle_effect_cadence.py')),
                    '--openttd', str(args.openttd), '--out', str(cadence), '--check'], check=True)
    for name in ['constants', 'increment', 'selected-procs',
                 'SteamSmoke-init', 'SteamSmoke-tick', 'DieselSmoke-init', 'DieselSmoke-tick',
                 'ElectricSpark-init', 'ElectricSpark-tick', 'Smoke-init', 'Smoke-tick']:
        shutil.copyfile(cadence / ('native-' + name + '.inc'), args.out / ('native-' + name + '.inc'))
    sources = {}
    for name in ['src/vehicle.cpp', 'src/effectvehicle.cpp', 'src/vehicle_base.h',
                 'src/vehicle_type.h', 'src/transparency.h', 'src/table/sprites.h']:
        raw = subprocess.check_output(['git', 'show', PIN + ':' + name], cwd=args.openttd)
        (args.out / ('native-' + name.replace('/', '__'))).write_bytes(raw)
        sources[name] = raw.decode()
    functions = {}
    for label, name, signature in [
        ('sprite-set', 'src/vehicle_base.h', 'void Set(SpriteID sprite)'),
        ('create', 'src/effectvehicle.cpp', 'EffectVehicle *CreateEffectVehicle('),
        ('transparency', 'src/effectvehicle.cpp', 'TransparencyOption EffectVehicle::GetTransparencyOption('),
        ('draw', 'src/vehicle.cpp', 'static void DoDrawVehicle('),
    ]:
        body = function_body(sources[name], signature)
        (args.out / ('native-' + label + '.inc')).write_text(body + '\n')
        functions[label] = hashlib.sha256(body.encode()).hexdigest()
    enums = {}
    for label, name, pattern in [
        ('vehicle-type', 'src/vehicle_type.h', r'enum VehicleType.*?\n};'),
        ('vehicle-status', 'src/vehicle_base.h', r'enum class VehState.*?\n};'),
        ('transparency-enum', 'src/transparency.h', r'enum TransparencyOption.*?\n};'),
    ]:
        match = re.search(pattern, sources[name], re.S)
        if match is None:
            raise ValueError('native enum missing: ' + label)
        (args.out / ('native-' + label + '.inc')).write_text(match[0] + '\n')
        enums[label] = hashlib.sha256(match[0].encode()).hexdigest()
    constants = []
    for name in ['PAL_NONE', 'PALETTE_CRASH']:
        match = re.search(r'\b' + name + r'\s*=\s*(\d+|0x[0-9A-Fa-f]+)\s*;',
                          sources['src/table/sprites.h'])
        if match is None:
            raise ValueError('native palette constant missing: ' + name)
        constants.append('constexpr PaletteID ' + name + '=' + match[1] + ';')
    (args.out / 'native-palette-constants.inc').write_text('\n'.join(constants) + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copyfile(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 389
    (args.out / 'native-vehicle-effect-palette.csv').write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-client/tests/fixtures/native-vehicle-effect-palette.csv'
        if output != fixture.read_text():
            raise ValueError('native effect palette differs from fixture')
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, sources={name: hashlib.sha256(raw.encode()).hexdigest() for name, raw in sources.items()},
        unmodified_function_sha256=functions, unmodified_enum_sha256=enums,
        parent_cadence_fixture_byte_exact=True, command=command, rows=388, scope=__doc__), indent=2) + '\n')
    print('Native effect draw palettes: 388 rows, F1/F2/F3/FA, ages 0..96')


if __name__ == '__main__':
    main()
