// Unchanged bodies extracted from pinned GPL-2.0 OpenTTD; adapted storage/sinks.
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
using uint = unsigned;
using SpriteID = uint32_t;
using Direction = uint8_t;
constexpr int VEH_TRAIN = 0, VEH_ROAD = 1;
constexpr int VEHICLE_LENGTH = 8, DIRDIFF_90RIGHT = 2;
constexpr int CBID_VEHICLE_SPAWN_VISUAL_EFFECT = 0x160, CALLBACK_FAILED = 0xffff;
enum EffectVehicleType { EV_STEAM_SMOKE, EV_DIESEL_SMOKE, EV_ELECTRIC_SPARK, EV_BREAKDOWN_SMOKE_AIRCRAFT };
enum class VehicleRailFlag { Flipped };
struct Flags { bool Test(VehicleRailFlag) const { return false; } };
struct Vehicle {
    int type = VEH_TRAIN, engine_type = 0;
    Direction direction = 1;
    struct { int cached_veh_length = 8; } gcache;
    Flags flags;
};
struct RoadVehicle { static const Vehicle *From(const Vehicle *v) { return v; } };
struct Train { static const Vehicle *From(const Vehicle *v) { return v; } };
uint GB(uint value, uint shift, uint bits) { return (value >> shift) & ((1U << bits) - 1); }
bool HasBit(uint value, uint bit) { return (value & (1U << bit)) != 0; }
Direction ReverseDir(Direction d) { return (d + 4) & 7; }
Direction ChangeDir(Direction d, int delta) { return (d + delta) & 7; }
uint Random() { return 0; } // Emission RNG is outside this lifetime oracle.
uint spawn_type;
uint16_t GetVehicleCallback(int, int, uint, int, const Vehicle *, std::array<int32_t, 4> &regs) {
    regs.fill(static_cast<int32_t>(spawn_type));
    return 1;
}
struct DrawLayer { SpriteID sprite = 0; };
struct SpriteSeq {
    std::array<DrawLayer, 1> seq{};
    void Set(SpriteID sprite) { seq[0].sprite = sprite; }
};
struct EffectVehicle {
    struct { SpriteSeq sprite_seq; } sprite_cache;
    uint8_t progress = 0;
    int z_pos = 0;
    unsigned updates = 0;
    void UpdatePositionAndViewport() { ++updates; }
};
#include "native-constants.inc"
#include "native-increment.inc"
#include "native-SteamSmoke-init.inc"
#include "native-SteamSmoke-tick.inc"
#include "native-DieselSmoke-init.inc"
#include "native-DieselSmoke-tick.inc"
#include "native-ElectricSpark-init.inc"
#include "native-ElectricSpark-tick.inc"
#include "native-Smoke-init.inc"
#include "native-Smoke-tick.inc"
constexpr int TO_INVALID = 0;
struct EffectProcs {
    void (*init_proc)(EffectVehicle *);
    bool (*tick_proc)(EffectVehicle *);
    int transparency;
};
// Enum is compact in this scaffold; each selected binding is a native row.
const std::array<EffectProcs, 4> effect_procs = {{
#include "native-selected-procs.inc"
}};
EffectVehicle *effect = nullptr;
EffectVehicleType actual_type;
void CreateEffectVehicleRel(const Vehicle *, int, int, int, EffectVehicleType type) {
    assert(effect == nullptr);
    effect = new EffectVehicle();
    actual_type = type;
    effect_procs[type].init_proc(effect);
}
#include "native-direction-table.inc"
#include "native-dispatch.inc"
int main() {
    std::cout << "effect_type,tick,alive,sprite_id,frame,rise,position_updates\n";
    for (uint type : {0xF1U, 0xF2U, 0xF3U, 0xFAU}) {
        spawn_type = type;
        Vehicle v;
        SpawnAdvancedVisualEffect(&v);
        assert(effect != nullptr);
        const SpriteID base = effect->sprite_cache.sprite_seq.seq[0].sprite;
        for (unsigned tick = 0; tick <= 96; ++tick) {
            if (tick > 0 && effect != nullptr) {
                const bool alive = effect_procs[actual_type].tick_proc(effect);
                if (!alive) effect = nullptr; // Native tick deleted it; do not dereference.
            }
            std::cout << type << ',' << tick << ',' << (effect != nullptr);
            if (effect != nullptr) {
                const SpriteID sprite = effect->sprite_cache.sprite_seq.seq[0].sprite;
                std::cout << ',' << sprite << ',' << sprite - base << ',' << effect->z_pos << ',' << effect->updates;
            } else {
                std::cout << ",-1,-1,-1,-1";
            }
            std::cout << '\n';
        }
        assert(effect == nullptr);
    }
}
