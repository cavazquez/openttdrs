// Unchanged bodies extracted from pinned GPL-2.0 OpenTTD; adapted storage/sinks.
#include <array>
#include <bit>
#include <initializer_list>
#include <source_location>
#include <vector>
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
enum EffectVehicleType { EV_STEAM_SMOKE, EV_DIESEL_SMOKE, EV_ELECTRIC_SPARK, EV_BREAKDOWN_SMOKE_AIRCRAFT, EV_END };
struct EffectVehicle;
std::vector<EffectVehicle *> created_effects;
unsigned pool_budget=0,allocation_attempts=0;
#include "native-visual-model.inc"
#include "native-visual-bits.inc"
#include "native-rail-flags.inc"
struct RailFlags { bool Test(VehicleRailFlag) const { return false; } };
struct StatusFlags {
    uint8_t value = 0;
    void operator=(VehState flag) { value = uint8_t(1U << uint8_t(flag)); }
    bool Test(VehState flag) const { return (value & (1U << uint8_t(flag))) != 0; }
    bool Any(std::initializer_list<VehState> flags) const { for(auto flag:flags) if(Test(flag))return true; return false; }
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
    static bool CanAllocateItem() { ++allocation_attempts; return created_effects.size()<pool_budget; }
    bool primary=false;
    uint16_t cur_speed=24, max_speed=160;
    uint8_t tick_counter=0, direction=1;
    uint tile=0, railtypes=1, engine_type=0;
    struct { uint cached_vis_effect=0; } vcache;
    struct { uint cached_power=2048,cached_weight=1024,cached_veh_length=8; } gcache;
    RailFlags flags;
    struct { bool ShouldStopAtStation(const Vehicle *, uint) const { return false; } } current_order;
    Vehicle *next=nullptr;
    bool IsPrimaryVehicle() const { return primary; }
    bool IsFrontEngine() const { return primary; }
    uint GetCurrentMaxSpeed() const { return max_speed; }
    const Vehicle *Next() const { return next; }
    static const Vehicle *From(const Vehicle *v) { return v; }
    void ShowVisualEffect() const;
};
struct EffectVehicle : Vehicle {
    EffectVehicle() { created_effects.push_back(this); }
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

using Direction=uint8_t;
using Train=Vehicle;
using RoadVehicle=Vehicle;
constexpr int VEHICLE_LENGTH=8,DIRDIFF_90RIGHT=2;
constexpr int CBID_VEHICLE_SPAWN_VISUAL_EFFECT=0x160,CALLBACK_FAILED=0xffff;
uint GB(uint value,uint shift,uint bits) { return (value>>shift)&((1U<<bits)-1); }
bool HasBit(uint value,uint bit) { return (value&(1U<<bit))!=0; }
Direction ReverseDir(Direction d) { return (d+4)&7; }
Direction ChangeDir(Direction d,int delta) { return (d+delta)&7; }
struct Randomizer {
    std::array<uint32_t,2> state{};
    uint32_t Next();
    void SetSeed(uint32_t seed);
};
#include "native-random-next.inc"
#include "native-random-seed.inc"
Randomizer randomizer;
unsigned rng_calls=0,callback_calls=0,sound_calls=0;
uint32_t Random(const std::source_location = std::source_location::current()) {
    ++rng_calls;return randomizer.Next();
}
#include "native-chance-i.inc"
#include "native-chance.inc"
uint16_t GetVehicleCallback(int,int,uint,int,const Vehicle *,std::array<int32_t,4> &regs) {
    ++callback_calls;regs.fill(0xF1);return 1;
}
#include "native-create-relative.inc"
#include "native-direction-table.inc"
#include "native-dispatch.inc"
struct { struct { uint smoke_amount=2; } vehicle; } _settings_game;
bool IsRailStationTile(uint) { return false; }
uint GetStationIndex(uint) { return 0; }
bool IsBridgeAboveVehicle(const Vehicle *) { return false; }
bool IsDepotTile(uint) { return false; }
bool IsTunnelTile(uint) { return false; }
bool HasPowerOnRail(uint,uint) { return true; }
uint GetTileRailType(uint) { return 0; }
constexpr int VSE_VISUAL_EFFECT=0;
void PlayVehicleSound(const Vehicle *,int) { ++sound_calls; }
#define NOT_REACHED() assert(false)
#include "native-show.inc"
int main() {
    std::cout<<"seed,model,free,heads,chain,tick_counter,smoke,speed,attempts,created,rng_calls,rng0,rng1,sound_calls\n";
    for(uint seed:{1,17})for(uint model:{1,2,3})for(uint free:{0,1,48})
    for(uint heads:{1,4,16})for(uint chain:{1,4})for(uint counter:{0,1,4})
    for(uint smoke:{0,1,2})for(uint speed:{24,160}) {
        pool_budget=free;allocation_attempts=rng_calls=callback_calls=sound_calls=0;
        randomizer.SetSeed(seed);_settings_game.vehicle.smoke_amount=smoke;
        std::vector<Vehicle> vehicles(heads*chain);
        for(uint i=0;i<vehicles.size();++i) {
            Vehicle &v=vehicles[i];v.type=VEH_TRAIN;v.primary=i%chain==0;
            v.tick_counter=counter;v.cur_speed=speed;v.vcache.cached_vis_effect=(model<<VE_TYPE_START)|VE_OFFSET_CENTRE;
            if(i%chain!=chain-1)v.next=&vehicles[i+1];
        }
        for(uint i=0;i<heads;++i)vehicles[i*chain].ShowVisualEffect();
        assert(callback_calls==0);
        std::cout<<seed<<','<<model<<','<<free<<','<<heads<<','<<chain<<','<<counter<<','<<smoke<<','<<speed<<','
                 <<allocation_attempts<<','<<created_effects.size()<<','<<rng_calls<<','<<randomizer.state[0]<<','<<randomizer.state[1]<<','<<sound_calls<<'\n';
        for(auto *effect:created_effects) {DoDrawVehicle(effect);delete effect;}
        created_effects.clear();
    }
}
