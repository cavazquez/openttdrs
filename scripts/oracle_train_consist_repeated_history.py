#!/usr/bin/env python3
"""Pinned native pixel geometry for six ordinary units revisiting a junction.

Entry coordinates, GetNewVehiclePos, CalcNextVehicleOffset, advance distance
and the native centre-spacing expression are unchanged. Route/storage/loops
are adapted. Entry positions are initialized from the native table. This is
not a full TrainController, speed/signals/traffic/depot/tunnel/callback or
renderer oracle. Optional continuous presentation targets the native pixel
candidate before entry rebinding, using the head step clock.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
from oracle_bridge_pillar_column import PIN, function_body
from oracle_train_consist_track_geometry import HARNESS

MAIN = r'''
int main(int argc, char **) {
    assert(argc == 1 || argc == 2);
    const bool render = argc == 2;
    const int coords[][2]={{7,4},{6,4},{5,4},{4,4},{3,4},{3,5},{4,5},{4,4},{3,4},{3,5},{4,5},{4,4},{3,4},{3,5},{4,5}};
    std::vector<Segment> segments;
    std::vector<State> states;
    for (uint i=0;i<15;++i) {
        int x=coords[i][0],y=coords[i][1];
        uint enter=i==0?0:entrance(x-coords[i-1][0],y-coords[i-1][1]);
        int nx=i==14?4:coords[i+1][0],ny=i==14?4:coords[i+1][1];
        uint exit=entrance(nx-x,ny-y),chosen=6;
        for (uint track=0;track<6;++track) {
            auto *e=_initial_tile_subcoord[track][enter];
            if(e[0]==0&&e[1]==0&&e[2]==0)continue;
            Vehicle v{(x<<4)+e[0],(y<<4)+e[1],TileVirtXY(x<<4,y<<4),e[2]};
            for (;;) { auto gp=GetNewVehiclePos(&v);
                if(gp.new_tile!=v.tile) { if(entrance((gp.x>>4)-x,(gp.y>>4)-y)==exit) {assert(chosen==6);chosen=track;} break; }
                v.x_pos=gp.x;v.y_pos=gp.y;
            }
        }
        assert(chosen<6);segments.push_back({x,y,chosen,enter,exit});
        auto *e=_initial_tile_subcoord[chosen][enter];
        Vehicle v{(x<<4)+e[0],(y<<4)+e[1],TileVirtXY(x<<4,y<<4),e[2]};
        for(uint pixel=0;;++pixel) {assert(pixel<16);states.push_back({v,i,pixel});auto gp=GetNewVehiclePos(&v);if(gp.new_tile!=v.tile)break;v.x_pos=gp.x;v.y_pos=gp.y;}
    }
    std::cout<<"head_segment,head_pixel,head_direction,unit,tile_x,tile_y,pixel,x,y,direction,enter,exit,distance_mismatch" << (render ? ",initial_remainder,render_budget,head_fraction" : "") << '\n';
    std::cout << std::fixed << std::setprecision(8);
    uint rows=0,mismatches=0;
    for(uint index=0;index<states.size();++index) {
        auto &h=states[index];if(h.segment<7||h.segment>11)continue;
        Train units[6];for(uint unit=0;unit<6;++unit){units[unit].gcache.cached_veh_length=8;units[unit].next=unit==5?nullptr:&units[unit+1];}
        for (uint quarter=0;quarter<(render?4U:1U);++quarter)
        for (uint budget : {0U,96U,192U,384U,512U}) {
        if (!render && budget != 0) continue;
        uint remainder = render ? h.v.GetAdvanceDistance()*quarter/4 : 0;
        uint advanced=index,credit=remainder+budget;
        while(credit>=states[advanced].v.GetAdvanceDistance()) {
            credit-=states[advanced].v.GetAdvanceDistance();++advanced;
            assert(advanced<states.size());
        }
        double fraction=double(credit)/states[advanced].v.GetAdvanceDistance();
        uint behind=0;
        for(uint unit=0;unit<6;++unit) {
            if(unit>0) { behind+=units[unit-1].CalcNextVehicleOffset(); }
            assert(behind<=index);
            auto &v=states[advanced-behind];auto &s=segments[v.segment];units[unit].x_pos=v.v.x_pos;units[unit].y_pos=v.v.y_pos;
            bool mismatch = false;
            if (unit > 0) {
                Train *u = &units[unit-1], *w = &units[unit];
                mismatch = CENTRE_SPACING_EXPRESSION;
            }
            mismatches += mismatch;
            std::cout<<h.segment<<','<<h.pixel<<','<<uint(h.v.direction)<<','<<unit<<','<<s.x<<','<<s.y<<','<<v.pixel<<',';
            if(render) {
                auto gp=GetNewVehiclePos(&v.v);
                std::cout<<v.v.x_pos+(gp.x-v.v.x_pos)*fraction<<','<<v.v.y_pos+(gp.y-v.v.y_pos)*fraction;
            } else {std::cout<<v.v.x_pos<<','<<v.v.y_pos;}
            std::cout<<','<<uint(v.v.direction)<<','<<s.enter*2+1<<','<<s.exit*2+1<<','<<mismatch;
            if(render)std::cout<<','<<remainder<<','<<budget<<','<<fraction;
            std::cout<<'\n';++rows;
        }
    }
        }
    assert(mismatches==0);
    std::cerr<<"rows="<<rows<<" distance mismatches="<<mismatches<<'\n';
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openttd', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--render', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sources = {}
    for name in ['src/train_cmd.cpp', 'src/train.h', 'src/vehicle.cpp', 'src/vehicle_base.h']:
        raw = subprocess.check_output(['git','show',PIN+':'+name], cwd=args.openttd)
        (args.out/('native-'+name.replace('/','__'))).write_bytes(raw)
        sources[name] = raw.decode()
    fragments = dict(
        next_offset=function_body(sources['src/train.h'],'int CalcNextVehicleOffset('),
        pixel_step=function_body(sources['src/vehicle.cpp'],'GetNewVehiclePosResult GetNewVehiclePos('),
        entry_table=re.search(r'static const uint8_t _initial_tile_subcoord\[6\]\[4\]\[3\] = \{.*?\};',sources['src/train_cmd.cpp'],re.S)[0],
        advance_distance=function_body(sources['src/vehicle_base.h'],'inline uint GetAdvanceDistance('),
        distance_constants='\n'.join(re.search(r'const uint '+n+r'\s*=\s*\d+;',sources['src/vehicle_base.h'])[0] for n in ['TILE_AXIAL_DISTANCE','TILE_CORNER_DISTANCE']))
    cpp = HARNESS[:HARNESS.index('int main(')] + MAIN
    for key, value in fragments.items():
        token = dict(next_offset='NEXT_OFFSET',pixel_step='PIXEL_STEP',entry_table='INITIAL_TABLE',advance_distance='ADVANCE_DISTANCE',distance_constants='DISTANCE_CONSTANTS')[key]
        cpp = cpp.replace(token,value)
    native_spacing = 'std::max(abs(u->x_pos - w->x_pos), abs(u->y_pos - w->y_pos)) != u->CalcNextVehicleOffset()'
    assert sources['src/train_cmd.cpp'].count(native_spacing)==1
    cpp = cpp.replace('CENTRE_SPACING_EXPRESSION',native_spacing)
    fragments['centre_spacing_expression'] = native_spacing
    source = args.out/'oracle.cpp';source.write_text(cpp)
    binary = args.out/'oracle'
    command = ['c++','-std=c++20','-O2','-Wall','-Wextra','-Werror',str(source),'-o',str(binary)]
    subprocess.run(command,check=True);binary.chmod(0o555)
    invocation = [str(binary.resolve())]+(['--render'] if args.render else [])
    output = subprocess.check_output(invocation,text=True)
    rows = len(output.splitlines())-1
    assert rows == (4800 if args.render else 240),rows
    filename = 'native-train-consist-repeated-history'+('-render' if args.render else '')+'.csv'
    (args.out/filename).write_text(output)
    if args.check:
        fixture = Path(__file__).resolve().parents[1]/'crates/openttdrs-core/tests/fixtures/parity'/filename
        assert fixture.read_text()==output,'native history fixture differs'
    (args.out/'provenance.json').write_text(json.dumps(dict(native_pin=PIN,source_sha256={n:hashlib.sha256(v.encode()).hexdigest() for n,v in sources.items()},unmodified_fragment_sha256={n:hashlib.sha256(v.encode()).hexdigest() for n,v in fragments.items()},command=command,invocation=invocation,rows=rows,scope=__doc__),indent=2)+'\n')
    print('Native repeated history:',rows,'samples; all centre-spacing invariants pass')

if __name__ == '__main__':
    main()
