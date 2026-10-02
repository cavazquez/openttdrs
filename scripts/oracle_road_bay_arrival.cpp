// Native bay arrival/turn fragments with explicit standalone service stubs.
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
using uint = unsigned;
using TileIndex = unsigned;
#include "native-direction-enums.inc"
#include "native-constants.inc"
struct RoadDriveEntry { uint8_t x, y; };
#include "native-drive-arrays.inc"
const std::array<const RoadDriveEntry *, 64> drive_tables = {{
#include "native-drive-bindings.inc"
}};
#include "native-stop-frames.inc"
constexpr int AM_ORIGINAL = 0, RVS_ENTERED_STOP = 2, RVS_DRIVE_SIDE = 4;
constexpr int RVSB_IN_ROAD_STOP = 32, RVSB_IN_ROAD_STOP_END = 48;
constexpr int RVSB_IN_DT_ROAD_STOP = 64, RVSB_IN_DT_ROAD_STOP_END = 80;
constexpr int RVC_DRIVE_THROUGH_STOP_FRAME = 11;
constexpr int OT_GOTO_STATION = 1, OT_LEAVESTATION = 2;
constexpr int WC_VEHICLE_VIEW = 0, WID_VV_START_STOP = 0;
enum class RoadStopType { Bus, Truck };
enum class StationRandomTrigger { VehicleArrives };
enum class StationAnimationTrigger { VehicleArrives };
#include "native-distance-constants.inc"
struct Settings { struct { int roadveh_acceleration_model = AM_ORIGINAL, road_side = 0; } vehicle; } _settings_game;
struct Order {
    bool ShouldStopAtStation(const void *, int) const { return true; }
    bool IsType(int type) const { return type == OT_GOTO_STATION; }
    int GetDestination() const { return 0; }
    void Free() {}
};
struct RoadVehicle {
    int x_pos = 0, y_pos = 0;
    unsigned frame = 0, cur_speed = 112, state = 32;
    Direction direction = DIR_N;
    TileIndex tile = 0;
    int owner = 0, compatible_roadtypes = 0, last_station_visited = -1, index = 0;
    bool begin_loading = false;
    Order current_order;
    bool IsFrontEngine() const { return true; }
    bool IsBus() const { return true; }
    void UpdateViewport(bool, bool) {}
    void UpdatePosition() {}
    int UpdateInclination(bool, bool) { return 0; }
    void BeginLoading() {
#include "native-begin-loading-speed.inc"
        begin_loading = true;
    }
#include "native-advance-distance.inc"
};
struct Station { int index = 0; static Station *GetByTile(TileIndex) { static Station st; return &st; } };
struct RoadStop {
    bool busy = true;
    static RoadStop *GetByTile(TileIndex, RoadStopType) { static RoadStop rs; return &rs; }
    void SetEntranceBusy(bool value) { busy = value; }
    bool IsEntranceBusy() const { return busy; }
    static bool IsDriveThroughRoadStopContinuation(TileIndex, TileIndex) { return false; }
};
bool IsInsideMM(unsigned value, unsigned first, unsigned end) { return value >= first && value < end; }
bool HasBit(unsigned value, unsigned bit) { return (value & (1U << bit)) != 0; }
void SetBit(unsigned &value, unsigned bit) { value |= 1U << bit; }
int GetStationIndex(TileIndex) { return 0; }
int GetTileOwner(TileIndex) { return 0; }
RoadStopType GetRoadStopType(TileIndex) { return RoadStopType::Bus; }
bool IsDriveThroughStopTile(TileIndex) { return false; }
bool IsBayRoadStopTile(TileIndex) { return true; }
TileIndex TileAddByDir(TileIndex tile, Direction) { return tile; }
bool HasTileAnyRoadType(TileIndex, int) { return false; }
void RoadVehArrivesAt(RoadVehicle *, Station *) {}
void TriggerRoadStopRandomisation(Station *, TileIndex, StationRandomTrigger) {}
void TriggerRoadStopAnimation(Station *, TileIndex, StationAnimationTrigger) {}
void StartRoadVehSound(RoadVehicle *) {}
void SetWindowWidgetDirty(int, int, int) {}
void RoadZPosAffectSpeed(RoadVehicle *, int) {}
#include "native-direction-functions.inc"
bool native_turn_only(RoadVehicle *v, Direction new_dir) {
#include "native-turn-block.inc"
    return false;
}
bool native_bay_step(RoadVehicle *v) {
    const auto *data = drive_tables[v->state + (_settings_game.vehicle.road_side << RVS_DRIVE_SIDE)];
    const auto rd = data[v->frame + 1];
    assert((rd.x & (RDE_NEXT_TILE | RDE_TURNED)) == 0);
    const int x = (v->x_pos & ~15) + (rd.x & 15);
    const int y = (v->y_pos & ~15) + (rd.y & 15);
    const Direction new_dir = RoadVehGetSlidingDirection(v, x, y);
    if (native_turn_only(v, new_dir)) return true;
#include "native-arrival-branch.inc"
    ++v->frame;
    v->x_pos = x;
    v->y_pos = y;
    return true;
}
void emit(unsigned table, unsigned step, const RoadVehicle &v, bool result) {
    std::cout << table << ',' << step << ',' << v.frame << ',' << v.x_pos << ','
              << v.y_pos << ',' << unsigned(v.direction) << ',' << v.cur_speed << ','
              << HasBit(v.state, RVS_ENTERED_STOP) << ',' << result << ',' << v.begin_loading << '\n';
}
int main() {
    std::cout << "table,substep,frame,x,y,direction,speed,entered,result,begin_loading\n";
    for (unsigned table = 32; table < 64; ++table) {
        if ((table & (1U << RVS_ENTERED_STOP)) != 0) continue;
        _settings_game.vehicle.road_side = table >= 48;
        RoadStop::GetByTile(0, RoadStopType::Bus)->busy = true;
        const auto *data = drive_tables[table];
        RoadVehicle v;
        v.state = table & ~16U;
        v.x_pos = data[0].x;
        v.y_pos = data[0].y;
        v.direction = RoadVehGetNewDirection(&v, data[1].x, data[1].y);
        emit(table, 0, v, true);
        for (unsigned step = 1; ; ++step) {
            assert(step < 64);
            const bool result = native_bay_step(&v);
            emit(table, step, v, result);
            if (!result) { assert(v.begin_loading); break; }
        }
    }
}
