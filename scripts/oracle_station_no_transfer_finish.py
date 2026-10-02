#!/usr/bin/env python3
"""Native no-transfer completion block followed by full loading/leave bodies.

The predecessor branch assumes no cargo was loaded or unloaded. Cargo counts,
one-type full/not-full masks, order predicates, flags, clock, storage and external
hooks are adapted. No cargo acceptance/staging, actual cooldown, consist policy,
callbacks/RNG, controller, world journey or renderer is certified.
"""
import argparse
import hashlib
import json
from pathlib import Path
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
    base = args.out / 'loading-fragments'
    generator = Path(__file__).with_name('oracle_station_loading_guard.py')
    subprocess.run([sys.executable, str(generator), '--openttd', str(args.openttd), '--out', str(base)], check=True)
    for name in ['native-enums.inc', 'native-leave-station.inc', 'native-handle-loading.inc']:
        shutil.copy2(base / name, args.out / name)
    raw = subprocess.check_output(['git', '-C', str(args.openttd), 'show', PIN + ':src/economy.cpp'])
    (args.out / 'native-src__economy.cpp').write_bytes(raw)
    body = function_body(raw.decode(), 'static void LoadUnloadVehicle(')
    marker = '} else {\n\t\tUpdateLoadUnloadTicks(front, st, 20); // We need the ticks for link refreshing.'
    assert body.count(marker) == 1
    opening = body.index('{', body.index(marker))
    block = function_body(body[opening:], '{')
    assert block.rstrip().endswith('front->vehicle_flags.Set(VehicleFlag::LoadingFinished, finished_loading);\n\t}')
    (args.out / 'native-no-transfer-finish.inc').write_text(block + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copy2(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 5377
    filename = 'native-station-no-transfer-finish.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        assert output == fixture.read_text(), 'native completion fixture changed'
    parent = json.loads((base / 'provenance.json').read_text())
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=PIN, rows=5376, command=command,
        source_sha256=hashlib.sha256(raw).hexdigest(),
        unchanged_block_sha256=hashlib.sha256(block.encode()).hexdigest(),
        output_sha256=hashlib.sha256(output.encode()).hexdigest(), scope=__doc__,
        parent_loading_provenance=parent), indent=2) + '\n')
    print('Native no-transfer completion: 5376 states, derived finished flag then loading/leave bodies')


if __name__ == '__main__':
    main()
