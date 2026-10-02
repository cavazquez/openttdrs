#!/usr/bin/env python3
"""Pinned native bay departure branch with adapted entrance occupancy.

Movement arrays, direction helpers, turn/arrival branches and stop frames are
unchanged native text. Storage, completed-loading order flags, occupancy and
repetition are adapted. No speed tick, order/service maintenance, traffic pool,
callbacks/RNG, complete journey or native renderer is executed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

MAIN = r'''
int main() {
    std::cout << "table,busy,calls,input_frame,input_x,input_y,input_direction,input_speed,input_subspeed,input_progress,frame,x,y,direction,speed,subspeed,progress,entered,result\n";
    for (unsigned table = 32; table < 64; ++table) {
        if ((table & (1U << RVS_ENTERED_STOP)) != 0) continue;
        _settings_game.vehicle.road_side = table >= 48;
        const auto *data = drive_tables[table];
        RoadVehicle anchor;
        anchor.state = table & ~16U;
        anchor.x_pos = data[0].x;
        anchor.y_pos = data[0].y;
        anchor.direction = RoadVehGetNewDirection(&anchor, data[1].x, data[1].y);
        for (unsigned steps = 0; native_bay_step(&anchor); ++steps) assert(steps < 64);
        assert(anchor.begin_loading && HasBit(anchor.state, RVS_ENTERED_STOP));
        anchor.current_order.leaving = true;
        anchor.begin_loading = false;
        for (bool busy : {false, true})
        for (unsigned calls : {1U, 2U, 10U, 100U})
        for (unsigned speed : {0U, 1U, 20U, 112U})
        for (unsigned subspeed : {0U, 99U, 255U})
        for (unsigned progress : {0U, 1U, 191U, 255U}) {
            if (!busy && calls != 1) continue;
            RoadVehicle v = anchor;
            v.cur_speed = speed;
            v.subspeed = subspeed;
            v.progress = progress;
            RoadStop::GetByTile(0, RoadStopType::Bus)->busy = busy;
            bool result = false;
            for (unsigned call = 0; call < calls; ++call) {
                result = native_bay_step(&v);
                assert(!busy || !result);
            }
            std::cout << table << ',' << busy << ',' << calls << ',' << anchor.frame << ',' << anchor.x_pos << ',' << anchor.y_pos << ',' << unsigned(anchor.direction) << ',' << speed << ',' << subspeed << ',' << progress << ',' << v.frame << ',' << v.x_pos << ',' << v.y_pos << ',' << unsigned(v.direction) << ',' << v.cur_speed << ',' << unsigned(v.subspeed) << ',' << unsigned(v.progress) << ',' << HasBit(v.state, RVS_ENTERED_STOP) << ',' << result << '\n';
        }
    }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    base = args.out / 'arrival-fragments'
    generator = Path(__file__).with_name('oracle_road_bay_arrival.py')
    subprocess.run([sys.executable, str(generator), '--openttd', str(args.openttd),
                    '--out', str(base)], check=True)
    for path in base.glob('native-*.inc'):
        shutil.copy2(path, args.out / path.name)
    prefix = (base / 'oracle.cpp').read_text().split('int main(')[0]
    replacements = {
        'bool loading = false;': 'bool loading = false, leaving = false;',
        'return loading ? type == OT_LOADING : type == OT_GOTO_STATION;':
            'return loading ? type == OT_LOADING : leaving ? type == OT_LEAVESTATION : type == OT_GOTO_STATION;',
    }
    for old, new in replacements.items():
        assert prefix.count(old) == 1, 'adapted order stub changed'
        prefix = prefix.replace(old, new)
    cpp = args.out / 'oracle.cpp'
    cpp.write_text(prefix + MAIN)
    binary = args.out / 'oracle'
    command = ['c++', '-std=c++20', '-O2', '-Wall', '-Wextra', '-Werror', str(cpp), '-o', str(binary)]
    subprocess.run(command, check=True)
    binary.chmod(0o555)
    output = subprocess.check_output([str(binary.resolve())], text=True)
    assert len(output.splitlines()) == 3841, 'expected 3840 native bay exit states'
    filename = 'native-road-bay-busy-exit.csv'
    (args.out / filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1] / 'crates/openttdrs-core/tests/fixtures/parity' / filename
        assert output == fixture.read_text(), 'native bay exit fixture changed'
    provenance = json.loads((base / 'provenance.json').read_text())
    (args.out / 'provenance.json').write_text(json.dumps(dict(
        native_pin=provenance['native_pin'], rows=3840, command=command,
        invocation=[str(binary.resolve())], output_sha256=hashlib.sha256(output.encode()).hexdigest(),
        scope=__doc__, parent_fragment_provenance=provenance), indent=2) + '\n')
    print('Native bay exits: 3840 states, 16 tables, busy/free entrance')


if __name__ == '__main__':
    main()
