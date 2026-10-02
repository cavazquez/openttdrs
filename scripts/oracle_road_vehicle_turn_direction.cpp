// Pinned GPL-2.0 OpenTTD bodies/fragments with adapted storage and outer loop.
#include <array>
#include <cassert>
#include <cstdint>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <vector>
using uint = unsigned;
#include "native-direction-enums.inc"
#include "native-constants.inc"
struct RoadDriveEntry { uint8_t x, y; };
#include "native-drive-arrays.inc"
const std::array<const RoadDriveEntry *, 64> drive_tables = {{
#include "native-drive-bindings.inc"
}};
constexpr int AM_ORIGINAL = 0;
#include "native-distance-constants.inc"
struct Settings { struct { int roadveh_acceleration_model = AM_ORIGINAL; } vehicle; } _settings_game;
struct RoadVehicle {
    int x_pos = 0, y_pos = 0;
    unsigned frame = 0, cur_speed = 112;
    Direction direction = DIR_N;
    void UpdateViewport(bool, bool) {}
#include "native-advance-distance.inc"
};
#include "native-direction-functions.inc"
bool native_turn_only(RoadVehicle *v, Direction new_dir) {
#include "native-turn-block.inc"
    return false;
}
void emit(unsigned table, unsigned step, const RoadVehicle &v, bool turn_only) {
    std::cout << table << ',' << step << ',' << v.frame << ',' << v.x_pos << ','
              << v.y_pos << ',' << unsigned(v.direction) << ',' << v.cur_speed << ',' << turn_only << '\n';
}
int main(int argc, char **argv) {
    assert(argc == 2 || argc == 3);
    const bool bay = argc == 3;
    std::ofstream predictions(argv[1]);
    assert(predictions);
    predictions << "table,substep,budget,x,y,direction\n" << std::fixed << std::setprecision(8);
    std::cout << "table,substep,frame,x,y,direction,speed,turn_only\n";
    for (unsigned table = 0; table < 64; ++table) {
        if (bay) {
            if (table < 32 || (table & 4) != 0) continue; // Entered-stop bindings repeat these arrays.
            assert(drive_tables[table] == drive_tables[table | 4]);
        } else if (table >= 32 || (table & 7) >= 6) {
            continue; // Reversal tables have separate controller branches.
        }
        const RoadDriveEntry *data = drive_tables[table];
        RoadVehicle v;
        v.x_pos = data[0].x;
        v.y_pos = data[0].y;
        v.direction = RoadVehGetNewDirection(&v, data[1].x, data[1].y);
        std::vector<RoadVehicle> states{v};
        emit(table, 0, v, false);
        for (unsigned step = 1; ; ++step) {
            assert(step < 64);
            const RoadDriveEntry rd = data[v.frame + 1];
            if ((rd.x & (RDE_NEXT_TILE | RDE_TURNED)) != 0) break;
            const int x = (v.x_pos & ~15) + (rd.x & 15);
            const int y = (v.y_pos & ~15) + (rd.y & 15);
            const Direction new_dir = RoadVehGetSlidingDirection(&v, x, y);
            const bool turn_only = native_turn_only(&v, new_dir);
            if (!turn_only) { ++v.frame; v.x_pos = x; v.y_pos = y; }
            emit(table, step, v, turn_only);
            states.push_back(v);
        }
        // Interpolate along the already recorded native sequence, including
        // stationary turn steps. This is independent of the Rust predictor.
        for (unsigned step = 0; step < states.size(); ++step) {
            for (uint initial_budget : {0U, 48U, 96U, 144U, 192U, 240U, 256U, 384U, 448U, 512U}) {
                uint budget = initial_budget;
                unsigned index = step;
                while (index + 1 < states.size()) {
                    RoadVehicle current = states[index];
                    const uint cost = current.GetAdvanceDistance();
                    if (budget < cost) break;
                    budget -= cost;
                    ++index;
                }
                RoadVehicle current = states[index];
                double x = current.x_pos, y = current.y_pos;
                if (index + 1 < states.size()) {
                    const auto &next = states[index + 1];
                    const double fraction = double(budget) / current.GetAdvanceDistance();
                    x += (next.x_pos - current.x_pos) * fraction;
                    y += (next.y_pos - current.y_pos) * fraction;
                }
                predictions << table << ',' << step << ',' << initial_budget << ',' << x << ','
                            << y << ',' << unsigned(current.direction) << '\n';
            }
        }
    }
}
