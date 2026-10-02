#!/usr/bin/env python3
"""Complete native loading handler and leave body with adapted state/hooks.

LoadingFinished is an input, not a native cargo calculation. Storage, flags,
orders, clock and external hooks are adapted. Repeated calls do not advance
game time. No actual cargo policy, timestep/world, callbacks/RNG, movement
controller or native renderer is executed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from oracle_bridge_pillar_column import function_body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    base = args.out / 'leave-fragments'
    generator = Path(__file__).with_name('oracle_station_departure_remainder.py')
    subprocess.run([sys.executable, str(generator), '--openttd', str(args.openttd), '--out', str(base)], check=True)
    for name in ['native-enums.inc', 'native-leave-station.inc']:
        shutil.copy2(base / name, args.out / name)
    source = (base / 'native-src__vehicle.cpp').read_text()
    handler = function_body(source, 'void Vehicle::HandleLoading(')
    (args.out / 'native-handle-loading.inc').write_text(handler + '\n')
    cpp = args.out / 'oracle.cpp'
    shutil.copy2(Path(__file__).with_suffix('.cpp'), cpp)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 24193, 'expected 24192 native loading-handler states'
    filename = 'native-station-loading-guard.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        assert output == fixture.read_text(), 'native loading-handler fixture changed'
    parent = json.loads((base / 'provenance.json').read_text())
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=parent['native_pin'], rows=24192, command=command, invocation=[str(binary.resolve())],
        unchanged_handle_loading_sha256=hashlib.sha256(handler.encode()).hexdigest(),
        output_sha256=hashlib.sha256(output.encode()).hexdigest(), scope=__doc__,
        parent_fragment_provenance=parent), indent=2) + '\n')
    print('Native loading handler: 24192 states, flags/waits/lateness/time, 1/2/10/100 calls')


if __name__ == '__main__':
    main()
