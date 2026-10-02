// Unchanged bodies extracted from pinned GPL-2.0 OpenTTD; adapted storage/sinks.
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
using uint = unsigned;
using SpriteID = uint32_t;
using PaletteID = uint32_t;
using TileIndex = uint32_t;
#include "native-vehicle-type.inc"
#include "native-vehicle-status.inc"
#include "native-transparency-enum.inc"
#include "native-palette-constants.inc"
enum EffectVehicleType { EV_STEAM_SMOKE, EV_DIESEL_SMOKE, EV_ELECTRIC_SPARK, EV_BREAKDOWN_SMOKE_AIRCRAFT };
struct StatusFlags {
    uint8_t value = 0;
    void operator=(VehState flag) { value = uint8_t(1U << uint8_t(flag)); }
    bool Test(VehState flag) const { return (value & (1U << uint8_t(flag))) != 0; }
};
struct DrawLayer { SpriteID sprite = 0; PaletteID pal = 0; };
struct SpriteSeq {
    uint count = 0;
    std::array<DrawLayer, 1> seq{};
#include "native-sprite-set.inc"
};
struct Bounds {}; // Bounds are passed to the sink but not certified here.
struct Vehicle {
    VehicleType type = VEH_EFFECT;
    int x_pos = 0, y_pos = 0, z_pos = 0;
    struct { SpriteSeq sprite_seq; } sprite_cache;
    StatusFlags vehstatus;
    Bounds bounds;
    static bool CanAllocateItem() { return true; }
};
struct EffectVehicle : Vehicle {
    uint8_t progress = 0;
    EffectVehicleType subtype = EV_STEAM_SMOKE;
    TileIndex tile = 0;
    unsigned updates = 0;
    void UpdateDeltaXY() {} // Native bounds setup is outside this palette contract.
    void UpdatePositionAndViewport() { ++updates; }
    TransparencyOption GetTransparencyOption() const;
    static const EffectVehicle *From(const Vehicle *v) {
        assert(v->type == VEH_EFFECT);
        return static_cast<const EffectVehicle *>(v);
    }
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
struct EffectProcs {
    void (*init_proc)(EffectVehicle *);
    bool (*tick_proc)(EffectVehicle *);
    TransparencyOption transparency;
};
// Compact storage, with each selected init/tick/transparency binding unchanged.
const std::array<EffectProcs, 4> _effect_procs = {{
#include "native-selected-procs.inc"
}};
#include "native-transparency.inc"
#include "native-create.inc"
PaletteID GetVehiclePalette(const Vehicle *) {
    assert(false && "effect must not request a company palette");
    return 0;
}
bool IsTransparencySet(TransparencyOption) { return false; }
bool IsInvisibilitySet(TransparencyOption) { return false; }
struct DrawCall { SpriteID sprite; PaletteID pal; int x, y, z; bool shadowed; };
DrawCall draw{};
unsigned draw_count = 0;
void StartSpriteCombine() { draw_count = 0; }
void EndSpriteCombine() {}
void AddSortableSpriteToDraw(SpriteID sprite, PaletteID pal, int x, int y, int z, const Bounds &, bool shadowed) {
    ++draw_count;
    draw = {sprite, pal, x, y, z, shadowed};
}
#include "native-draw.inc"
int main() {
    std::cout << "effect_type,tick,alive,sprite_id,frame,rise,position_updates,palette,shadowed,vehstatus,x,y,z\n";
    constexpr std::array<uint, 4> public_types = {0xF1, 0xF2, 0xF3, 0xFA};
    for (uint type = 0; type < public_types.size(); ++type) {
        EffectVehicle *effect = CreateEffectVehicle(16, 16, 10, EffectVehicleType(type));
        assert(effect != nullptr);
        const SpriteID base = effect->sprite_cache.sprite_seq.seq[0].sprite;
        for (unsigned tick = 0; tick <= 96; ++tick) {
            if (tick > 0 && effect != nullptr && !_effect_procs[type].tick_proc(effect)) {
                effect = nullptr; // Native tick deleted it; do not dereference.
            }
            std::cout << public_types[type] << ',' << tick << ',' << (effect != nullptr);
            if (effect != nullptr) {
                assert(effect->GetTransparencyOption() == TO_INVALID);
                DoDrawVehicle(effect);
                assert(draw_count == 1);
                std::cout << ',' << draw.sprite << ',' << draw.sprite - base << ',' << draw.z - 10 << ','
                          << effect->updates << ',' << draw.pal << ',' << draw.shadowed << ','
                          << uint(effect->vehstatus.value) << ',' << draw.x << ',' << draw.y << ',' << draw.z;
            } else {
                std::cout << ",-1,-1,-1,-1,-1,-1,-1,-1,-1,-1";
            }
            std::cout << '\n';
        }
        assert(effect == nullptr);
    }
}
